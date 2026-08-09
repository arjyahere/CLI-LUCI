use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Metrics collector for the daemon
pub struct MetricsCollector {
    start_time: Instant,
    jobs_submitted: Arc<AtomicU64>,
    jobs_completed: Arc<AtomicU64>,
    jobs_failed: Arc<AtomicU64>,
    jobs_running: Arc<AtomicU64>,
    bytes_transferred: Arc<AtomicU64>,
    active_connections: Arc<AtomicU64>,
    total_requests: Arc<AtomicU64>,
    errors: Arc<AtomicU64>,
    custom_counters: Arc<std::sync::RwLock<HashMap<String, u64>>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            jobs_submitted: Arc::new(AtomicU64::new(0)),
            jobs_completed: Arc::new(AtomicU64::new(0)),
            jobs_failed: Arc::new(AtomicU64::new(0)),
            jobs_running: Arc::new(AtomicU64::new(0)),
            bytes_transferred: Arc::new(AtomicU64::new(0)),
            active_connections: Arc::new(AtomicU64::new(0)),
            total_requests: Arc::new(AtomicU64::new(0)),
            errors: Arc::new(AtomicU64::new(0)),
            custom_counters: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    pub fn record_job_submitted(&self) {
        self.jobs_submitted.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_job_completed(&self) {
        self.jobs_completed.fetch_add(1, Ordering::Relaxed);
        self.jobs_running.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn record_job_failed(&self) {
        self.jobs_failed.fetch_add(1, Ordering::Relaxed);
        self.jobs_running.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn record_job_started(&self) {
        self.jobs_running.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_bytes_transferred(&self, bytes: u64) {
        self.bytes_transferred.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_connection_opened(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_connection_closed(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn record_request(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_custom(&self, name: &str) {
        let mut counters = self.custom_counters.write().unwrap();
        *counters.entry(name.to_string()).or_insert(0) += 1;
    }

    pub fn get_snapshot(&self) -> MetricsSnapshot {
        let uptime_secs = self.start_time.elapsed().as_secs();
        let jobs_submitted = self.jobs_submitted.load(Ordering::Relaxed);
        let jobs_completed = self.jobs_completed.load(Ordering::Relaxed);
        let jobs_failed = self.jobs_failed.load(Ordering::Relaxed);

        let success_rate = if jobs_submitted > 0 {
            (jobs_completed as f64 / jobs_submitted as f64) * 100.0
        } else {
            100.0
        };

        let custom_counters = self.custom_counters.read().unwrap().clone();

        MetricsSnapshot {
            uptime_secs,
            jobs_submitted,
            jobs_completed,
            jobs_failed,
            jobs_running: self.jobs_running.load(Ordering::Relaxed),
            bytes_transferred: self.bytes_transferred.load(Ordering::Relaxed),
            active_connections: self.active_connections.load(Ordering::Relaxed),
            total_requests: self.total_requests.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            success_rate,
            custom_counters,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricsSnapshot {
    pub uptime_secs: u64,
    pub jobs_submitted: u64,
    pub jobs_completed: u64,
    pub jobs_failed: u64,
    pub jobs_running: u64,
    pub bytes_transferred: u64,
    pub active_connections: u64,
    pub total_requests: u64,
    pub errors: u64,
    pub success_rate: f64,
    pub custom_counters: HashMap<String, u64>,
}

impl MetricsSnapshot {
    pub fn to_prometheus(&self) -> String {
        let mut output = String::new();
        
        output.push_str("# HELP luci_uptime_seconds Daemon uptime in seconds\n");
        output.push_str(&format!("luci_uptime_seconds {}\n", self.uptime_secs));
        
        output.push_str("# HELP luci_jobs_submitted Total jobs submitted\n");
        output.push_str(&format!("luci_jobs_submitted {}\n", self.jobs_submitted));
        
        output.push_str("# HELP luci_jobs_completed Total jobs completed successfully\n");
        output.push_str(&format!("luci_jobs_completed {}\n", self.jobs_completed));
        
        output.push_str("# HELP luci_jobs_failed Total jobs failed\n");
        output.push_str(&format!("luci_jobs_failed {}\n", self.jobs_failed));
        
        output.push_str("# HELP luci_jobs_running Currently running jobs\n");
        output.push_str(&format!("luci_jobs_running {}\n", self.jobs_running));
        
        output.push_str("# HELP luci_bytes_transferred_total Total bytes transferred\n");
        output.push_str(&format!("luci_bytes_transferred_total {}\n", self.bytes_transferred));
        
        output.push_str("# HELP luci_active_connections Current active connections\n");
        output.push_str(&format!("luci_active_connections {}\n", self.active_connections));
        
        output.push_str("# HELP luci_requests_total Total requests handled\n");
        output.push_str(&format!("luci_requests_total {}\n", self.total_requests));
        
        output.push_str("# HELP luci_errors_total Total errors\n");
        output.push_str(&format!("luci_errors_total {}\n", self.errors));
        
        output.push_str("# HELP luci_success_rate Job success rate percentage\n");
        output.push_str(&format!("luci_success_rate {:.2}\n", self.success_rate));

        for (name, value) in &self.custom_counters {
            output.push_str(&format!("luci_{} {}\n", name, value));
        }

        output
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// Health check status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub version: String,
    pub uptime_secs: u64,
    pub node_id: String,
    pub network_id: Option<String>,
    pub jobs_running: u64,
    pub memory_usage_mb: u64,
    pub disk_usage_mb: u64,
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        self.status == "ok"
    }
}

/// Tracing helper for job execution
pub struct JobTracer {
    job_id: String,
    start_time: Instant,
}

impl JobTracer {
    pub fn new(job_id: &str) -> Self {
        tracing::info!(job_id = job_id, "Job execution started");
        Self {
            job_id: job_id.to_string(),
            start_time: Instant::now(),
        }
    }

    pub fn completed(&self, success: bool, exit_code: i32) {
        let duration = self.start_time.elapsed();
        if success {
            tracing::info!(
                job_id = self.job_id,
                duration_ms = duration.as_millis() as u64,
                exit_code = exit_code,
                "Job execution completed successfully"
            );
        } else {
            tracing::warn!(
                job_id = self.job_id,
                duration_ms = duration.as_millis() as u64,
                exit_code = exit_code,
                "Job execution failed"
            );
        }
    }

    pub fn failed(&self, error: &str) {
        let duration = self.start_time.elapsed();
        tracing::error!(
            job_id = self.job_id,
            duration_ms = duration.as_millis() as u64,
            error = error,
            "Job execution failed with error"
        );
    }
}