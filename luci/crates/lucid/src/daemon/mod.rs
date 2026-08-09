pub mod audit;
pub mod cross_network;
pub mod discovery;
pub mod distributed_tracing;
pub mod file_sync;
pub mod job;
pub mod metrics;
pub mod network;
pub mod proxy;
pub mod resource;
pub mod scheduler;
pub mod web_dashboard;

use anyhow::{Context, Result};
use luci_common::{
    Config, DaemonStatus, InviteCode, Job, JobId, MetricsCollector, NodeId, NodeState, NodeStatus,
};
use luci_protocol::{ProtocolMessage, MessageType, Payload};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::RwLock;

use crate::ipc_server::IpcServer;
use self::discovery::DiscoveryService;
use self::job::JobManager;
pub use self::network::NetworkManager;
use self::resource::ResourceCollector;
use self::scheduler::Scheduler;

pub struct Daemon {
    pub config: Config,
    pub state: Arc<RwLock<DaemonState>>,
    pub resource_collector: ResourceCollector,
    pub scheduler: Scheduler,
    pub network: NetworkManager,
}

pub struct DaemonState {
    pub start_time: Instant,
    pub network_id: Option<String>,
    pub network_name: Option<String>,
    pub invite_code: Option<InviteCode>,
    pub nodes: HashMap<NodeId, NodeState>,
    pub jobs: HashMap<JobId, Job>,
    pub memory_pools: HashMap<String, luci_common::MemoryPool>,
    pub page_store: HashMap<String, HashMap<u64, Vec<u8>>>,
    pub total_jobs_completed: u64,
    pub pending_remote_results: HashMap<String, tokio::sync::oneshot::Sender<Result<luci_runtime::ExecutionResult, String>>>,
    pub job_manager: JobManager,
    pub synced_folders: Vec<SyncedFolder>,
    pub proxy_state: Option<ProxyState>,
    pub failover_state: FailoverState,
    pub metrics: Arc<MetricsCollector>,
    pub auth_manager: Option<luci_common::auth::AuthManager>,
    pub peer_auth: HashMap<String, luci_common::auth::PeerAuth>,
    pub audit_logger: audit::AuditLogger,
    pub shutdown_notify: Arc<tokio::sync::Notify>,
}

#[derive(Clone, Debug)]
pub struct SyncedFolder {
    pub source_path: String,
    pub target_node: String,
    pub files_synced: u64,
    pub last_sync: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug)]
pub struct ProxyState {
    pub listen_port: u16,
    pub targets: Vec<String>,
    pub running: bool,
    pub total_requests: u64,
    pub total_errors: u64,
}

#[derive(Clone, Debug)]
pub struct FailoverState {
    pub enabled: bool,
    pub check_interval_secs: u64,
    pub failed_jobs: u64,
    pub restarted_jobs: u64,
    pub dead_nodes: u64,
    pub recent_events: Vec<FailoverEvent>,
}

#[derive(Clone, Debug)]
pub struct FailoverEvent {
    pub time: chrono::DateTime<chrono::Utc>,
    pub event_type: String,
    pub details: String,
}

pub enum StateChangeType {
    JobUpdate,
    ResourceUpdate,
}

impl DaemonState {
    fn new(node_id: &str, max_concurrent: usize, data_dir: &std::path::Path) -> Self {
        let metrics = Arc::new(MetricsCollector::new());
        
        // Initialize job persistence
        let persistence = match luci_runtime::JobPersistence::new(data_dir) {
            Ok(p) => {
                tracing::info!("Job persistence initialized");
                Some(Arc::new(luci_runtime::JobCache::new(p)))
            }
            Err(e) => {
                tracing::warn!("Failed to initialize job persistence: {}", e);
                None
            }
        };

        let mut job_manager = JobManager::new(node_id, max_concurrent, metrics.clone());
        if let Some(cache) = persistence {
            job_manager = job_manager.with_persistence((*cache).clone());
        }

        Self {
            start_time: Instant::now(),
            network_id: None,
            network_name: None,
            invite_code: None,
            nodes: HashMap::new(),
            jobs: HashMap::new(),
            memory_pools: HashMap::new(),
            page_store: HashMap::new(),
            total_jobs_completed: 0,
            pending_remote_results: HashMap::new(),
            job_manager,
            synced_folders: Vec::new(),
            proxy_state: None,
            failover_state: FailoverState {
                enabled: true,
                check_interval_secs: 10,
                failed_jobs: 0,
                restarted_jobs: 0,
                dead_nodes: 0,
                recent_events: Vec::new(),
            },
            metrics,
            auth_manager: None,
            peer_auth: HashMap::new(),
            audit_logger: audit::AuditLogger::new(node_id.to_string(), 10000),
            shutdown_notify: Arc::new(tokio::sync::Notify::new()),
        }
    }

    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    pub fn connected_nodes(&self) -> usize {
        self.nodes
            .values()
            .filter(|n| matches!(n.status, NodeStatus::Online | NodeStatus::Idle | NodeStatus::Busy))
            .count()
    }

    pub fn active_jobs(&self) -> usize {
        self.jobs
            .values()
            .filter(|j| {
                matches!(
                    j.status,
                    luci_common::JobStatus::Running
                        | luci_common::JobStatus::Scheduled
                        | luci_common::JobStatus::Transferring
                )
            })
            .count()
    }
}

