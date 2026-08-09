use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

// ============================================================================
// ID Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(String);

impl NodeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        Uuid::parse_str(s)
            .map(|_| Self(s.to_string()))
            .map_err(|e| format!("Invalid node ID: {}", e))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn short(&self) -> &str {
        if self.0.len() > 8 {
            &self.0[..8]
        } else {
            &self.0
        }
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NetworkId(String);

impl NetworkId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for NetworkId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NetworkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(String);

impl JobId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for JobId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for JobId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InviteCode(String);

impl InviteCode {
    pub fn new() -> Self {
        let uuid = Uuid::new_v4();
        let bytes = uuid.as_bytes();
        let encoded = base64url_encode(bytes);
        // Take first 15 characters and format with dot separator
        // Base64url alphabet: A-Z, a-z, 0-9, -, _
        // Dot is NOT in base64url, so we use it as separator to avoid ambiguity
        let part1 = &encoded[0..4];
        let part2 = &encoded[4..8];
        let part3 = &encoded[8..12];
        let part4 = &encoded[12..15];
        let formatted = format!("{}.{}.{}.{}", part1, part2, part3, part4);
        Self(formatted)
    }

    pub fn encode(&self) -> &str {
        &self.0
    }

    pub fn decode(s: &str) -> Result<Self, String> {
        // Expected format: "XXXX.XXXX.XXX" (4.4.4.3 with dot separator)
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 4 {
            return Err("Invalid invite code format".to_string());
        }
        let cleaned: String = parts.concat();
        if cleaned.len() < 15 {
            return Err("Invalid invite code length".to_string());
        }
        let truncated = &cleaned[..15];
        let formatted = format!(
            "{}.{}.{}.{}",
            &truncated[0..4],
            &truncated[4..8],
            &truncated[8..12],
            &truncated[12..15]
        );
        Ok(Self(formatted))
    }
}

impl Default for InviteCode {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for InviteCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for InviteCode {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

fn base64url_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        }
    }
    result
}

// ============================================================================
// System Information Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: NodeId,
    pub name: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    pub node_id: NodeId,
    pub timestamp: DateTime<Utc>,
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub gpu: Option<GpuInfo>,
    pub disk: DiskInfo,
    pub network: NetworkInfo,
    pub idle_seconds: u64,
    pub is_charging: Option<bool>,
    pub battery_percent: Option<f32>,
    pub temperature_c: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub cores: usize,
    pub load_percent: f32,
    pub frequency_mhz: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
}

