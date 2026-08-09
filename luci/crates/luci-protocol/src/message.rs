use chrono::{DateTime, Utc};
use luci_common::{
    InviteCode, Job, ResourceSnapshot,
};
use serde::{Deserialize, Serialize};

// ============================================================================
// Protocol Messages
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMessage {
    pub id: String,
    pub msg_type: MessageType,
    pub payload: Payload,
    pub timestamp: DateTime<Utc>,
    pub sender_id: String,
}

impl ProtocolMessage {
    pub fn new(sender_id: &str, msg_type: MessageType, payload: Payload) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            msg_type,
            payload,
            timestamp: Utc::now(),
            sender_id: sender_id.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    // Discovery
    Ping,
    Pong,
    Announce,
    Goodbye,
    
    // Heartbeat
    Heartbeat,
    HeartbeatAck,
    
    // Resource
    ResourceUpdate,
    ResourceQuery,
    ResourceResponse,
    
    // Job
    JobSubmit,
    JobAssigned,
    JobStarted,
    JobProgress,
    JobCompleted,
    JobFailed,
    JobCancel,
    
    // Memory
    MemoryPageRequest,
    MemoryPageResponse,
    MemoryPageWrite,
    MemoryPageAck,
    
    // Network
    JoinRequest,
    JoinResponse,
    LeaveNotification,

    // File sync
    FileSyncRequest,
    FileSyncData,
    FileSyncAck,

    // Chunked file transfer
    FileManifest,
    FileManifestResponse,
    FileChunk,
    FileChunkAck,
    FileTransferComplete,

