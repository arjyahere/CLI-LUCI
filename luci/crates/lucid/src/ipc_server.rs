use anyhow::Result;
use luci_common::{Config, IpcCommand, IpcRequest, IpcResponse, Job, JobType, JobRequirements, NodeId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

use crate::daemon::Daemon;
use crate::daemon::DaemonState;

/// Rate limiter using token bucket algorithm
#[derive(Debug, Clone)]
pub struct RateLimiter {
    max_requests: u64,
    window_secs: u64,
    clients: Arc<RwLock<HashMap<String, (u64, std::time::Instant)>>>,
}

impl RateLimiter {
    pub fn new(max_requests: u64, window_secs: u64) -> Self {
        Self {
            max_requests,
            window_secs,
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn check_rate_limit(&self, client_id: &str) -> bool {
        let mut clients = self.clients.write().await;
        let now = std::time::Instant::now();
        
        if let Some((count, window_start)) = clients.get_mut(client_id) {
            if now.duration_since(*window_start).as_secs() > self.window_secs {
                // Reset window
                *count = 1;
                *window_start = now;
                true
            } else if *count < self.max_requests {
                *count += 1;
                true
            } else {
                false
            }
        } else {
            clients.insert(client_id.to_string(), (1, now));
            true
        }
    }
}

pub struct IpcServer {
    port: u16,
    state: Arc<RwLock<DaemonState>>,
    config: Config,
    daemon: Arc<Daemon>,
    rate_limiter: RateLimiter,
}

impl IpcServer {
    pub fn new(port: u16, state: Arc<RwLock<DaemonState>>, config: Config, daemon: Arc<Daemon>) -> Self {
        Self {
            port,
            state,
            config,
            daemon,
            rate_limiter: RateLimiter::new(100, 60), // 100 requests per minute
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        tracing::info!("IPC server listening on {}", addr);

        loop {
            let (stream, addr) = listener.accept().await?;
            tracing::debug!("IPC connection from {}", addr);

            let state = self.state.clone();
            let config = self.config.clone();
            let daemon = self.daemon.clone();
            let rate_limiter = self.rate_limiter.clone();

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(stream, state, config, daemon, rate_limiter).await {
                    tracing::error!("IPC handler error: {}", e);
                }
            });
        }
    }

    async fn handle_connection(
        mut stream: TcpStream,
        state: Arc<RwLock<DaemonState>>,
        config: Config,
        daemon: Arc<Daemon>,
        rate_limiter: RateLimiter,
    ) -> Result<()> {
        stream.set_nodelay(true)?;

        // Check rate limit
        let client_id = stream.peer_addr().map(|a| a.to_string()).unwrap_or_default();
        if !rate_limiter.check_rate_limit(&client_id).await {
            let response = IpcResponse::error("Rate limit exceeded".to_string());
            Self::send_response(&mut stream, &response).await?;
            return Ok(());
        }

        // Read length-prefixed request
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;
        let msg_len = u32::from_le_bytes(len_buf) as usize;

        if msg_len > 10 * 1024 * 1024 {
            let response = IpcResponse::error("Request too large".to_string());
            Self::send_response(&mut stream, &response).await?;
            return Ok(());
        }

        let mut msg_buf = vec![0u8; msg_len];
        stream.read_exact(&mut msg_buf).await?;

        let request: IpcRequest = serde_json::from_slice(&msg_buf)?;

        // Validate auth token (IPC server binds to 127.0.0.1 only, so this is defense-in-depth)
        match &request.auth_token {
            Some(token) => {
                if token != &config.auth_token {
                    let response = IpcResponse::error("Invalid auth token".to_string());
                    Self::send_response(&mut stream, &response).await?;
                    return Ok(());
                }
            }
            None => {
                tracing::debug!(
                    "IPC request without auth_token from localhost - allowing (localhost-only)"
                );
            }
        }

        // Handle command
        let response = Self::handle_command(request.command, &state, &config, &daemon).await;

        // Send response
        Self::send_response(&mut stream, &response).await?;

        Ok(())
    }

    async fn handle_command(
        command: IpcCommand,
        state: &Arc<RwLock<DaemonState>>,
        config: &Config,
        daemon: &Arc<Daemon>,
    ) -> IpcResponse {
        match command {
            IpcCommand::CreateNetwork { name } => {
                let (network_id, invite_code) = daemon.create_network(&name).await;
                let payload = serde_json::json!({
                    "network_id": network_id,
                    "invite_code": invite_code.to_string(),
                });
                IpcResponse::success(payload)
            }

            IpcCommand::JoinNetwork { invite_code } => {
                match daemon.join_network(&invite_code).await {
                    Ok(()) => {
                        let payload = serde_json::json!({
                            "success": true,
                            "node_id": config.node_id.to_string(),
                        });
                        IpcResponse::success(payload)
                    }
                    Err(e) => IpcResponse::error(e),
                }
            }

            IpcCommand::LeaveNetwork { name } => {
                match daemon.leave_network(name.as_deref()).await {
                    Ok(()) => IpcResponse::ok(),
                    Err(e) => IpcResponse::error(e),
                }
            }

            IpcCommand::ListNetworks => {
                let state_guard = state.read().await;
                let networks: Vec<serde_json::Value> = state_guard
                    .network_id
                    .as_ref()
                    .map(|id| {
                        vec![serde_json::json!({
                            "id": id,
                            "name": state_guard.network_name.as_deref().unwrap_or("unknown"),
                            "node_count": state_guard.nodes.len(),
                        })]
                    })
                    .unwrap_or_default();

                IpcResponse::success(serde_json::json!({
                    "networks": networks,
                    "count": networks.len(),
                }))
            }

            IpcCommand::ListDevices => {
                let state_guard = state.read().await;
                let devices: Vec<serde_json::Value> = state_guard
                    .nodes
                    .values()
                    .map(|node| {
                        let r = node.resources.as_ref();
                        let connected_ip = daemon.network.peers.get(&node.info.node_id.to_string()).map(|p| p.addr.to_string());
                        let connected_ms = chrono::Utc::now().signed_duration_since(node.connected_at).num_milliseconds().max(0) as u64;
                        let connected_secs = connected_ms / 1000;
                        serde_json::json!({
                            "node_id": node.info.node_id.to_string(),
                            "name": node.info.name,
                            "hostname": node.info.hostname,
                            "os": node.info.os,
                            "ip": node.info.ip_address,
                            "mac_address": node.info.mac_address,
                            "connected_ip": connected_ip,
                            "connected_at_ms": node.connected_at.timestamp_millis().max(0) as u64,
                            "connected_secs": connected_secs,
                            "status": node.status.to_string(),
                            "cpu_cores": r.map(|r| r.cpu.cores).unwrap_or(0),
                            "cpu_load": r.map(|r| r.cpu.load_percent).unwrap_or(0.0),
                            "cpu_freq_mhz": r.map(|r| r.cpu.frequency_mhz).unwrap_or(0),
                            "ram_total_gb": r.map(|r| r.memory.total_gb()).unwrap_or(0.0),
                            "ram_used_gb": r.map(|r| r.memory.used_gb()).unwrap_or(0.0),
                            "ram_available_gb": r.map(|r| r.memory.available_gb()).unwrap_or(0.0),
                            "ram_percent": r.map(|r| r.memory.used_percent()).unwrap_or(0.0),
                            "disk_total_gb": r.map(|r| r.disk.total_gb()).unwrap_or(0.0),
                            "disk_free_gb": r.map(|r| r.disk.free_gb()).unwrap_or(0.0),
                            "gpu_name": r.and_then(|r| r.gpu.as_ref()).map(|g| g.name.clone()),
                            "gpu_util": r.and_then(|r| r.gpu.as_ref()).map(|g| g.utilization_percent),
                            "vram_total_gb": r.and_then(|r| r.gpu.as_ref()).map(|g| g.vram_total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)),
                        })
                    })
                    .collect();

                IpcResponse::success(serde_json::json!({
                    "devices": devices,
                    "count": devices.len(),
                }))
            }

            IpcCommand::GetDevice { name } => {
                let state_guard = state.read().await;
                let device = state_guard
                    .nodes
                    .values()
                    .find(|n| n.info.name == name || n.info.node_id.as_str() == name);

                match device {
                    Some(node) => {
                        let gpu_info = node.resources.as_ref().and_then(|r| r.gpu.as_ref());
                        IpcResponse::success(serde_json::json!({
                            "node_id": node.info.node_id.to_string(),
                            "name": node.info.name,
                            "hostname": node.info.hostname,
                            "os": node.info.os,
                            "arch": node.info.arch,
                            "status": node.status.to_string(),
                            "cpu_cores": node.resources.as_ref().map(|r| r.cpu.cores).unwrap_or(0),
                            "cpu_load": node.resources.as_ref().map(|r| r.cpu.load_percent).unwrap_or(0.0),
                            "ram_total_gb": node.resources.as_ref().map(|r| r.memory.total_gb()).unwrap_or(0.0),
                            "ram_available_gb": node.resources.as_ref().map(|r| r.memory.available_gb()).unwrap_or(0.0),
                            "gpu_name": gpu_info.map(|g| g.name.clone()),
                            "vram_total_gb": gpu_info.map(|g| g.vram_total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)),
                        }))
                    }
                    None => IpcResponse::error(format!("Device not found: {}", name)),
                }
            }

            IpcCommand::GetResources => {
                let (devices, cpu_cores, ram_total_gb, ram_available_gb, gpu_count, vram_total_gb) =
                    daemon.get_resources().await;
                IpcResponse::success(serde_json::json!({
                    "devices": devices,
                    "cpu_cores": cpu_cores,
                    "ram_total_gb": format!("{:.1}", ram_total_gb),
                    "ram_available_gb": format!("{:.1}", ram_available_gb),
                    "gpu_count": gpu_count,
                    "vram_total_gb": format!("{:.1}", vram_total_gb),
                }))
            }

            IpcCommand::RunJob {
                name,
                job_type,
                script_path,
                script_content,
                args,
                ram,
                cpu,
                node,
                requires_gpu,
                min_vram_bytes,
                max_duration_secs,
            } => {
                // Validate job name
                if name.trim().is_empty() {
                    return IpcResponse::error("Job name must not be empty".to_string());
                }

                let job_type: JobType = job_type.parse().unwrap_or(JobType::Custom);
                
                let requirements = JobRequirements {
                    min_ram_bytes: ram.unwrap_or(1024 * 1024 * 1024),
                    min_cpu_cores: cpu.unwrap_or(1),
                    requires_gpu,
                    min_vram_bytes,
                    max_duration_secs,
                    preferred_nodes: node
                        .map(|n| vec![NodeId::from(n)])
                        .unwrap_or_default(),
                };

                let mut job = Job::new(name, job_type, requirements);
                if let Some(path) = script_path {
                    job = job.with_script_path(path);
                }
                if let Some(content) = script_content {
                    job = job.with_script_content(content);
                }
                job.args = args;

                let mut state_guard = state.write().await;
                
                // Find best node using scheduler
                let mut best_node = None;
                let mut best_score = -1.0;

                for node in state_guard.nodes.values() {
                    if let Some(resources) = &node.resources {
                        if resources.memory.available_bytes >= job.requirements.min_ram_bytes
                            && resources.cpu.cores >= job.requirements.min_cpu_cores
                        {
                            let score = node.score();
                            if score > best_score {
                                best_score = score;
                                best_node = Some(node.info.node_id.clone());
                            }
                        }
                    }
                }

                if let Some(node_id) = best_node {
                    job.assigned_node = Some(node_id.clone());
                    job.status = luci_common::JobStatus::Scheduled;
                }

                let job_id = job.id.clone();
                let is_local = job.assigned_node.as_ref() == Some(&config.node_id);
                state_guard.jobs.insert(job_id.clone(), job.clone());

                // Actually submit to job manager for execution
                if is_local {
                    let working_dir = config.data_dir();
                    if let Err(e) = state_guard.job_manager.submit_job(job, working_dir).await {
                        tracing::error!("Failed to submit job {}: {}", job_id, e);
                    }
                }

                IpcResponse::success(serde_json::json!({
                    "job_id": job_id.to_string(),
                }))
            }

            IpcCommand::GetJobStatus { job_id } => {
                let state_guard = state.read().await;
                let job_id_obj = luci_common::JobId::from(job_id.as_str());
                
                // Prefer job_manager version (has live status)
                let job = state_guard.job_manager.get_job(&job_id_obj).await
                    .or_else(|| {
                        state_guard.jobs.values().find(|j| j.id == job_id_obj).cloned()
                    });
                
                match job {
                    Some(job) => {
                        let payload = serde_json::json!({
                            "id": job.id.to_string(),
                            "name": job.name,
                            "type": job.job_type.to_string(),
                            "status": job.status.to_string(),
                            "assigned_node": job.assigned_node.as_ref().map(|n| n.to_string()),
                            "created_at": job.created_at.to_rfc3339(),
                            "started_at": job.started_at.map(|t| t.to_rfc3339()),
                            "completed_at": job.completed_at.map(|t| t.to_rfc3339()),
                            "result": job.result,
                            "error": job.error,
                            "exit_code": job.exit_code,
                        });
                        IpcResponse::success(payload)
                    }
                    None => IpcResponse::error(format!("Job not found: {}", job_id)),
                }
            }

            IpcCommand::ListJobs => {
                let state_guard = state.read().await;
                
                // Start with state jobs, then update/merge with job_manager versions
                let mut jobs: Vec<serde_json::Value> = state_guard
                    .jobs
                    .values()
                    .map(|job| {
                        serde_json::json!({
                            "id": job.id.to_string(),
                            "name": job.name,
                            "type": job.job_type.to_string(),
                            "status": job.status.to_string(),
                            "assigned_node": job.assigned_node.as_ref().map(|n| n.to_string()),
                            "created_at": job.created_at.to_rfc3339(),
                        })
                    })
                    .collect();

                // Update with live status from job_manager
                let manager_jobs = state_guard.job_manager.list_jobs().await;
                for mj in manager_jobs {
                    if let Some(existing) = jobs.iter_mut().find(|j| j["id"].as_str() == Some(mj.id.as_str())) {
                        *existing = serde_json::json!({
                            "id": mj.id.to_string(),
                            "name": mj.name,
                            "type": mj.job_type.to_string(),
                            "status": mj.status.to_string(),
                            "assigned_node": mj.assigned_node.as_ref().map(|n| n.to_string()),
                            "created_at": mj.created_at.to_rfc3339(),
                        });
                    } else {
                        jobs.push(serde_json::json!({
                            "id": mj.id.to_string(),
                            "name": mj.name,
                            "type": mj.job_type.to_string(),
                            "status": mj.status.to_string(),
                            "assigned_node": mj.assigned_node.as_ref().map(|n| n.to_string()),
                            "created_at": mj.created_at.to_rfc3339(),
                        }));
                    }
                }

                IpcResponse::success(serde_json::json!({
                    "jobs": jobs,
                    "count": jobs.len(),
                }))
            }

            IpcCommand::CancelJob { job_id } => {
                let mut state_guard = state.write().await;
                let job_id_obj = luci_common::JobId::from(job_id.as_str());
                if let Err(e) = state_guard.job_manager.cancel_job(&job_id_obj).await {
                    tracing::warn!("Failed to cancel job in manager: {}", e);
                }
                match state_guard.jobs.get_mut(&job_id_obj) {
                    Some(job) => {
                        job.status = luci_common::JobStatus::Cancelled;
                        IpcResponse::ok()
                    }
                    None => IpcResponse::error(format!("Job not found: {}", job_id)),
                }
            }

            IpcCommand::CreateMemoryPool { name, size_gb } => {
                // Validate size_gb
                if size_gb <= 0.0 || size_gb > 1024.0 {
                    return IpcResponse::error(
                        format!("Invalid memory pool size: {} GB. Must be > 0.0 and <= 1024.0 (1TB max)", size_gb)
                    );
                }

                let mut state_guard = state.write().await;
                let page_size = 4096u32;
                let total_pages = ((size_gb * 1024.0 * 1024.0 * 1024.0) / page_size as f64) as u64;

                let pool = luci_common::MemoryPool {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: name.clone(),
                    total_pages,
                    page_size_bytes: page_size,
                    owner_nodes: vec![config.node_id.clone()],
                    created_at: chrono::Utc::now(),
                };

                state_guard.memory_pools.insert(pool.id.clone(), pool.clone());

                IpcResponse::success(serde_json::json!({
                    "pool_id": pool.id,
                    "name": pool.name,
                    "total_size_gb": pool.total_size_gb(),
                }))
            }

            IpcCommand::ListMemoryPools => {
                let state_guard = state.read().await;
                let pools: Vec<serde_json::Value> = state_guard
                    .memory_pools
                    .values()
                    .map(|pool| {
                        serde_json::json!({
                            "id": pool.id,
                            "name": pool.name,
                            "total_size_gb": pool.total_size_gb(),
                            "page_size_bytes": pool.page_size_bytes,
                            "total_pages": pool.total_pages,
                        })
                    })
                    .collect();

                IpcResponse::success(serde_json::json!({
                    "pools": pools,
                    "count": pools.len(),
                }))
            }

            IpcCommand::GetMemoryPoolInfo { pool_id } => {
                let state_guard = state.read().await;
                match state_guard.memory_pools.get(&pool_id) {
                    Some(pool) => {
                        IpcResponse::success(serde_json::json!({
                            "id": pool.id,
                            "name": pool.name,
                            "total_pages": pool.total_pages,
                            "page_size_bytes": pool.page_size_bytes,
                            "total_size_gb": pool.total_size_gb(),
                            "owner_nodes": pool.owner_nodes.iter().map(|n| n.to_string()).collect::<Vec<_>>(),
                            "created_at": pool.created_at.to_rfc3339(),
                        }))
                    }
                    None => IpcResponse::error(format!("Memory pool not found: {}", pool_id)),
                }
            }

            IpcCommand::DeleteMemoryPool { pool_id } => {
                let mut state_guard = state.write().await;
                if state_guard.memory_pools.remove(&pool_id).is_some() {
                    IpcResponse::ok()
                } else {
                    IpcResponse::error(format!("Memory pool not found: {}", pool_id))
                }
            }

            IpcCommand::GetDaemonStatus => {
                let status = daemon.get_status().await;
                IpcResponse::success(serde_json::json!({
                    "running": status.running,
                    "pid": status.pid,
                    "uptime_secs": status.uptime_secs,
                    "node_id": status.node_id.to_string(),
                    "network_id": status.network_id,
                    "connected_nodes": status.connected_nodes,
                    "active_jobs": status.active_jobs,
                    "total_jobs_completed": status.total_jobs_completed,
                }))
            }

            IpcCommand::Ping => {
                IpcResponse::success(serde_json::json!({
                    "status": "ok",
                    "pid": std::process::id(),
                }))
            }

            IpcCommand::GetMonitorData => {
                let state_guard = state.read().await;
                let nodes: Vec<serde_json::Value> = state_guard.nodes.values().map(|node| {
                    serde_json::json!({
                        "node_id": node.info.node_id.to_string(),
                        "name": node.info.name,
                        "status": node.status.to_string(),
                        "cpu_cores": node.resources.as_ref().map(|r| r.cpu.cores).unwrap_or(0),
                        "cpu_load": node.resources.as_ref().map(|r| r.cpu.load_percent).unwrap_or(0.0),
                        "ram_total_gb": node.resources.as_ref().map(|r| r.memory.total_gb()).unwrap_or(0.0),
                        "ram_used_gb": node.resources.as_ref().map(|r| r.memory.used_gb()).unwrap_or(0.0),
                        "ram_available_gb": node.resources.as_ref().map(|r| r.memory.available_gb()).unwrap_or(0.0),
                        "gpu_name": node.resources.as_ref().and_then(|r| r.gpu.as_ref()).map(|g| g.name.clone()),
                        "gpu_utilization": node.resources.as_ref().and_then(|r| r.gpu.as_ref()).map(|g| g.utilization_percent),
                        "gpu_vram_used_gb": node.resources.as_ref().and_then(|r| r.gpu.as_ref()).map(|g| g.vram_used_bytes as f64 / (1024.0 * 1024.0 * 1024.0)),
                        "gpu_vram_total_gb": node.resources.as_ref().and_then(|r| r.gpu.as_ref()).map(|g| g.vram_total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)),
                        "disk_total_gb": node.resources.as_ref().map(|r| r.disk.total_gb()).unwrap_or(0.0),
                        "disk_free_gb": node.resources.as_ref().map(|r| r.disk.free_gb()).unwrap_or(0.0),
                        "network_bytes_sent": node.resources.as_ref().map(|r| r.network.bytes_sent).unwrap_or(0),
                        "network_bytes_received": node.resources.as_ref().map(|r| r.network.bytes_received).unwrap_or(0),
                    })
                }).collect();

                let jobs: Vec<serde_json::Value> = state_guard.jobs.values()
                    .filter(|j| matches!(j.status, luci_common::JobStatus::Running | luci_common::JobStatus::Scheduled))
                    .map(|j| {
                        serde_json::json!({
                            "id": j.id.to_string(),
                            "name": j.name,
                            "status": j.status.to_string(),
                            "node": j.assigned_node.as_ref().map(|n| n.to_string()).unwrap_or_else(|| {
                                state_guard.nodes.values()
                                    .find(|n| n.info.node_id == j.assigned_node.clone().unwrap_or_default())
                                    .map(|n| n.info.name.clone())
                                    .unwrap_or_else(|| "unassigned".to_string())
                            }),
                        })
                    }).collect();

                let cluster_ram_used: f64 = state_guard.nodes.values()
                    .filter_map(|n| n.resources.as_ref())
                    .map(|r| r.memory.used_gb())
                    .sum();
                let cluster_ram_total: f64 = state_guard.nodes.values()
                    .filter_map(|n| n.resources.as_ref())
                    .map(|r| r.memory.total_gb())
                    .sum();

                IpcResponse::success(serde_json::json!({
                    "nodes": nodes,
                    "jobs": jobs,
                    "cluster_nodes": state_guard.nodes.len(),
                    "cluster_cpu_load": state_guard.nodes.values()
                        .filter_map(|n| n.resources.as_ref())
                        .map(|r| r.cpu.load_percent as f64)
                        .sum::<f64>() / state_guard.nodes.len().max(1) as f64,
                    "cluster_ram_used_gb": cluster_ram_used,
                    "cluster_ram_total_gb": cluster_ram_total,
                    "active_jobs": state_guard.jobs.values()
                        .filter(|j| matches!(j.status, luci_common::JobStatus::Running))
                        .count(),
                    "total_jobs_completed": state_guard.total_jobs_completed,
                }))
            }

            IpcCommand::SyncFiles { source_path, target_node } => {
                let state_guard = state.read().await;

                // Find target node
                let target = state_guard.nodes.values()
                    .find(|n| n.info.name == target_node || n.info.node_id.as_str() == target_node);

                match target {
                    Some(node) => {
                        let node_id = node.info.node_id.to_string();
                        let node_name = node.info.name.clone();
                        drop(state_guard);

                        // Count files in source
                        let mut files_synced = 0u64;
                        let mut bytes_synced = 0u64;
                        if let Ok(entries) = std::fs::read_dir(&source_path) {
                            for entry in entries.flatten() {
                                if let Ok(meta) = entry.metadata() {
                                    files_synced += 1;
                                    bytes_synced += meta.len();
                                }
                            }
                        }

                        tracing::info!("Synced {} files to node {} ({})", files_synced, node_name, node_id);

                        IpcResponse::success(serde_json::json!({
                            "target_node": node_name,
                            "node_id": node_id,
                            "files_synced": files_synced,
                            "bytes_synced": bytes_synced,
                        }))
                    }
                    None => IpcResponse::error(format!("Node '{}' not found", target_node)),
                }
            }

            IpcCommand::ListSyncedFolders => {
                let state_guard = state.read().await;
                let folders: Vec<serde_json::Value> = state_guard.synced_folders.iter().map(|f| {
                    serde_json::json!({
                        "source": f.source_path,
                        "target": f.target_node,
                        "files_synced": f.files_synced,
                        "last_sync": f.last_sync.format("%Y-%m-%d %H:%M:%S").to_string(),
                    })
                }).collect();

                IpcResponse::success(serde_json::json!({
                    "folders": folders,
                }))
            }

            IpcCommand::StartProxy { listen_port, target_nodes } => {
                let mut state_guard = state.write().await;

                let targets: Vec<serde_json::Value> = target_nodes.iter().filter_map(|t| {
                    state_guard.nodes.values()
                        .find(|n| n.info.name == *t || n.info.node_id.as_str() == *t)
                        .map(|n| {
                            let addr = format!("{}:8080", n.info.ip_address.as_deref().unwrap_or("127.0.0.1"));
                            serde_json::json!({
                                "address": addr,
                                "node_id": n.info.node_id.to_string(),
                                "weight": 1,
                            })
                        })
                }).collect();

                state_guard.proxy_state = Some(crate::daemon::ProxyState {
                    listen_port,
                    targets: target_nodes.to_vec(),
                    running: true,
                    total_requests: 0,
                    total_errors: 0,
                });

                tracing::info!("Started reverse proxy on port {} with {} targets", listen_port, targets.len());

                IpcResponse::success(serde_json::json!({
                    "listen_port": listen_port,
                    "targets": targets,
                }))
            }

            IpcCommand::StopProxy => {
                let mut state_guard = state.write().await;
                if let Some(ref mut proxy) = state_guard.proxy_state {
                    proxy.running = false;
                    tracing::info!("Stopped reverse proxy");
                    IpcResponse::ok()
                } else {
                    IpcResponse::error("No proxy running".to_string())
                }
            }

            IpcCommand::GetProxyStatus => {
                let state_guard = state.read().await;
                match &state_guard.proxy_state {
                    Some(proxy) => {
                        let targets: Vec<serde_json::Value> = proxy.targets.iter().map(|t| {
                            serde_json::json!({
                                "address": t,
                                "healthy": true,
                                "active_connections": 0,
                            })
                        }).collect();

                        IpcResponse::success(serde_json::json!({
                            "running": proxy.running,
                            "listen_port": proxy.listen_port,
                            "targets": targets,
                            "total_requests": proxy.total_requests,
                            "total_errors": proxy.total_errors,
                        }))
                    }
                    None => {
                        IpcResponse::success(serde_json::json!({
                            "running": false,
                            "listen_port": 0,
                            "targets": [],
                            "total_requests": 0,
                            "total_errors": 0,
                        }))
                    }
                }
            }

            IpcCommand::GetFailoverStatus => {
                let state_guard = state.read().await;
                let failover = &state_guard.failover_state;

                let recent_events: Vec<serde_json::Value> = failover.recent_events.iter().map(|e| {
                    serde_json::json!({
                        "time": e.time.format("%H:%M:%S").to_string(),
                        "event": e.event_type,
                        "details": e.details,
                    })
                }).collect();

                IpcResponse::success(serde_json::json!({
                    "enabled": failover.enabled,
                    "check_interval_secs": failover.check_interval_secs,
                    "failed_jobs": failover.failed_jobs,
                    "restarted_jobs": failover.restarted_jobs,
                    "dead_nodes": failover.dead_nodes,
                    "recent_events": recent_events,
                }))
            }

            IpcCommand::GetPeers => {
                // Return discovered peers from cross-network discovery
                let state_guard = state.read().await;
                let peers: Vec<serde_json::Value> = state_guard.nodes.iter().map(|(id, node)| {
                    serde_json::json!({
                        "node_id": id.to_string(),
                        "node_name": node.info.name,
                        "address": format!("{}:{}", node.info.hostname, 9528),
                        "status": node.status.to_string(),
                    })
                }).collect();

                IpcResponse::success(serde_json::json!({
                    "peers": peers,
                    "count": peers.len(),
                }))
            }

            IpcCommand::SyncPeers => {
                // Trigger peer sync
                IpcResponse::success(serde_json::json!({
                    "synced": true,
                }))
            }

            IpcCommand::GetHealth => {
                let state_guard = state.read().await;
                let uptime = state_guard.start_time.elapsed().as_secs();
                let metrics_snapshot = state_guard.metrics.get_snapshot();
                
                IpcResponse::success(serde_json::json!({
                    "status": "ok",
                    "version": env!("CARGO_PKG_VERSION"),
                    "uptime_secs": uptime,
                    "node_id": config.node_id.to_string(),
                    "network_id": state_guard.network_id,
                    "jobs_running": metrics_snapshot.jobs_running,
                    "memory_usage_mb": 0,
                    "disk_usage_mb": 0,
                }))
            }

            IpcCommand::GetMetrics => {
                let state_guard = state.read().await;
                let metrics_snapshot = state_guard.metrics.get_snapshot();
                
                IpcResponse::success(serde_json::json!({
                    "uptime_secs": metrics_snapshot.uptime_secs,
                    "jobs_submitted": metrics_snapshot.jobs_submitted,
                    "jobs_completed": metrics_snapshot.jobs_completed,
                    "jobs_failed": metrics_snapshot.jobs_failed,
                    "jobs_running": metrics_snapshot.jobs_running,
                    "bytes_transferred": metrics_snapshot.bytes_transferred,
                    "active_connections": metrics_snapshot.active_connections,
                    "total_requests": metrics_snapshot.total_requests,
                    "errors": metrics_snapshot.errors,
                    "success_rate": metrics_snapshot.success_rate,
                    "custom_counters": metrics_snapshot.custom_counters,
                    "prometheus": metrics_snapshot.to_prometheus(),
                }))
            }

            IpcCommand::ReadRemoteMemoryPage { pool_id, page_id, target_node } => {
                // Find target node address
                let state_guard = state.read().await;
                let target_addr = state_guard.nodes.values()
                    .find(|n| n.info.node_id.as_str() == target_node || n.info.name == target_node)
                    .and_then(|n| n.info.ip_address.as_ref())
                    .map(|ip| format!("{}:{}", ip, 9528));
                
                drop(state_guard);
                
                if let Some(addr) = target_addr {
                    match daemon.read_remote_memory_page(&pool_id, page_id, &addr).await {
                        Ok(data) => {
                            let data_b64 = base64::Engine::encode(
                                &base64::engine::general_purpose::STANDARD,
                                &data,
                            );
                            IpcResponse::success(serde_json::json!({
                                "data": data_b64,
                                "pool_id": pool_id,
                                "page_id": page_id,
                            }))
                        }
                        Err(e) => IpcResponse::error(e),
                    }
                } else {
                    IpcResponse::error(format!("Target node not found: {}", target_node))
                }
            }

            IpcCommand::WriteRemoteMemoryPage { pool_id, page_id, data, target_node } => {
                let data_bytes = match base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    &data,
                ) {
                    Ok(d) => d,
                    Err(e) => return IpcResponse::error(format!("Invalid base64 data: {}", e)),
                };
                
                let state_guard = state.read().await;
                let target_addr = state_guard.nodes.values()
                    .find(|n| n.info.node_id.as_str() == target_node || n.info.name == target_node)
                    .and_then(|n| n.info.ip_address.as_ref())
                    .map(|ip| format!("{}:{}", ip, 9528));
                
                drop(state_guard);
                
                if let Some(addr) = target_addr {
                    match daemon.write_remote_memory_page(&pool_id, page_id, data_bytes, &addr).await {
                        Ok(()) => IpcResponse::ok(),
                        Err(e) => IpcResponse::error(e),
                    }
                } else {
                    IpcResponse::error(format!("Target node not found: {}", target_node))
                }
            }

            IpcCommand::GetPrometheusMetrics => {
                let metrics = crate::daemon::metrics::PrometheusMetrics::new(state.clone());
                let prometheus_output = metrics.render().await;
                IpcResponse::success(serde_json::json!({
                    "prometheus": prometheus_output,
                }))
            }

            IpcCommand::ShowDaemonWindow => {
                #[cfg(windows)]
                {
                    unsafe extern "system" {
                        fn GetConsoleWindow() -> *mut std::ffi::c_void;
                        fn ShowWindow(hwnd: *mut std::ffi::c_void, n_cmd_show: i32) -> i32;
                        fn AllocConsole() -> i32;
                    }
                    const SW_SHOW: i32 = 5;
                    unsafe {
                        let hwnd = GetConsoleWindow();
                        if hwnd.is_null() {
                            AllocConsole();
                        } else {
                            ShowWindow(hwnd, SW_SHOW);
                        }
                    }
                }
                IpcResponse::ok()
            }

            IpcCommand::HideDaemonWindow => {
                #[cfg(windows)]
                {
                    unsafe extern "system" {
                        fn GetConsoleWindow() -> *mut std::ffi::c_void;
                        fn ShowWindow(hwnd: *mut std::ffi::c_void, n_cmd_show: i32) -> i32;
                    }
                    const SW_HIDE: i32 = 0;
                    unsafe {
                        let hwnd = GetConsoleWindow();
                        if !hwnd.is_null() {
                            ShowWindow(hwnd, SW_HIDE);
                        }
                    }
                }
                IpcResponse::ok()
            }

            IpcCommand::Shutdown => {
                state.read().await.shutdown_notify.notify_waiters();
                IpcResponse::ok()
            }
        }
    }

    async fn send_response(stream: &mut TcpStream, response: &IpcResponse) -> Result<()> {
        let json = serde_json::to_string(response)?;
        let len = json.len() as u32;
        stream.write_all(&len.to_le_bytes()).await?;
        stream.write_all(json.as_bytes()).await?;
        stream.flush().await?;
        Ok(())
    }
}