impl MemoryInfo {
    pub fn total_gb(&self) -> f64 {
        self.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn available_gb(&self) -> f64 {
        self.available_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn used_gb(&self) -> f64 {
        self.used_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn used_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.used_bytes as f32 / self.total_bytes as f32) * 100.0
    }

    pub fn can_allocate(&self, bytes: u64) -> bool {
        self.available_bytes >= bytes
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vram_total_bytes: u64,
    pub vram_used_bytes: u64,
    pub vram_available_bytes: u64,
    pub utilization_percent: f32,
    pub temperature_c: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub total_bytes: u64,
    pub free_bytes: u64,
}

impl DiskInfo {
    pub fn total_gb(&self) -> f64 {
        self.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn free_gb(&self) -> f64 {
        self.free_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

// ============================================================================
// Node State Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Online,
    Idle,
    Busy,
    Offline,
    Error,
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeStatus::Online => write!(f, "online"),
            NodeStatus::Idle => write!(f, "idle"),
            NodeStatus::Busy => write!(f, "busy"),
            NodeStatus::Offline => write!(f, "offline"),
            NodeStatus::Error => write!(f, "error"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    pub info: NodeInfo,
    pub status: NodeStatus,
    pub resources: Option<ResourceSnapshot>,
    pub last_heartbeat: DateTime<Utc>,
}

impl NodeState {
    pub fn is_available(&self) -> bool {
        matches!(self.status, NodeStatus::Online | NodeStatus::Idle)
    }

    pub fn score(&self) -> f64 {
        self.resources
            .as_ref()
            .map(|r| {
                let ram_score = r.memory.available_gb() * 10.0;
                let cpu_score = (100.0 - r.cpu.load_percent as f64) * 0.5;
                let idle_bonus = if r.idle_seconds > 300 { 20.0 } else { 0.0 };
                let charging_bonus = if r.is_charging.unwrap_or(false) { 5.0 } else { 0.0 };
                ram_score + cpu_score + idle_bonus + charging_bonus
            })
            .unwrap_or(0.0)
    }
}

// ============================================================================
// Job Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobType {
    Python,
    NodeJs,
    Shell,
    Custom,
}

impl fmt::Display for JobType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobType::Python => write!(f, "python"),
            JobType::NodeJs => write!(f, "nodejs"),
            JobType::Shell => write!(f, "shell"),
            JobType::Custom => write!(f, "custom"),
        }
    }
}

impl std::str::FromStr for JobType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "python" | "py" => Ok(JobType::Python),
            "nodejs" | "node" | "js" => Ok(JobType::NodeJs),
            "shell" | "sh" | "bash" => Ok(JobType::Shell),
            "custom" => Ok(JobType::Custom),
            _ => Err(format!("Unknown job type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Scheduled,
    Transferring,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobStatus::Pending => write!(f, "pending"),
            JobStatus::Scheduled => write!(f, "scheduled"),
            JobStatus::Transferring => write!(f, "transferring"),
            JobStatus::Running => write!(f, "running"),
            JobStatus::Completed => write!(f, "completed"),
            JobStatus::Failed => write!(f, "failed"),
            JobStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRequirements {
    pub min_ram_bytes: u64,
    pub min_cpu_cores: usize,
    pub requires_gpu: bool,
    pub min_vram_bytes: Option<u64>,
    pub max_duration_secs: Option<u64>,
    pub preferred_nodes: Vec<NodeId>,
}

impl Default for JobRequirements {
    fn default() -> Self {
        Self {
            min_ram_bytes: 1024 * 1024 * 1024, // 1GB
            min_cpu_cores: 1,
            requires_gpu: false,
            min_vram_bytes: None,
            max_duration_secs: None,
            preferred_nodes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub name: String,
    pub job_type: JobType,
    pub status: JobStatus,
    pub assigned_node: Option<NodeId>,
    pub requirements: JobRequirements,
    pub script_path: Option<String>,
    pub script_content: Option<String>,
    pub args: Vec<String>,
    pub env: std::collections::HashMap<String, String>,
    pub working_dir: Option<std::path::PathBuf>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub exit_code: Option<i32>,
}

impl Job {
    pub fn new(name: String, job_type: JobType, requirements: JobRequirements) -> Self {
        Self {
            id: JobId::new(),
            name,
            job_type,
            status: JobStatus::Pending,
            assigned_node: None,
            requirements,
            script_path: None,
            script_content: None,
            args: Vec::new(),
            env: std::collections::HashMap::new(),
            working_dir: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
            exit_code: None,
        }
    }

    pub fn with_script_path(mut self, path: String) -> Self {
        self.script_path = Some(path);
        self
    }

    pub fn with_script_content(mut self, content: String) -> Self {
        self.script_content = Some(content);
        self
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_env(mut self, key: String, value: String) -> Self {
        self.env.insert(key, value);
        self
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
        )
    }
}

// ============================================================================
// Memory Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryPageStatus {
    Clean,
    Dirty,
    Migrating,
    Evicted,
}

impl fmt::Display for MemoryPageStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryPageStatus::Clean => write!(f, "clean"),
            MemoryPageStatus::Dirty => write!(f, "dirty"),
            MemoryPageStatus::Migrating => write!(f, "migrating"),
            MemoryPageStatus::Evicted => write!(f, "evicted"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPage {
    pub page_id: u64,
    pub owner_node: NodeId,
    pub size_bytes: u32,
    pub status: MemoryPageStatus,
    pub compressed: bool,
    pub encrypted: bool,
    pub access_count: u64,
    pub last_accessed: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPool {
    pub id: String,
    pub name: String,
    pub total_pages: u64,
    pub page_size_bytes: u32,
    pub owner_nodes: Vec<NodeId>,
    pub created_at: DateTime<Utc>,
}

impl MemoryPool {
    pub fn total_size_bytes(&self) -> u64 {
        self.total_pages * self.page_size_bytes as u64
    }

    pub fn total_size_gb(&self) -> f64 {
        self.total_size_bytes() as f64 / (1024.0 * 1024.0 * 1024.0)
    }
}

// ============================================================================
// Daemon Status
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub uptime_secs: u64,
    pub node_id: NodeId,
    pub network_id: Option<String>,
    pub connected_nodes: usize,
    pub active_jobs: usize,
    pub total_jobs_completed: u64,
}

// ============================================================================
// Network Information
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMeta {
    pub id: String,
    pub name: String,
    pub node_count: usize,
    pub created_at: DateTime<Utc>,
}