    // State sync
    StateSync,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Payload {
    Ping,
    Pong {
        node_name: String,
    },
    
    Announce {
        node_id: String,
        node_name: String,
        hostname: String,
        os: String,
        arch: String,
        capabilities: Vec<String>,
    },
    
    Goodbye {
        reason: String,
    },
    
    Heartbeat {
        resources: ResourceSnapshot,
        status: String,
    },
    
    HeartbeatAck {
        cluster_size: u32,
    },
    
    ResourceUpdate {
        resources: ResourceSnapshot,
    },
    
    ResourceQuery,
    
    ResourceResponse {
        nodes: Vec<ResourceSnapshot>,
    },
    
    JobSubmit {
        job: Job,
    },
    
    JobAssigned {
        job_id: String,
        node_id: String,
    },
    
    JobStarted {
        job_id: String,
    },
    
    JobProgress {
        job_id: String,
        progress_percent: f32,
        message: Option<String>,
    },
    
    JobCompleted {
        job_id: String,
        result: String,
    },
    
    JobFailed {
        job_id: String,
        error: String,
    },
    
    JobCancel {
        job_id: String,
    },
    
    MemoryPageRequest {
        page_id: u64,
        pool_id: String,
    },
    
    MemoryPageResponse {
        page_id: u64,
        pool_id: String,
        data: Vec<u8>,
        compressed: bool,
        encrypted: bool,
    },
    
    MemoryPageWrite {
        page_id: u64,
        pool_id: String,
        data: Vec<u8>,
        compressed: bool,
        encrypted: bool,
    },
    
    MemoryPageAck {
        page_id: u64,
        pool_id: String,
        success: bool,
    },
    
    JoinRequest {
        invite_code: String,
        node_name: String,
        hostname: String,
        os: String,
        arch: String,
    },
    
    JoinResponse {
        success: bool,
        node_id: Option<String>,
        network_id: String,
        error: Option<String>,
    },
    
    LeaveNotification {
        node_id: String,
        reason: String,
    },

    FileSyncRequest {
        source_path: String,
        file_name: String,
        file_data: Vec<u8>,
        target_path: String,
    },

    FileSyncData {
        source_path: String,
        files: Vec<FileSyncEntry>,
    },

    FileSyncAck {
        success: bool,
        files_received: u64,
        error: Option<String>,
    },

    // Chunked file transfer
    FileManifest {
        transfer_id: String,
        root_path: String,
        files: Vec<FileManifestEntry>,
    },

    FileManifestResponse {
        transfer_id: String,
        needed_files: Vec<String>,
    },

    FileChunk {
        transfer_id: String,
        file_path: String,
        chunk_index: u32,
        total_chunks: u32,
        data: Vec<u8>,
        chunk_hash: String,
    },

    FileChunkAck {
        transfer_id: String,
        chunk_index: u32,
        success: bool,
        error: Option<String>,
    },

    FileTransferComplete {
        transfer_id: String,
        total_files: u32,
        total_bytes: u64,
    },

    StateSync {
        state_type: String,
        data: Vec<u8>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSyncEntry {
    pub path: String,
    pub data: Vec<u8>,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileManifestEntry {
    pub relative_path: String,
    pub size: u64,
    pub blake3_hash: String,
    pub modified_at: DateTime<Utc>,
}

impl Payload {
    pub fn message_type(&self) -> MessageType {
        match self {
            Payload::Ping => MessageType::Ping,
            Payload::Pong { .. } => MessageType::Pong,
            Payload::Announce { .. } => MessageType::Announce,
            Payload::Goodbye { .. } => MessageType::Goodbye,
            Payload::Heartbeat { .. } => MessageType::Heartbeat,
            Payload::HeartbeatAck { .. } => MessageType::HeartbeatAck,
            Payload::ResourceUpdate { .. } => MessageType::ResourceUpdate,
            Payload::ResourceQuery => MessageType::ResourceQuery,
            Payload::ResourceResponse { .. } => MessageType::ResourceResponse,
            Payload::JobSubmit { .. } => MessageType::JobSubmit,
            Payload::JobAssigned { .. } => MessageType::JobAssigned,
            Payload::JobStarted { .. } => MessageType::JobStarted,
            Payload::JobProgress { .. } => MessageType::JobProgress,
            Payload::JobCompleted { .. } => MessageType::JobCompleted,
            Payload::JobFailed { .. } => MessageType::JobFailed,
            Payload::JobCancel { .. } => MessageType::JobCancel,
            Payload::MemoryPageRequest { .. } => MessageType::MemoryPageRequest,
            Payload::MemoryPageResponse { .. } => MessageType::MemoryPageResponse,
            Payload::MemoryPageWrite { .. } => MessageType::MemoryPageWrite,
            Payload::MemoryPageAck { .. } => MessageType::MemoryPageAck,
            Payload::JoinRequest { .. } => MessageType::JoinRequest,
            Payload::JoinResponse { .. } => MessageType::JoinResponse,
            Payload::LeaveNotification { .. } => MessageType::LeaveNotification,
            Payload::FileSyncRequest { .. } => MessageType::FileSyncRequest,
            Payload::FileSyncData { .. } => MessageType::FileSyncData,
            Payload::FileSyncAck { .. } => MessageType::FileSyncAck,
            Payload::FileManifest { .. } => MessageType::FileManifest,
            Payload::FileManifestResponse { .. } => MessageType::FileManifestResponse,
            Payload::FileChunk { .. } => MessageType::FileChunk,
            Payload::FileChunkAck { .. } => MessageType::FileChunkAck,
            Payload::FileTransferComplete { .. } => MessageType::FileTransferComplete,
            Payload::StateSync { .. } => MessageType::StateSync,
        }
    }
}

// ============================================================================
// Helper functions for creating messages
// ============================================================================

pub fn ping_message(sender_id: &str) -> ProtocolMessage {
    ProtocolMessage::new(sender_id, MessageType::Ping, Payload::Ping)
}

pub fn pong_message(sender_id: &str, node_name: &str) -> ProtocolMessage {
    ProtocolMessage::new(
        sender_id,
        MessageType::Pong,
        Payload::Pong {
            node_name: node_name.to_string(),
        },
    )
}

pub fn announce_message(
    sender_id: &str,
    node_id: &str,
    node_name: &str,
    hostname: &str,
    os: &str,
    arch: &str,
    capabilities: Vec<String>,
) -> ProtocolMessage {
    ProtocolMessage::new(
        sender_id,
        MessageType::Announce,
        Payload::Announce {
            node_id: node_id.to_string(),
            node_name: node_name.to_string(),
            hostname: hostname.to_string(),
            os: os.to_string(),
            arch: arch.to_string(),
            capabilities,
        },
    )
}

pub fn heartbeat_message(
    sender_id: &str,
    resources: ResourceSnapshot,
    status: &str,
) -> ProtocolMessage {
    ProtocolMessage::new(
        sender_id,
        MessageType::Heartbeat,
        Payload::Heartbeat {
            resources,
            status: status.to_string(),
        },
    )
}

pub fn job_submit_message(sender_id: &str, job: Job) -> ProtocolMessage {
    ProtocolMessage::new(
        sender_id,
        MessageType::JobSubmit,
        Payload::JobSubmit { job },
    )
}

pub fn job_completed_message(sender_id: &str, job_id: &str, result: &str) -> ProtocolMessage {
    ProtocolMessage::new(
        sender_id,
        MessageType::JobCompleted,
        Payload::JobCompleted {
            job_id: job_id.to_string(),
            result: result.to_string(),
        },
    )
}

pub fn job_failed_message(sender_id: &str, job_id: &str, error: &str) -> ProtocolMessage {
    ProtocolMessage::new(
        sender_id,
        MessageType::JobFailed,
        Payload::JobFailed {
            job_id: job_id.to_string(),
            error: error.to_string(),
        },
    )
}

pub fn join_request_message(
    sender_id: &str,
    invite_code: &InviteCode,
    node_name: &str,
    hostname: &str,
    os: &str,
    arch: &str,
) -> ProtocolMessage {
    ProtocolMessage::new(
        sender_id,
        MessageType::JoinRequest,
        Payload::JoinRequest {
            invite_code: invite_code.encode().to_string(),
            node_name: node_name.to_string(),
            hostname: hostname.to_string(),
            os: os.to_string(),
            arch: arch.to_string(),
        },
    )
}

pub fn file_manifest_message(
    sender_id: &str,
    transfer_id: &str,
    root_path: &str,
    files: Vec<FileManifestEntry>,
) -> ProtocolMessage {
    ProtocolMessage::new(
        sender_id,
        MessageType::FileManifest,
        Payload::FileManifest {
            transfer_id: transfer_id.to_string(),
            root_path: root_path.to_string(),
            files,
        },
    )
}

pub fn file_chunk_message(
    sender_id: &str,
    transfer_id: &str,
    file_path: &str,
    chunk_index: u32,
    total_chunks: u32,
    data: Vec<u8>,
    chunk_hash: &str,
) -> ProtocolMessage {
    ProtocolMessage::new(
        sender_id,
        MessageType::FileChunk,
        Payload::FileChunk {
            transfer_id: transfer_id.to_string(),
            file_path: file_path.to_string(),
            chunk_index,
            total_chunks,
            data,
            chunk_hash: chunk_hash.to_string(),
        },
    )
}

pub fn file_transfer_complete_message(
    sender_id: &str,
    transfer_id: &str,
    total_files: u32,
    total_bytes: u64,
) -> ProtocolMessage {
    ProtocolMessage::new(
        sender_id,
        MessageType::FileTransferComplete,
        Payload::FileTransferComplete {
            transfer_id: transfer_id.to_string(),
            total_files,
            total_bytes,
        },
    )
}
