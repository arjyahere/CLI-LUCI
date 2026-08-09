use serde::{Deserialize, Serialize};

// ============================================================================
// IPC Protocol for CLI <-> Daemon Communication
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcCommand {
    // Network management
    CreateNetwork { name: String },
    JoinNetwork { invite_code: String },
    LeaveNetwork { name: Option<String> },
    ListNetworks,

    // Device management
    ListDevices,
    GetDevice { name: String },

    // Resources
    GetResources,

    // Job management
    RunJob {
        name: String,
        job_type: String,
        script_path: Option<String>,
        script_content: Option<String>,
        args: Vec<String>,
        ram: Option<u64>,
        cpu: Option<usize>,
        node: Option<String>,
        requires_gpu: bool,
        min_vram_bytes: Option<u64>,
        max_duration_secs: Option<u64>,
    },
    GetJobStatus { job_id: String },
    ListJobs,
    CancelJob { job_id: String },

    // Memory management
    CreateMemoryPool { name: String, size_gb: f64 },
    ListMemoryPools,
    GetMemoryPoolInfo { pool_id: String },
    DeleteMemoryPool { pool_id: String },
    ReadRemoteMemoryPage { pool_id: String, page_id: u64, target_node: String },
    WriteRemoteMemoryPage { pool_id: String, page_id: u64, data: String, target_node: String },

    // Daemon management
    GetDaemonStatus,
    Ping,

    // Resource monitoring
    GetMonitorData,

    // File sync
    SyncFiles {
        source_path: String,
        target_node: String,
    },
    ListSyncedFolders,

    // Reverse proxy
    StartProxy {
        listen_port: u16,
        target_nodes: Vec<String>,
    },
    StopProxy,
    GetProxyStatus,

    // Auto-failover
    GetFailoverStatus,

    // Cross-network discovery
    GetPeers,
    SyncPeers,

    // Health & Metrics
    GetHealth,
    GetMetrics,
    GetPrometheusMetrics,

    // Window control
    ShowDaemonWindow,
    HideDaemonWindow,

    // Daemon lifecycle
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcResponse {
    Success { payload: serde_json::Value },
    Error { message: String },
}

impl IpcResponse {
    pub fn success(value: serde_json::Value) -> Self {
        Self::Success { payload: value }
    }

    pub fn error(message: String) -> Self {
        Self::Error { message }
    }

    pub fn ok() -> Self {
        Self::Success {
            payload: serde_json::Value::Null,
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, IpcResponse::Success { .. })
    }

    pub fn payload(&self) -> Option<&serde_json::Value> {
        match self {
            IpcResponse::Success { payload } => Some(payload),
            IpcResponse::Error { .. } => None,
        }
    }

    pub fn error_message(&self) -> Option<&str> {
        match self {
            IpcResponse::Success { .. } => None,
            IpcResponse::Error { message } => Some(message),
        }
    }
}

// ============================================================================
// IPC Request (used for serialization)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcRequest {
    pub command: IpcCommand,
    #[serde(default)]
    pub auth_token: Option<String>,
}

impl IpcRequest {
    pub fn new(command: IpcCommand) -> Self {
        Self {
            command,
            auth_token: None,
        }
    }
}


