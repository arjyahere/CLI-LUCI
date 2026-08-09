use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Daemon not running. Start with: lucid start")]
    DaemonNotRunning,

    #[error("Daemon already running (PID: {0})")]
    DaemonAlreadyRunning(u32),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Job not found: {0}")]
    JobNotFound(String),

    #[error("Memory pool not found: {0}")]
    MemoryPoolNotFound(String),

    #[error("Insufficient resources: {0}")]
    InsufficientResources(String),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Connection refused: {0}")]
    ConnectionRefused(String),

    #[error("Connection timeout")]
    ConnectionTimeout,

    #[error("Invalid invite code: {0}")]
    InvalidInviteCode(String),

    #[error("Already connected to network: {0}")]
    AlreadyConnected(String),

    #[error("Not connected to any network")]
    NotConnected,

    #[error("Job execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Sandbox violation: {0}")]
    SandboxViolation(String),

    #[error("Worker error: {0}")]
    Worker(String),

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("Memory error: {0}")]
    Memory(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}