impl Daemon {
    pub async fn new(config: Config) -> Result<Self> {
        let state = Arc::new(RwLock::new(DaemonState::new(&config.node_id.to_string(), config.max_jobs, &config.data_dir)));
        let resource_collector = ResourceCollector::new(config.node_id.clone());
        let scheduler = Scheduler::new();

        // Initialize TLS if enabled
        let network = if config.tls_enabled {
            let tls_config = config.tls_config();
            tls_config.ensure_certs().context("Failed to initialize TLS certificates")?;
            
            let server_config = tls_config.load_server_config()
                .context("Failed to load TLS server config")?;
            let client_config = tls_config.load_client_config()
                .context("Failed to load TLS client config")?;
            
            tracing::info!("TLS enabled for node-to-node communication");
            NetworkManager::with_tls(config.node_port, server_config, client_config)
        } else {
            tracing::warn!("TLS disabled - node-to-node traffic is unencrypted");
            NetworkManager::new(config.node_port)
        };

        Ok(Self {
            config,
            state,
            resource_collector,
            scheduler,
            network,
        })
    }

    pub async fn run(self) -> Result<()> {
        let daemon = Arc::new(self);
        let state = daemon.state.clone();
        let config = daemon.config.clone();
        let net_peers = daemon.network.peers.clone();

        // Add self to nodes
        {
            let mut state_guard = state.write().await;
            let node_id = config.node_id.clone();
            state_guard.nodes.insert(
                node_id.clone(),
                NodeState {
                    info: luci_common::NodeInfo {
                        node_id: node_id.clone(),
                        name: config.node_name.clone(),
                        hostname: hostname::get()
                            .map(|h| h.to_string_lossy().to_string())
                            .unwrap_or_else(|_| "unknown".to_string()),
                        os: std::env::consts::OS.to_string(),
                        arch: std::env::consts::ARCH.to_string(),
                        ip_address: get_local_ip(),
                        mac_address: get_local_mac(),
                        created_at: chrono::Utc::now(),
                    },
                    status: NodeStatus::Online,
                    resources: None,
                    last_heartbeat: chrono::Utc::now(),
                },
            );
        }

        // Start IPC server
        let ipc_state = state.clone();
        let ipc_config = config.clone();
        let ipc_daemon = daemon.clone();
        let ipc_handle = tokio::spawn(async move {
            let mut server = IpcServer::new(ipc_config.daemon_port, ipc_state, ipc_config, ipc_daemon);
            if let Err(e) = server.run().await {
                tracing::error!("IPC server error: {}", e);
            }
        });

        // Start resource collector
        let resource_state = state.clone();
        let resource_config = config.clone();
        let resource_collector = daemon.resource_collector.clone();
        let resource_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                let snapshot = resource_collector.collect().await;

                let mut state = resource_state.write().await;
                let node_id = resource_config.node_id.clone();

                if let Some(node_state) = state.nodes.get_mut(&node_id) {
                    node_state.resources = Some(snapshot);
                    node_state.last_heartbeat = chrono::Utc::now();
                }
            }
        });

        // Start discovery service
        let discovery_config = config.clone();
        let discovery_state = state.clone();
        let discovery_peers = daemon.network.peers.clone();
        let discovery_handle = tokio::spawn(async move {
            let discovery = match DiscoveryService::new(
                discovery_config.node_id.to_string(),
                discovery_config.node_name.clone(),
                discovery_config.node_port,
                discovery_config.discovery_port,
            ).await {
                Ok(d) => Arc::new(d),
                Err(e) => {
                    tracing::error!("Failed to start discovery: {}", e);
                    return;
                }
            };

            // Start listening for peers
            let discovery_clone = discovery.clone();
            let listen_state = discovery_state.clone();
            let listen_peers = discovery_peers.clone();
            tokio::spawn(async move {
                if let Err(e) = discovery_clone.listen_for_peers().await {
                    tracing::error!("Discovery listen error: {}", e);
                }
            });

            // Broadcast presence and connect to peers periodically
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;

                // Broadcast our presence
                if let Err(e) = discovery.broadcast_presence().await {
                    tracing::debug!("Broadcast error: {}", e);
                }

                // Connect to newly discovered peers
                let peers = discovery.get_known_peers().await;
                let mut state = listen_state.write().await;

                for peer in &peers {
                    // Skip if already connected or if it's ourselves
                    if peer.node_id == discovery_config.node_id.to_string() {
                        continue;
                    }

                    if state.nodes.contains_key(&NodeId::from(peer.node_id.as_str())) {
                        // Already known, update last_heartbeat
                        if let Some(node) = state.nodes.get_mut(&NodeId::from(peer.node_id.as_str())) {
                            node.last_heartbeat = chrono::Utc::now();
                            node.status = NodeStatus::Online;
                        }
                        continue;
                    }

                    // New peer - clone data before spawning
                    let peer_ip = peer.ip_addresses.first().cloned().unwrap_or_default();
                    let peer_id = peer.node_id.clone();
                    let peer_name = peer.node_name.clone();
                    let peer_hostname = peer.hostname.clone();
                    let peer_os = peer.os.clone();
                    let peer_arch = peer.arch.clone();
                    let addr_str = format!("{}:{}", peer_ip, peer.tcp_port);

                    // Add to nodes immediately
                    state.nodes.insert(
                        NodeId::from(peer_id.as_str()),
                        NodeState {
                            info: luci_common::NodeInfo {
                                node_id: NodeId::from(peer_id.as_str()),
                                name: peer_name.clone(),
                                hostname: peer_hostname.clone(),
                                os: peer_os.clone(),
                                arch: peer_arch.clone(),
                                ip_address: Some(peer_ip.clone()),
                        mac_address: None,
                                created_at: chrono::Utc::now(),
                            },
                            status: NodeStatus::Online,
                            resources: None,
                            last_heartbeat: chrono::Utc::now(),
                        },
                    );

                    tracing::info!("New peer discovered: {} ({}) at {}", peer_name, peer_id, addr_str);

                    // Spawn connection handler
                    let peer_peers = listen_peers.clone();
                    let peer_state = listen_state.clone();
                    tokio::spawn(async move {
                        let addr: SocketAddr = match addr_str.parse() {
                            Ok(a) => a,
                            Err(_) => {
                                tracing::error!("Invalid address: {}", addr_str);
                                return;
                            }
                        };
                        match TcpStream::connect(addr).await {
                            Ok(stream) => {
                                NetworkManager::handle_connection(
                                    stream,
                                    addr,
                                    peer_id.clone(),
                                    peer_peers,
                                    peer_state,
                                    None, // No TLS for LAN discovery
                                ).await;
                            }
                            Err(e) => {
                                tracing::debug!("Failed to connect to {}: {}", addr, e);
                            }
                        }
                    });
                }

                // Check for stale peers (no heartbeat for 60 seconds)
                let now = chrono::Utc::now();
                let stale_nodes: Vec<NodeId> = state.nodes.iter()
                    .filter(|(id, node)| {
                        **id != discovery_config.node_id
                            && now.signed_duration_since(node.last_heartbeat).num_seconds() > 60
                    })
                    .map(|(id, _)| id.clone())
                    .collect();

                for node_id in stale_nodes {
                    tracing::info!("Node {} timed out, marking offline", node_id);
                    if let Some(node) = state.nodes.get_mut(&node_id) {
                        node.status = NodeStatus::Offline;
                    }
                }
            }
        });

        // Start cross-network discovery (optional relay-based)
        let cross_config = config.clone();
        let cross_state = state.clone();
        let _cross_handle = tokio::spawn(async move {
            let discovery = cross_network::CrossNetworkDiscovery::new(
                cross_config.node_id.to_string(),
                cross_config.node_name.clone(),
                cross_config.network_id.clone(),
                Vec::new(), // relay_addresses - will be configured later
                Vec::new(), // direct_addresses - will be configured later
                cross_config.node_public_key.clone(),
            );

            // Register with relay if configured
            if let Err(e) = discovery.register_with_relay().await {
                tracing::debug!("Relay registration skipped: {}", e);
            }

            // Periodic sync
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                if let Err(e) = discovery.sync_peers().await {
                    tracing::debug!("Cross-network sync failed: {}", e);
                }

                // Merge cross-network peers into state
                let peers = discovery.list_peers().await;
                let mut state = cross_state.write().await;
                for peer in peers {
                    if peer.node_id == cross_config.node_id.to_string() {
                        continue;
                    }
                    let node_id = NodeId::from(peer.node_id.as_str());
                    if let std::collections::hash_map::Entry::Vacant(e) = state.nodes.entry(node_id) {
                        let nid = e.key().clone();
                        e.insert(NodeState {
                            info: luci_common::NodeInfo {
                                node_id: nid,
                                name: peer.node_name,
                                hostname: peer.node_id.clone(),
                                os: peer.capabilities.os,
                                arch: peer.capabilities.arch,
                                ip_address: peer.direct_addresses.first().cloned(),
                        mac_address: None,
                                created_at: chrono::Utc::now(),
                            },
                            status: NodeStatus::Online,
                            resources: None,
                            last_heartbeat: chrono::Utc::now(),
                        });
                    }
                }
            }
        });

        // Start auto-failover monitor
        let failover_state = state.clone();
        let failover_config = config.clone();
        let _failover_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;

                let mut state = failover_state.write().await;
                if !state.failover_state.enabled {
                    continue;
                }

                // Check for dead nodes and restart their jobs
                let dead_nodes: Vec<NodeId> = state.nodes.iter()
                    .filter(|(id, node)| {
                        **id != failover_config.node_id
                            && matches!(node.status, NodeStatus::Offline)
                    })
                    .map(|(id, _)| id.clone())
                    .collect();

                for dead_node in &dead_nodes {
                    // Find jobs assigned to dead node
                    let failed_jobs: Vec<JobId> = state.jobs.iter()
                        .filter(|(_, j)| {
                            j.assigned_node.as_ref() == Some(dead_node)
                                && matches!(j.status, luci_common::JobStatus::Running | luci_common::JobStatus::Scheduled)
                        })
                        .map(|(id, _)| id.clone())
                        .collect();

                    for job_id in failed_jobs {
                        if let Some(job) = state.jobs.get_mut(&job_id) {
                            tracing::warn!("Failover: Job {} was on dead node {}, restarting", job_id, dead_node);
                            job.status = luci_common::JobStatus::Scheduled;
                            job.assigned_node = None;
                            state.failover_state.failed_jobs += 1;

                            // Add failover event
                            state.failover_state.recent_events.push(FailoverEvent {
                                time: chrono::Utc::now(),
                                event_type: "job_restarted".to_string(),
                                details: format!("Job {} restarted (was on dead node {})", job_id, dead_node),
                            });

                            // Keep only last 20 events
                            if state.failover_state.recent_events.len() > 20 {
                                state.failover_state.recent_events.remove(0);
                            }
                        }
                    }

                    state.failover_state.dead_nodes += 1;
                    state.failover_state.recent_events.push(FailoverEvent {
                        time: chrono::Utc::now(),
                        event_type: "node_down".to_string(),
                        details: format!("Node {} went offline", dead_node),
                    });

                    if state.failover_state.recent_events.len() > 20 {
                        state.failover_state.recent_events.remove(0);
                    }
                }
            }
        });

        // Start heartbeat sender
        let heartbeat_state = state.clone();
        let heartbeat_config = config.clone();
        let heartbeat_peers = net_peers.clone();
        let heartbeat_network = daemon.network.clone();
        let heartbeat_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;

                let state = heartbeat_state.read().await;
                let local_resources = state.nodes.get(&heartbeat_config.node_id)
                    .and_then(|n| n.resources.clone());
                drop(state);

                if let Some(resources) = local_resources {
                    for peer in heartbeat_peers.iter() {
                        let addr = format!("{}:{}", peer.addr.ip(), peer.addr.port());
                        if let Err(e) = heartbeat_network.send_heartbeat(
                            &addr,
                            &heartbeat_config.node_id.to_string(),
                            resources.clone(),
                            "online",
                        ).await {
                            tracing::debug!("Failed to send heartbeat to {}: {}", peer.key(), e);
                        }
                    }
                }
            }
        });

        // Start network listener
        let net_state = state.clone();
        let net_config = config.clone();
        let net_listener = daemon.network.start_listener().await.map_err(|e| {
            anyhow::anyhow!(
                "Failed to bind to port {}: {}. Is another lucid instance running?",
                config.node_port,
                e
            )
        })?;
        let net_peers = daemon.network.peers.clone();
        let peers_for_shutdown = daemon.network.peers.clone();
        let tls_acceptor = daemon.network.tls_acceptor();
        let network_handle = tokio::spawn(async move {
            let node_id = net_config.node_id.clone();
            loop {
                match net_listener.accept().await {
                    Ok((stream, addr)) => {
                        let nid = node_id.clone();
                        let peers = net_peers.clone();
                        let state_clone = net_state.clone();
                        let acceptor = tls_acceptor.clone();
                        tokio::spawn(async move {
                            NetworkManager::handle_connection(
                                stream,
                                addr,
                                nid.to_string(),
                                peers,
                                state_clone,
                                acceptor,
                            ).await;
                        });
                    }
                    Err(e) => {
                        tracing::error!("Network accept error: {}", e);
                    }
                }
            }
        });

        tracing::info!("Daemon started on node {}", config.node_id);
        tracing::info!("  IPC port: {}", config.daemon_port);
        tracing::info!("  Node port: {}", config.node_port);
        tracing::info!("  Discovery port: {}", config.discovery_port);

        // Write PID file
        let pid_path = config.data_dir.join("lucid.pid");
        if let Some(parent) = pid_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("Failed to create PID directory {}: {}", parent.display(), e);
            }
        }
        if let Err(e) = std::fs::write(&pid_path, std::process::id().to_string()) {
            tracing::warn!("Failed to write PID file: {}", e);
        }

        // Wait for shutdown signal
        let shutdown_notify = { daemon.state.read().await.shutdown_notify.clone() };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = shutdown_notify.notified() => {},
        }

        tracing::info!("Shutting down daemon...");

        // Cleanup PID file
        if let Err(e) = std::fs::remove_file(&pid_path) {
            tracing::debug!("Failed to remove PID file: {}", e);
        }

        // Send Goodbye messages to all connected peers
        {
            let peer_count = peers_for_shutdown.len();
            tracing::info!("Sending goodbye to {} peers...", peer_count);

            for peer in peers_for_shutdown.iter() {
                let addr = format!("{}:{}", peer.addr.ip(), peer.addr.port());
                let goodbye = ProtocolMessage::new(
                    &config.node_id.to_string(),
                    MessageType::Goodbye,
                    Payload::Goodbye {
                        reason: "Daemon shutting down".to_string(),
                    },
                );
                if let Err(e) = daemon.network.send_message_to_peer(&addr, &goodbye).await {
                    tracing::debug!("Failed to send goodbye to {}: {}", peer.key(), e);
                }
            }
        }

        // Cancel all running jobs
        {
            let mut state_guard = state.write().await;
            let running_jobs: Vec<JobId> = state_guard.jobs.iter()
                .filter(|(_, j)| matches!(
                    j.status,
                    luci_common::JobStatus::Running
                        | luci_common::JobStatus::Scheduled
                        | luci_common::JobStatus::Transferring
                ))
                .map(|(id, _)| id.clone())
                .collect();

            if !running_jobs.is_empty() {
                tracing::info!("Cancelling {} running jobs...", running_jobs.len());
            }

            for job_id in running_jobs {
                if let Some(job) = state_guard.jobs.get_mut(&job_id) {
                    job.status = luci_common::JobStatus::Cancelled;
                    job.error = Some("Daemon shutting down".to_string());
                }
                if let Err(e) = state_guard.job_manager.cancel_job(&job_id).await {
                    tracing::debug!("Failed to cancel job {}: {}", job_id, e);
                }
            }
        }

        // Abort all task handles
        ipc_handle.abort();
        resource_handle.abort();
        discovery_handle.abort();
        heartbeat_handle.abort();
        network_handle.abort();

        Ok(())
    }

    pub async fn get_status(&self) -> DaemonStatus {
        let state = self.state.read().await;
        DaemonStatus {
            running: true,
            pid: Some(std::process::id()),
            uptime_secs: state.uptime_secs(),
            node_id: self.config.node_id.clone(),
            network_id: state.network_id.clone(),
            connected_nodes: state.connected_nodes(),
            active_jobs: state.active_jobs(),
            total_jobs_completed: state.total_jobs_completed,
        }
    }

    pub async fn create_network(&self, name: &str) -> (String, InviteCode) {
        let mut state = self.state.write().await;
        let network_id = luci_common::NetworkId::new();
        let invite_code = InviteCode::new();

        state.network_id = Some(network_id.to_string());
        state.network_name = Some(name.to_string());
        state.invite_code = Some(invite_code.clone());

        let node_id = self.config.node_id.clone();
        state.nodes.insert(
            node_id.clone(),
            NodeState {
                info: luci_common::NodeInfo {
                    node_id,
                    name: self.config.node_name.clone(),
                    hostname: hostname::get()
                        .map(|h| h.to_string_lossy().to_string())
                        .unwrap_or_else(|_| "unknown".to_string()),
                    os: std::env::consts::OS.to_string(),
                    arch: std::env::consts::ARCH.to_string(),
                    ip_address: None,
                        mac_address: None,
                    created_at: chrono::Utc::now(),
                },
                status: NodeStatus::Online,
                resources: None,
                last_heartbeat: chrono::Utc::now(),
            },
        );

        let mut config = self.config.clone();
        config.network_id = Some(network_id.to_string());
        config.network_name = Some(name.to_string());
        if let Err(e) = config.save() {
            tracing::warn!("Failed to save config after creating network: {}", e);
        }

        (network_id.to_string(), invite_code)
    }

    pub async fn join_network(&self, invite_code: &str) -> Result<(), String> {
        // Parse the invite code (format: "XXXX.XXXX.XXX")
        let parsed_code = InviteCode::decode(invite_code)
            .map_err(|e| format!("Invalid invite code: {}", e))?;
        
        // Generate a deterministic network_id from the invite code
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        invite_code.hash(&mut hasher);
        let network_id = format!("{:016x}", hasher.finish());
        
        let mut state = self.state.write().await;
        state.network_id = Some(network_id.clone());
        state.network_name = Some(format!("network-{}", &network_id[..8]));
        state.invite_code = Some(parsed_code);
        
        // Update config with new network settings
        let mut config = self.config.clone();
        config.network_id = Some(network_id.clone());
        config.network_name = Some(format!("network-{}", &network_id[..8]));
        if let Err(e) = config.save() {
            tracing::warn!("Failed to save config after joining network: {}", e);
        }
        
        tracing::info!("Joined network {}", network_id);
        Ok(())
    }

    pub async fn leave_network(&self, name: Option<&str>) -> Result<(), String> {
        tracing::info!("Leaving network{}...", name.map(|n| format!(" '{}'", n)).unwrap_or_default());
        
        // Send LeaveNotification to all connected peers
        {
            for peer in self.network.peers.iter() {
                let addr = format!("{}:{}", peer.addr.ip(), peer.addr.port());
                let leave_msg = ProtocolMessage::new(
                    &self.config.node_id.to_string(),
                    MessageType::LeaveNotification,
                    Payload::LeaveNotification {
                        node_id: self.config.node_id.to_string(),
                        reason: "Node leaving network".to_string(),
                    },
                );
                if let Err(e) = self.network.send_message_to_peer(&addr, &leave_msg).await {
                    tracing::debug!("Failed to send leave notification to {}: {}", peer.key(), e);
                }
            }
        }
        
        // Clear network state
        {
            let mut state = self.state.write().await;
            state.network_id = None;
            state.network_name = None;
            state.invite_code = None;
        }
        
        // Update config
        let mut config = self.config.clone();
        config.network_id = None;
        config.network_name = None;
        if let Err(e) = config.save() {
            tracing::warn!("Failed to save config after leaving network: {}", e);
        }
        
        tracing::info!("Left network successfully");
        Ok(())
    }

    pub async fn get_devices(&self) -> Vec<NodeState> {
        let state = self.state.read().await;
        state.nodes.values().cloned().collect()
    }

    pub async fn get_device(&self, name: &str) -> Option<NodeState> {
        let state = self.state.read().await;
        state
            .nodes
            .values()
            .find(|n| n.info.name == name || n.info.node_id.as_str() == name)
            .cloned()
    }

    pub async fn get_resources(&self) -> (usize, usize, f64, f64, usize, f64) {
        let state = self.state.read().await;
        let mut total_cpu = 0;
        let mut total_ram = 0.0;
        let mut available_ram = 0.0;
        let mut gpu_count = 0;
        let mut total_vram = 0.0;

        for node in state.nodes.values() {
            if let Some(resources) = &node.resources {
                total_cpu += resources.cpu.cores;
                total_ram += resources.memory.total_gb();
                available_ram += resources.memory.available_gb();
                if let Some(gpu) = &resources.gpu {
                    gpu_count += 1;
                    total_vram += gpu.vram_total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                }
            }
        }

        (state.nodes.len(), total_cpu, total_ram, available_ram, gpu_count, total_vram)
    }

    pub async fn submit_job(&self, mut job: Job) -> JobId {
        let job_id = job.id.clone();
        let mut state = self.state.write().await;

        // Find best node
        let best_node = self.scheduler.find_best_node(&state, &job);

        if let Some(node_id) = best_node {
            job.assigned_node = Some(node_id.clone());
            job.status = luci_common::JobStatus::Scheduled;

            state.jobs.insert(job_id.clone(), job.clone());

            // If assigned to local node, execute immediately
            if node_id == self.config.node_id {
            let working_dir = job.working_dir.as_ref()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| self.config.data_dir().to_path_buf());
                if let Err(e) = state.job_manager.submit_job(job, &working_dir).await {
                    tracing::error!("Failed to submit job {}: {}", job_id, e);
                }
            } else {
                // Remote node - auto-sync files then send job
                tracing::info!("Sending job {} to remote node {}", job_id, node_id);

                // Find peer address
                let peer_addr = state.nodes.get(&node_id)
                    .and_then(|n| n.info.ip_address.as_ref())
                    .map(|ip| format!("{}:{}", ip, self.config.node_port));

                if let Some(addr) = peer_addr {
                    let addr_clone = addr.clone();
                    let job_clone = job.clone();
                    let node_id_clone = self.config.node_id.to_string();
                    let network = self.network.clone();
                    let state_ref = self.state.clone();
                    drop(state);

                    // AUTO-SYNC: If job has a script path, sync the file to remote node
                    if let Some(ref script_path) = job_clone.script_path {
                        let source_path = std::path::Path::new(script_path);
                        if source_path.exists() {
                            tracing::info!("Auto-syncing {} to node {}", script_path, node_id_clone);
                            // Create temp working dir on remote node
                            let remote_work_dir = format!("/tmp/luci-jobs/{}", job_clone.id);
                            let _ = self.sync_files(
                                source_path.parent().unwrap_or(source_path).to_str().unwrap_or("."),
                                &addr_clone,
                                &remote_work_dir,
                            ).await;
                        }
                    }

                    match network.send_job_to_peer(&addr_clone, &node_id_clone, &job_clone).await {
                        Ok(()) => {
                            tracing::info!("Job {} sent to remote node {}", job_id, node_id);
                        }
                        Err(e) => {
                            tracing::error!("Failed to send job to remote: {}", e);
                            let mut state = state_ref.write().await;
                            if let Some(j) = state.jobs.get_mut(&job_id) {
                                j.status = luci_common::JobStatus::Failed;
                                j.error = Some(format!("Failed to send to remote: {}", e));
                            }
                        }
                    }
                    return job_id;
                } else {
                    tracing::error!("No address found for node {}", node_id);
                    if let Some(j) = state.jobs.get_mut(&job_id) {
                        j.status = luci_common::JobStatus::Failed;
                        j.error = Some(format!("No address for node {}", node_id));
                    }
                }
            }
        } else {
            // No node available, assign to self
            job.assigned_node = Some(self.config.node_id.clone());
            job.status = luci_common::JobStatus::Scheduled;
            state.jobs.insert(job_id.clone(), job.clone());

            let working_dir = job.working_dir.as_ref()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| self.config.data_dir().to_path_buf());
            if let Err(e) = state.job_manager.submit_job(job, &working_dir).await {
                tracing::error!("Failed to submit job {}: {}", job_id, e);
            }
        }

        job_id
    }

    pub async fn get_job(&self, id: &str) -> Option<Job> {
        let job_id = JobId::from(id);
        let state = self.state.read().await;
        // Prefer job_manager version (has live status)
        if let Some(job) = state.job_manager.get_job(&job_id).await {
            return Some(job);
        }
        state.jobs.values().find(|j| j.id.as_str() == id).cloned()
    }

    pub async fn list_jobs(&self) -> Vec<Job> {
        let state = self.state.read().await;
        let mut jobs: Vec<Job> = state.jobs.values().cloned().collect();

        let manager_jobs = state.job_manager.list_jobs().await;
        for mj in manager_jobs {
            if !jobs.iter().any(|j| j.id == mj.id) {
                jobs.push(mj);
            } else {
                // Update with live status from job_manager
                if let Some(existing) = jobs.iter_mut().find(|j| j.id == mj.id) {
                    *existing = mj;
                }
            }
        }
        jobs
    }

    pub async fn cancel_job(&self, id: &str) -> Result<(), String> {
        let job_id = JobId::from(id);
        let mut state = self.state.write().await;

        if let Err(e) = state.job_manager.cancel_job(&job_id).await {
            tracing::warn!("Failed to cancel job in manager: {}", e);
        }

        if let Some(job) = state.jobs.get_mut(&job_id) {
            job.status = luci_common::JobStatus::Cancelled;
            Ok(())
        } else {
            Err(format!("Job not found: {}", id))
        }
    }

    pub async fn create_memory_pool(&self, name: &str, size_gb: f64) -> luci_common::MemoryPool {
        let mut state = self.state.write().await;

        let page_size = 4096u32;
        let total_pages = ((size_gb * 1024.0 * 1024.0 * 1024.0) / page_size as f64) as u64;

        let pool = luci_common::MemoryPool {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            total_pages,
            page_size_bytes: page_size,
            owner_nodes: vec![self.config.node_id.clone()],
            created_at: chrono::Utc::now(),
        };

        state.memory_pools.insert(pool.id.clone(), pool.clone());
        pool
    }

    pub async fn list_memory_pools(&self) -> Vec<luci_common::MemoryPool> {
        let state = self.state.read().await;
        state.memory_pools.values().cloned().collect()
    }

    pub async fn get_memory_pool(&self, id: &str) -> Option<luci_common::MemoryPool> {
        let state = self.state.read().await;
        state.memory_pools.get(id).cloned()
    }

    pub async fn delete_memory_pool(&self, id: &str) -> Result<(), String> {
        let mut state = self.state.write().await;
        if state.memory_pools.remove(id).is_some() {
            Ok(())
        } else {
            Err(format!("Memory pool not found: {}", id))
        }
    }

    /// Sync files from source path to target node's target path
    pub async fn sync_files(
        &self,
        source_path: &str,
        target_addr: &str,
        target_path: &str,
    ) -> Result<(), String> {
        let source = std::path::Path::new(source_path);
        if !source.exists() {
            return Err(format!("Source path does not exist: {}", source_path));
        }

        // Build manifest of local files
        let files = file_sync::FileTransferManager::build_manifest(source).await
            .map_err(|e| format!("Failed to build manifest: {}", e))?;

        if files.is_empty() {
            tracing::info!("No files to sync from {}", source_path);
            return Ok(());
        }

        tracing::info!("Syncing {} files from {} to {}:{}", files.len(), source_path, target_addr, target_path);

        // Send all files directly via FileSyncData message
        let mut file_entries = Vec::new();
        for entry in &files {
            let full_path = source.join(&entry.relative_path);
            match tokio::fs::read(&full_path).await {
                Ok(data) => {
                    file_entries.push(luci_protocol::FileSyncEntry {
                        path: entry.relative_path.clone(),
                        data,
                        size: entry.size,
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to read file {}: {}", full_path.display(), e);
                }
            }
        }

        if file_entries.is_empty() {
            return Ok(());
        }

        let message = luci_protocol::ProtocolMessage::new(
            &self.config.node_id.to_string(),
            luci_protocol::MessageType::FileSyncData,
            luci_protocol::Payload::FileSyncData {
                source_path: source_path.to_string(),
                files: file_entries,
            },
        );

        self.network.send_message_to_peer(target_addr, &message).await
            .map_err(|e| format!("Failed to send files: {}", e))?;

        tracing::info!("File sync complete");
        Ok(())
    }

    /// Write data to a remote memory page on another node
    pub async fn write_remote_memory_page(
        &self,
        pool_id: &str,
        page_id: u64,
        data: Vec<u8>,
        target_addr: &str,
    ) -> Result<(), String> {
        let message = luci_protocol::ProtocolMessage::new(
            &self.config.node_id.to_string(),
            luci_protocol::MessageType::MemoryPageWrite,
            luci_protocol::Payload::MemoryPageWrite {
                page_id,
                pool_id: pool_id.to_string(),
                data,
                compressed: false,
                encrypted: false,
            },
        );

        self.network.send_message_to_peer(target_addr, &message).await
            .map_err(|e| format!("Failed to send memory page: {}", e))?;
        Ok(())
    }

    /// Read a remote memory page from another node
    pub async fn read_remote_memory_page(
        &self,
        pool_id: &str,
        page_id: u64,
        target_addr: &str,
    ) -> Result<Vec<u8>, String> {
        let message = luci_protocol::ProtocolMessage::new(
            &self.config.node_id.to_string(),
            luci_protocol::MessageType::MemoryPageRequest,
            luci_protocol::Payload::MemoryPageRequest {
                page_id,
                pool_id: pool_id.to_string(),
            },
        );

        self.network.send_message_to_peer(target_addr, &message).await
            .map_err(|e| format!("Failed to request memory page: {}", e))?;

        // Wait a bit for the response to be stored
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Read from page store (response handler stores it)
        let state = self.state.read().await;
        Ok(state.page_store.get(pool_id)
            .and_then(|pages| pages.get(&page_id))
            .cloned()
            .unwrap_or_default())
    }

    /// Distribute memory pages across available nodes (round-robin)
    pub async fn distribute_memory_pages(
        &self,
        pool_id: &str,
        data: &[u8],
        page_size: usize,
    ) -> Result<(), String> {
        let node_addresses: Vec<(String, String)> = {
            let state = self.state.read().await;
            state.nodes.iter()
                .filter(|(_, n)| n.status == luci_common::NodeStatus::Online)
                .filter_map(|(id, n)| n.info.ip_address.as_ref().map(|ip| (id.to_string(), format!("{}:{}", ip, self.config.node_port))))
                .collect()
        };

        if node_addresses.is_empty() {
            return Err("No nodes available for memory distribution".to_string());
        }

        let total_pages = data.len().div_ceil(page_size);
        let node_id = self.config.node_id.to_string();
        let network = self.network.clone();

        for page_idx in 0..total_pages {
            let node_idx = page_idx % node_addresses.len();
            let (target_node_id, addr) = &node_addresses[node_idx];
            let start = page_idx * page_size;
            let end = std::cmp::min(start + page_size, data.len());
            let page_data = data[start..end].to_vec();

            let message = luci_protocol::ProtocolMessage::new(
                &node_id,
                luci_protocol::MessageType::MemoryPageWrite,
                luci_protocol::Payload::MemoryPageWrite {
                    page_id: page_idx as u64,
                    pool_id: pool_id.to_string(),
                    data: page_data,
                    compressed: false,
                    encrypted: false,
                },
            );

            if let Err(e) = network.send_message_to_peer(addr, &message).await {
                tracing::warn!("Failed to send page {} to {}: {}", page_idx, target_node_id, e);
            }
        }

        Ok(())
    }

    /// Aggregate GPU VRAM from all nodes into unified pool
    pub async fn get_total_gpu_vram(&self) -> u64 {
        let state = self.state.read().await;
        state.nodes.values()
            .filter(|n| n.status == luci_common::NodeStatus::Online)
            .filter_map(|n| n.resources.as_ref())
            .filter_map(|r| r.gpu.as_ref())
            .map(|g| g.vram_total_bytes)
            .sum()
    }

    /// Get list of all GPUs across nodes
    pub async fn get_all_gpus(&self) -> Vec<(String, luci_common::types::GpuInfo)> {
        let state = self.state.read().await;
        state.nodes.iter()
            .filter(|(_, n)| n.status == luci_common::NodeStatus::Online)
            .filter_map(|(id, n)| n.resources.as_ref().and_then(|r| r.gpu.as_ref().map(|g| (id.to_string(), g.clone()))))
            .collect()
    }

    /// Send job to remote node for execution and wait for result
    pub async fn execute_remote_job(
        &self,
        job: Job,
        target_addr: &str,
    ) -> Result<String, String> {
        let job_id = job.id.clone();

        // Send job to remote node
        self.network.send_job_to_peer(target_addr, &self.config.node_id.to_string(), &job).await
            .map_err(|e| format!("Failed to send job to remote: {}", e))?;

        // Store in local state
        {
            let mut state = self.state.write().await;
            state.jobs.insert(job_id.clone(), job);
        }

        // Broadcast job state change to all nodes
        self.broadcast_state_change(StateChangeType::JobUpdate).await;

        Ok(job_id.to_string())
    }

    /// Broadcast state changes to all connected nodes
    pub async fn broadcast_state_change(&self, change_type: StateChangeType) {
        let state = self.state.read().await;
        let nodes: Vec<_> = state.nodes.iter()
            .filter(|(_, n)| n.status == luci_common::NodeStatus::Online)
            .filter_map(|(id, n)| n.info.ip_address.as_ref().map(|ip| (id.to_string(), format!("{}:{}", ip, self.config.node_port))))
            .collect();

        let message = match change_type {
            StateChangeType::JobUpdate => {
                let jobs: Vec<serde_json::Value> = state.jobs.values().map(|j| {
                    serde_json::json!({
                        "id": j.id.to_string(),
                        "name": j.name,
                        "status": format!("{:?}", j.status),
                        "assigned_node": j.assigned_node.as_ref().map(|n| n.to_string()),
                    })
                }).collect();
                luci_protocol::ProtocolMessage::new(
                    &self.config.node_id.to_string(),
                    luci_protocol::MessageType::StateSync,
                    luci_protocol::Payload::StateSync {
                        state_type: "jobs".to_string(),
                        data: serde_json::to_vec(&jobs).unwrap_or_default(),
                    },
                )
            }
            StateChangeType::ResourceUpdate => {
                let snapshot = self.resource_collector.collect().await;
                let resources = serde_json::to_vec(&snapshot).unwrap_or_default();
                luci_protocol::ProtocolMessage::new(
                    &self.config.node_id.to_string(),
                    luci_protocol::MessageType::StateSync,
                    luci_protocol::Payload::StateSync {
                        state_type: "resources".to_string(),
                        data: resources,
                    },
                )
            }
        };

        for (node_id, addr) in nodes {
            if node_id != self.config.node_id.to_string() {
                if let Err(e) = self.network.send_message_to_peer(&addr, &message).await {
                    tracing::warn!("Failed to broadcast state to {}: {}", node_id, e);
                }
            }
        }
    }
}


fn get_local_ip() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("ipconfig").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let line = line.trim();
                if line.contains("IPv4 Address") || line.contains("IP Address") {
                    if let Some(ip) = line.split(':').next_back() {
                        let ip = ip.trim().trim_start_matches("(Preferred)").trim().to_string();
                        if !ip.is_empty() && ip != "127.0.0.1" {
                            return Some(ip);
                        }
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(output) = std::process::Command::new("hostname").args(["-I"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(ip) = stdout.split_whitespace().next() {
                if !ip.is_empty() && ip != "127.0.0.1" { return Some(ip.to_string()); }
            }
        }
    }
    None
}

fn get_local_mac() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("getmac").args(["/fo","csv","/nh"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let line = line.trim().trim_matches('"');
                // csv: "AA-BB-CC-DD-EE-FF","..."
                if let Some(mac) = line.split(',').next() {
                    let mac = mac.trim().trim_matches('"').to_string();
                    if mac.len() == 17 && mac != "00-00-00-00-00-00" {
                        return Some(mac);
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(output) = std::process::Command::new("cat").arg("/sys/class/net/$(ls /sys/class/net | head -1)/address").output() {
            let _ = output; // fallback below
        }
        if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name == "lo" { continue; }
                if let Ok(mac) = std::fs::read_to_string(e.path().join("address")) {
                    let mac = mac.trim().to_string();
                    if !mac.is_empty() && mac != "00:00:00:00:00:00" { return Some(mac); }
                }
            }
        }
    }
    None
}
