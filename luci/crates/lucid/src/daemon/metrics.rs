use std::sync::Arc;
use tokio::sync::RwLock;

/// Prometheus metrics formatter
pub struct PrometheusMetrics {
    state: Arc<RwLock<crate::daemon::DaemonState>>,
}

impl PrometheusMetrics {
    pub fn new(state: Arc<RwLock<crate::daemon::DaemonState>>) -> Self {
        Self { state }
    }

    pub async fn render(&self) -> String {
        let state = self.state.read().await;
        let metrics = state.metrics.get_snapshot();
        let uptime = state.start_time.elapsed().as_secs();
        let connected_nodes = state.connected_nodes();
        let active_jobs = state.active_jobs();

        format!(
            r#"# HELP luci_uptime_seconds Daemon uptime in seconds
# TYPE luci_uptime_seconds counter
luci_uptime_seconds {uptime}

# HELP luci_connected_nodes Number of connected nodes
# TYPE luci_connected_nodes gauge
luci_connected_nodes {connected_nodes}

# HELP luci_active_jobs Number of active jobs
# TYPE luci_active_jobs gauge
luci_active_jobs {active_jobs}

# HELP luci_jobs_submitted Total jobs submitted
# TYPE luci_jobs_submitted counter
luci_jobs_submitted {jobs_submitted}

# HELP luci_jobs_completed Total jobs completed
# TYPE luci_jobs_completed counter
luci_jobs_completed {jobs_completed}

# HELP luci_jobs_failed Total jobs failed
# TYPE luci_jobs_failed counter
luci_jobs_failed {jobs_failed}

# HELP luci_bytes_transferred Total bytes transferred
# TYPE luci_bytes_transferred counter
luci_bytes_transferred {bytes_transferred}

# HELP luci_active_connections Active connections
# TYPE luci_active_connections gauge
luci_active_connections {active_connections}

# HELP luci_total_requests Total requests
# TYPE luci_total_requests counter
luci_total_requests {total_requests}

# HELP luci_errors Total errors
# TYPE luci_errors counter
luci_errors {errors}
"#,
            uptime = uptime,
            connected_nodes = connected_nodes,
            active_jobs = active_jobs,
            jobs_submitted = metrics.jobs_submitted,
            jobs_completed = metrics.jobs_completed,
            jobs_failed = metrics.jobs_failed,
            bytes_transferred = metrics.bytes_transferred,
            active_connections = metrics.active_connections,
            total_requests = metrics.total_requests,
            errors = metrics.errors,
        )
    }
}
