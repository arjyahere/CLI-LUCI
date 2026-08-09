use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Audit event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    NodeJoined { node_id: String, node_name: String },
    NodeLeft { node_id: String, reason: String },
    JobSubmitted { job_id: String, job_name: String, node_id: Option<String> },
    JobCompleted { job_id: String, success: bool },
    JobFailed { job_id: String, error: String },
    MemoryPageAccess { pool_id: String, page_id: u64, operation: String },
    FileSync { source: String, target: String, files: u64 },
    AuthChallenge { node_id: String, success: bool },
    RateLimitExceeded { client_id: String },
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub node_id: String,
    pub details: HashMap<String, String>,
}

/// Audit logger for tracking system events
#[derive(Debug, Clone)]
pub struct AuditLogger {
    entries: Arc<RwLock<Vec<AuditEntry>>>,
    max_entries: usize,
    node_id: String,
}

impl AuditLogger {
    pub fn new(node_id: String, max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            max_entries,
            node_id,
        }
    }

    pub async fn log_event(&self, event_type: AuditEventType, details: HashMap<String, String>) {
        let entry = AuditEntry {
            timestamp: Utc::now(),
            event_type,
            node_id: self.node_id.clone(),
            details,
        };

        let mut entries = self.entries.write().await;
        entries.push(entry);

        // Trim old entries if over limit
        if entries.len() > self.max_entries {
            let excess = entries.len() - self.max_entries;
            entries.drain(0..excess);
        }
    }

    pub async fn get_entries(&self, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.read().await;
        entries.iter().rev().take(limit).cloned().collect()
    }

    pub async fn get_entries_since(&self, since: DateTime<Utc>) -> Vec<AuditEntry> {
        let entries = self.entries.read().await;
        entries.iter()
            .filter(|e| e.timestamp >= since)
            .cloned()
            .collect()
    }

    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();
    }
}
