use anyhow::{Context, Result};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use luci_common::{Job, ResourceSnapshot};
use luci_protocol::{LengthPrefixedCodec, Payload, ProtocolMessage, MessageType, pong_message, heartbeat_message, job_submit_message};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_util::codec::Framed;

use super::DaemonState;

/// Stream type that can be either TLS or plain TCP
pub enum PeerStream {
    Tls(tokio_rustls::client::TlsStream<TcpStream>),
    Plain(TcpStream),
}

#[derive(Clone)]
pub struct NetworkManager {
    port: u16,
    pub peers: Arc<DashMap<String, PeerConnection>>,
    tls_config: Option<Arc<rustls::ServerConfig>>,
    client_tls_config: Option<Arc<rustls::ClientConfig>>,
}

pub struct PeerConnection {
    pub node_id: String,
    pub addr: SocketAddr,
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,
    pub resources: Option<ResourceSnapshot>,
}

impl NetworkManager {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            peers: Arc::new(DashMap::new()),
            tls_config: None,
            client_tls_config: None,
        }
    }

    /// Create a new NetworkManager with TLS enabled
    pub fn with_tls(port: u16, server_config: rustls::ServerConfig, client_config: rustls::ClientConfig) -> Self {
        Self {
            port,
            peers: Arc::new(DashMap::new()),
            tls_config: Some(Arc::new(server_config)),
            client_tls_config: Some(Arc::new(client_config)),
        }
    }

    pub async fn start_listener(&self) -> Result<TcpListener> {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        tracing::info!("Network listener started on {}", addr);
        Ok(listener)
    }

    pub async fn handle_connection(
        stream: TcpStream,
        addr: SocketAddr,
        node_id: String,
        peers: Arc<DashMap<String, PeerConnection>>,
        state: Arc<RwLock<DaemonState>>,
        tls_acceptor: Option<Arc<TlsAcceptor>>,
    ) {
        tracing::debug!("New connection from {}", addr);

        // Perform TLS handshake if configured
        if let Some(acceptor) = tls_acceptor {
            match acceptor.accept(stream).await {
                Ok(tls_stream) => {
                    let codec: LengthPrefixedCodec<ProtocolMessage> = LengthPrefixedCodec::new();
                    let mut framed = Framed::new(tls_stream, codec);
                    loop {
                        match framed.next().await {
                            Some(Ok(message)) => {
                                let response = Self::process_message(&message, &node_id, addr, &peers, &state).await;
                                if let Some(resp) = response {
                                    if let Err(e) = framed.send(resp).await {
                                        tracing::error!("Failed to send response to {}: {}", addr, e);
                                        break;
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                tracing::debug!("Connection error from {}: {}", addr, e);
                                break;
                            }
                            None => {
                                tracing::debug!("Connection closed from {}", addr);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("TLS handshake failed from {}: {}", addr, e);
                }
            }
        } else {
            let codec: LengthPrefixedCodec<ProtocolMessage> = LengthPrefixedCodec::new();
            let mut framed = Framed::new(stream, codec);
            loop {
                match framed.next().await {
                    Some(Ok(message)) => {
                        let response = Self::process_message(&message, &node_id, addr, &peers, &state).await;
                        if let Some(resp) = response {
                            if let Err(e) = framed.send(resp).await {
                                tracing::error!("Failed to send response to {}: {}", addr, e);
                                break;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        tracing::debug!("Connection error from {}: {}", addr, e);
                        break;
                    }
                    None => {
                        tracing::debug!("Connection closed from {}", addr);
                        break;
                    }
                }
            }
        }
    }

    async fn process_message(
        message: &ProtocolMessage,
        node_id: &str,
        addr: SocketAddr,
        peers: &Arc<DashMap<String, PeerConnection>>,
        state: &Arc<RwLock<DaemonState>>,
    ) -> Option<ProtocolMessage> {
        match &message.payload {
            Payload::Ping => {
                let hostname = hostname::get()
                    .map(|h| h.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "unknown".to_string());
                Some(pong_message(node_id, &hostname))
            }
            Payload::Pong { node_name } => {
                tracing::info!("Pong from {} ({})", message.sender_id, node_name);
                None
            }
            Payload::Heartbeat { resources, status } => {
                tracing::debug!(
                    "Heartbeat from {} (status: {}, RAM: {:.1}GB)",
                    message.sender_id,
                    status,
                    resources.memory.available_gb()
                );

                // Update peer resources
                let cluster_size = {
                    if let Some(mut peer) = peers.get_mut(&message.sender_id) {
                        peer.resources = Some(resources.clone());
                        peer.last_heartbeat = chrono::Utc::now();
                    }
                    (peers.len() + 1) as u32
                };

                // Update daemon state
                {
                    let mut state_guard = state.write().await;
                    let peer_node_id = luci_common::NodeId::from(message.sender_id.as_str());
                    if let Some(node_state) = state_guard.nodes.get_mut(&peer_node_id) {
                        node_state.resources = Some(resources.clone());
                        node_state.last_heartbeat = chrono::Utc::now();
                        node_state.status = luci_common::NodeStatus::Online;
                    }
                }

                Some(ProtocolMessage::new(
                    node_id,
                    MessageType::HeartbeatAck,
                    Payload::HeartbeatAck { cluster_size },
                ))
            }
            Payload::HeartbeatAck { cluster_size } => {
                tracing::debug!("HeartbeatAck from {} (cluster_size: {})", message.sender_id, cluster_size);
                None
            }
            Payload::Announce { node_id: remote_id, node_name, hostname: _, os, arch, capabilities } => {
                tracing::info!(
                    "Node announced: {} ({}) from {}/{} capabilities={:?}",
                    node_name, remote_id, os, arch, capabilities
                );

                // Add to network peers
                peers.entry(remote_id.clone()).or_insert_with(|| {
                    PeerConnection {
                        node_id: remote_id.clone(),
                        addr,
                        last_heartbeat: chrono::Utc::now(),
                        resources: None,
                    }
                });

                // Add to daemon state
                let remote_node_id = luci_common::NodeId::from(remote_id.as_str());
                let mut state_guard = state.write().await;
                if !state_guard.nodes.contains_key(&remote_node_id) {
                    state_guard.nodes.insert(
                        remote_node_id.clone(),
                        super::NodeState {
                            info: luci_common::NodeInfo {
                                node_id: remote_node_id,
                                name: node_name.clone(),
                                hostname: node_name.clone(),
                                os: os.clone(),
                                arch: arch.clone(),
                                ip_address: Some(addr.ip().to_string()),
                        mac_address: None,
                                created_at: chrono::Utc::now(),
                            },
                            status: luci_common::NodeStatus::Online,
                            resources: None,
                            last_heartbeat: chrono::Utc::now(),
                            connected_at: chrono::Utc::now(),
                        },
                    );
                    tracing::info!("Added new peer from Announce: {} ({})", node_name, addr);
                } else if let Some(node) = state_guard.nodes.get_mut(&remote_node_id) {
                    node.last_heartbeat = chrono::Utc::now();
                    node.status = luci_common::NodeStatus::Online;
                    node.info.ip_address = Some(addr.ip().to_string());
                }

                None
            }
            Payload::JoinRequest { invite_code, node_name, hostname: _, os, arch } => {
                tracing::info!(
                    "Join request from {} ({}) at {}/{}",
                    node_name, invite_code, os, arch
                );

                let (valid, network_id) = {
                    let state_guard = state.read().await;
                    let valid = match &state_guard.invite_code {
                        Some(expected) => expected.to_string() == *invite_code,
                        None => false,
                    };
                    let network_id = state_guard.network_id.clone()
                        .unwrap_or_else(String::new);
                    (valid, network_id)
                };

                if !valid {
                    tracing::warn!(
                        "Join request rejected: invalid invite code '{}' from {}",
                        invite_code, node_name
                    );
                    return Some(ProtocolMessage::new(
                        node_id,
                        MessageType::JoinResponse,
                        Payload::JoinResponse {
                            success: false,
                            node_id: None,
                            network_id: String::new(),
                            error: Some("Invalid invite code or no network created".to_string()),
                        },
                    ));
                }

                Some(ProtocolMessage::new(
                    node_id,
                    MessageType::JoinResponse,
                    Payload::JoinResponse {
                        success: true,
                        node_id: Some(node_id.to_string()),
                        network_id,
                        error: None,
                    },
                ))
            }
            Payload::JobSubmit { job } => {
                tracing::info!("Remote job submitted: {} ({}) from {}", job.name, job.id, message.sender_id);

                let sender_id = message.sender_id.clone();

                // Store sender address for result callback
                let _sender_addr = peers.get(&sender_id)
                    .map(|p| format!("{}:{}", p.addr.ip(), p.addr.port()));

                {
                    let mut state_guard = state.write().await;
                    // Use job's working_dir if provided, otherwise use temp dir
                    let working_dir = job.working_dir.as_ref()
                        .map(std::path::PathBuf::from)
                        .unwrap_or_else(std::env::temp_dir);

                    if let Err(e) = state_guard.job_manager.submit_job(job.clone(), &working_dir).await {
                        tracing::error!("Failed to submit remote job {}: {}", job.id, e);
                        return Some(ProtocolMessage::new(
                            node_id,
                            MessageType::JobFailed,
                            Payload::JobFailed {
                                job_id: job.id.to_string(),
                                error: format!("Failed to submit job: {}", e),
                            },
                        ));
                    }

                    state_guard.jobs.insert(job.id.clone(), job.clone());
                }

                tracing::info!("Remote job {} submitted to local executor", job.id);

                Some(ProtocolMessage::new(
                    node_id,
                    MessageType::JobAssigned,
                    Payload::JobAssigned {
                        job_id: job.id.to_string(),
                        node_id: node_id.to_string(),
                    },
                ))
            }
            Payload::JobAssigned { job_id, node_id: assigned_node } => {
                tracing::info!("Job {} assigned to {}", job_id, assigned_node);
                None
            }
            Payload::JobCompleted { job_id, result } => {
                tracing::info!("Job {} completed on remote node with result: {}", job_id, result);
                // Update local job state
                {
                    let mut state_guard = state.write().await;
                    let jid = luci_common::JobId::from(job_id.as_str());
                    if let Some(job) = state_guard.jobs.get_mut(&jid) {
                        job.status = luci_common::JobStatus::Completed;
                        job.result = Some(result.clone());
                        job.completed_at = Some(chrono::Utc::now());
                        state_guard.total_jobs_completed += 1;
                    }
                    // Also update in job_manager if present
                    let _ = state_guard.job_manager.update_job_status(&jid, luci_common::JobStatus::Completed, Some(result.clone())).await;
                }
                Some(ProtocolMessage::new(
                    node_id,
                    MessageType::JobCompleted,
                    Payload::JobCompleted {
                        job_id: job_id.clone(),
                        result: result.clone(),
                    },
                ))
            }
            Payload::JobFailed { job_id, error } => {
                tracing::error!("Job {} failed on remote node: {}", job_id, error);
                // Update local job state
                {
                    let mut state_guard = state.write().await;
                    let jid = luci_common::JobId::from(job_id.as_str());
                    if let Some(job) = state_guard.jobs.get_mut(&jid) {
                        job.status = luci_common::JobStatus::Failed;
                        job.error = Some(error.clone());
                        job.completed_at = Some(chrono::Utc::now());
                    }
                    let _ = state_guard.job_manager.update_job_status(&jid, luci_common::JobStatus::Failed, Some(error.clone())).await;
                }
                Some(ProtocolMessage::new(
                    node_id,
                    MessageType::JobFailed,
                    Payload::JobFailed {
                        job_id: job_id.clone(),
                        error: error.clone(),
                    },
                ))
            }
            Payload::FileSyncRequest { source_path, file_name, file_data, target_path } => {
                tracing::info!("Receiving file sync: {} ({} bytes)", file_name, file_data.len());

                let full_path = format!("{}/{}", target_path.trim_end_matches('/'), file_name);
                if let Some(parent) = std::path::Path::new(&full_path).parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        tracing::error!("Failed to create directory {}: {}", parent.display(), e);
                        return Some(ProtocolMessage::new(
                            node_id,
                            MessageType::FileSyncAck,
                            Payload::FileSyncAck {
                                success: false,
                                files_received: 0,
                                error: Some(format!("Failed to create directory: {}", e)),
                            },
                        ));
                    }
                }

                match tokio::fs::write(&full_path, &file_data).await {
                    Ok(()) => {
                        tracing::info!("File synced: {} -> {}", source_path, full_path);
                        Some(ProtocolMessage::new(
                            node_id,
                            MessageType::FileSyncAck,
                            Payload::FileSyncAck {
                                success: true,
                                files_received: 1,
                                error: None,
                            },
                        ))
                    }
                    Err(e) => {
                        tracing::error!("Failed to write file {}: {}", full_path, e);
                        Some(ProtocolMessage::new(
                            node_id,
                            MessageType::FileSyncAck,
                            Payload::FileSyncAck {
                                success: false,
                                files_received: 0,
                                error: Some(format!("Failed to write file: {}", e)),
                            },
                        ))
                    }
                }
            }
            Payload::FileSyncData { source_path, files } => {
                let mut received = 0u64;
                for file in files {
                    let full_path = format!("{}/{}", source_path.trim_end_matches('/'), file.path);
                    if let Some(parent) = std::path::Path::new(&full_path).parent() {
                        let _ = tokio::fs::create_dir_all(parent).await;
                    }
                    if let Err(e) = tokio::fs::write(&full_path, &file.data).await {
                        tracing::error!("Failed to write file {}: {}", full_path, e);
                    } else {
                        received += 1;
                        tracing::debug!("Received file: {} ({} bytes)", full_path, file.size);
                    }
                }
                Some(ProtocolMessage::new(
                    node_id,
                    MessageType::FileSyncAck,
                    Payload::FileSyncAck {
                        success: true,
                        files_received: received,
                        error: None,
                    },
                ))
            }
            Payload::FileSyncAck { success, files_received, error } => {
                if *success {
                    tracing::info!("File sync ack: {} files received", files_received);
                } else {
                    tracing::error!("File sync failed: {:?}", error);
                }
                None
            }
            Payload::FileManifest { transfer_id, root_path, files } => {
                tracing::info!(
                    "Received file manifest: {} files from {}",
                    files.len(),
                    root_path
                );
                
                // Compare with local files and respond with needed files
                let local_root = std::path::Path::new(&root_path);
                let needed = super::file_sync::FileTransferManager::handle_manifest(
                    transfer_id,
                    root_path,
                    files,
                    local_root,
                ).await;

                Some(ProtocolMessage::new(
                    node_id,
                    MessageType::FileManifestResponse,
                    Payload::FileManifestResponse {
                        transfer_id: transfer_id.clone(),
                        needed_files: needed,
                    },
                ))
            }
            Payload::FileManifestResponse { transfer_id, needed_files } => {
                tracing::info!(
                    "Manifest response: {} files needed for transfer {}",
                    needed_files.len(),
                    transfer_id
                );
                // This would be handled by the sender to know which files to send
                None
            }
            Payload::FileChunk { transfer_id, file_path, chunk_index, total_chunks, data, chunk_hash } => {
                let target_root = std::env::temp_dir().join("luci_sync");
                if let Err(e) = super::file_sync::FileTransferManager::new(
                    luci_common::NodeId::from(node_id)
                ).handle_chunk(
                    transfer_id,
                    file_path,
                    *chunk_index,
                    *total_chunks,
                    data,
                    chunk_hash,
                    &target_root,
                ).await {
                    tracing::error!("Failed to handle file chunk: {}", e);
                    return Some(ProtocolMessage::new(
                        node_id,
                        MessageType::FileChunkAck,
                        Payload::FileChunkAck {
                            transfer_id: transfer_id.clone(),
                            chunk_index: *chunk_index,
                            success: false,
                            error: Some(e.to_string()),
                        },
                    ));
                }

                Some(ProtocolMessage::new(
                    node_id,
                    MessageType::FileChunkAck,
                    Payload::FileChunkAck {
                        transfer_id: transfer_id.clone(),
                        chunk_index: *chunk_index,
                        success: true,
                        error: None,
                    },
                ))
            }
            Payload::FileChunkAck { transfer_id, chunk_index, success, error } => {
                if !success {
                    tracing::error!(
                        "File chunk {} ack failed for transfer {}: {:?}",
                        chunk_index,
                        transfer_id,
                        error
                    );
                }
                None
            }
            Payload::FileTransferComplete { transfer_id, total_files, total_bytes } => {
                tracing::info!(
                    "File transfer {} complete: {} files ({} bytes)",
                    transfer_id,
                    total_files,
                    total_bytes
                );
                None
            }
            Payload::MemoryPageRequest { page_id, pool_id } => {
                tracing::debug!("Memory page request: pool={}, page={}", pool_id, page_id);
                let result = {
                    let state_guard = state.read().await;
                    state_guard.page_store.get(pool_id)
                        .and_then(|pages| pages.get(page_id))
                        .cloned()
                };
                match result {
                    Some(data) => Some(ProtocolMessage::new(
                        node_id,
                        MessageType::MemoryPageResponse,
                        Payload::MemoryPageResponse {
                            page_id: *page_id,
                            pool_id: pool_id.clone(),
                            data,
                            compressed: false,
                            encrypted: false,
                        },
                    )),
                    None => Some(ProtocolMessage::new(
                        node_id,
                        MessageType::MemoryPageResponse,
                        Payload::MemoryPageResponse {
                            page_id: *page_id,
                            pool_id: pool_id.clone(),
                            data: vec![],
                            compressed: false,
                            encrypted: false,
                        },
                    )),
                }
            }
            Payload::MemoryPageResponse { page_id, pool_id, data, compressed: _, encrypted: _ } => {
                tracing::debug!("Memory page response: pool={}, page={}, {} bytes", pool_id, page_id, data.len());
                if !data.is_empty() {
                    let mut state_guard = state.write().await;
                    let pool_pages = state_guard.page_store.entry(pool_id.clone()).or_insert_with(HashMap::new);
                    pool_pages.insert(*page_id, data.clone());
                    tracing::info!("Stored memory page {} from pool {} ({} bytes)", page_id, pool_id, data.len());
                }
                None
            }
            Payload::MemoryPageWrite { page_id, pool_id, data, compressed: _, encrypted: _ } => {
                tracing::debug!("Memory page write: pool={}, page={}, {} bytes", pool_id, page_id, data.len());
                let mut state_guard = state.write().await;
                let pool_pages = state_guard.page_store.entry(pool_id.clone()).or_insert_with(HashMap::new);
                pool_pages.insert(*page_id, data.clone());
                let success = state_guard.memory_pools.contains_key(pool_id);
                tracing::info!("Stored memory page {} to pool {} ({} bytes)", page_id, pool_id, data.len());
                Some(ProtocolMessage::new(
                    node_id,
                    MessageType::MemoryPageAck,
                    Payload::MemoryPageAck {
                        page_id: *page_id,
                        pool_id: pool_id.clone(),
                        success,
                    },
                ))
            }
            Payload::MemoryPageAck { page_id, pool_id, success } => {
                tracing::debug!("Memory page ack: pool={}, page={}, success={}", pool_id, page_id, success);
                None
            }
            Payload::JobStarted { job_id } => {
                tracing::info!("Job {} started on remote node", job_id);
                {
                    let mut state_guard = state.write().await;
                    let jid = luci_common::JobId::from(job_id.as_str());
                    if let Some(job) = state_guard.jobs.get_mut(&jid) {
                        job.status = luci_common::JobStatus::Running;
                        job.started_at = Some(chrono::Utc::now());
                    }
                }
                None
            }
            Payload::JobProgress { job_id, progress_percent, message } => {
                tracing::debug!("Job {} progress: {}% - {:?}", job_id, progress_percent, message);
                None
            }
            Payload::JobCancel { job_id } => {
                tracing::info!("Cancel request for job {}", job_id);
                {
                    let mut state_guard = state.write().await;
                    let jid = luci_common::JobId::from(job_id.as_str());
                    if let Some(job) = state_guard.jobs.get_mut(&jid) {
                        job.status = luci_common::JobStatus::Cancelled;
                        job.completed_at = Some(chrono::Utc::now());
                    }
                    let _ = state_guard.job_manager.cancel_job(&jid).await;
                }
                Some(ProtocolMessage::new(
                    node_id,
                    MessageType::JobFailed,
                    Payload::JobFailed {
                        job_id: job_id.clone(),
                        error: "Cancelled by remote request".to_string(),
                    },
                ))
            }
            Payload::StateSync { state_type, data } => {
                tracing::info!("Received state sync for type '{}' ({} bytes)", state_type, data.len());
                // Update local state based on the type
                match state_type.as_str() {
                    "jobs" => {
                        if let Ok(jobs) = serde_json::from_slice::<Vec<serde_json::Value>>(data) {
                            tracing::info!("Synced {} jobs from peer", jobs.len());
                        }
                    }
                    "resources" => {
                        if let Ok(resources) = serde_json::from_slice::<luci_common::ResourceSnapshot>(data) {
                            tracing::info!("Synced resource snapshot from peer");
                            // Update peer resources
                            if let Some(mut peer) = peers.get_mut(&message.sender_id) {
                                peer.resources = Some(resources);
                            }
                        }
                    }
                    _ => {
                        tracing::debug!("Unknown state type: {}", state_type);
                    }
                }
                None
            }
            _ => {
                tracing::debug!("Unhandled message type: {:?}", message.msg_type);
                None
            }
        }
    }

    /// Send a message over a TLS stream
    pub async fn send_message_tls(stream: &mut tokio_rustls::client::TlsStream<TcpStream>, message: &ProtocolMessage) -> Result<()> {
        let codec: LengthPrefixedCodec<ProtocolMessage> = LengthPrefixedCodec::new();
        let mut framed = Framed::new(stream, codec);
        framed.send(message.clone()).await?;
        Ok(())
    }

    /// Send a message over a plain TCP stream
    pub async fn send_message_plain(stream: &mut TcpStream, message: &ProtocolMessage) -> Result<()> {
        let codec: LengthPrefixedCodec<ProtocolMessage> = LengthPrefixedCodec::new();
        let mut framed = Framed::new(stream, codec);
        framed.send(message.clone()).await?;
        Ok(())
    }

    /// Connect to a peer with TLS (or plain TCP if TLS not configured)
    pub async fn connect_peer(&self, addr: &str) -> Result<PeerStream> {
        let tcp_stream = TcpStream::connect(addr).await?;
        
        if let Some(client_config) = &self.client_tls_config {
            let connector = TlsConnector::from(client_config.clone());
            let domain = rustls::pki_types::ServerName::try_from("luci-node.local")
                .context("Invalid server name")?;
            let tls_stream = connector.connect(domain, tcp_stream).await
                .context("TLS handshake failed")?;
            Ok(PeerStream::Tls(tls_stream))
        } else {
            // No TLS configured, use plain TCP
            Ok(PeerStream::Plain(tcp_stream))
        }
    }

    /// Send a message to a peer (connects fresh each time)
    pub async fn send_message_to_peer(&self, addr: &str, message: &ProtocolMessage) -> Result<()> {
        let mut stream = self.connect_peer(addr).await?;
        match &mut stream {
            PeerStream::Tls(tls) => Self::send_message_tls(tls, message).await,
            PeerStream::Plain(tcp) => Self::send_message_plain(tcp, message).await,
        }
    }

    /// Send heartbeat to a peer
    pub async fn send_heartbeat(
        &self,
        addr: &str,
        sender_id: &str,
        resources: ResourceSnapshot,
        status: &str,
    ) -> Result<()> {
        let message = heartbeat_message(sender_id, resources, status);
        self.send_message_to_peer(addr, &message).await
    }

    /// Send job to a peer
    pub async fn send_job_to_peer(
        &self,
        addr: &str,
        sender_id: &str,
        job: &Job,
    ) -> Result<()> {
        let message = job_submit_message(sender_id, job.clone());
        self.send_message_to_peer(addr, &message).await
    }

    /// Get the TLS acceptor for incoming connections
    pub fn tls_acceptor(&self) -> Option<Arc<TlsAcceptor>> {
        self.tls_config.as_ref().map(|config| {
            Arc::new(TlsAcceptor::from(config.clone()))
        })
    }
}

use tokio::sync::RwLock;
