use luci_common::{
    Config, InviteCode, Job, JobId, JobType, MetricsCollector, NodeId, MemoryPool,
};

#[test]
fn test_node_id_creation() {
    let id1 = NodeId::new();
    let id2 = NodeId::new();
    assert_ne!(id1.to_string(), id2.to_string());
}

#[test]
fn test_node_id_parse() {
    let id = NodeId::new();
    let parsed = NodeId::parse(&id.to_string());
    assert!(parsed.is_ok());
    assert_eq!(id, parsed.unwrap());
}

#[test]
fn test_invite_code_creation() {
    let code1 = InviteCode::new();
    let code2 = InviteCode::new();
    assert_ne!(code1.to_string(), code2.to_string());
}

#[test]
fn test_invite_code_encode_decode() {
    let code = InviteCode::new();
    let encoded = code.encode();
    let decoded = InviteCode::decode(encoded).unwrap();
    assert_eq!(code.to_string(), decoded.to_string());
}

#[test]
fn test_config_default() {
    let config = Config::default();
    assert_eq!(config.daemon_port, 9527);
    assert_eq!(config.node_port, 9528);
    assert_eq!(config.contribute_ram_percent, 60);
}

#[test]
fn test_job_creation() {
    let job = Job::new(
        "test-job".to_string(),
        JobType::Python,
        Default::default(),
    );
    assert_eq!(job.name, "test-job");
    assert_eq!(job.job_type, JobType::Python);
    assert!(job.id.to_string().len() > 0);
}

#[test]
fn test_job_id_creation() {
    let id1 = JobId::new();
    let id2 = JobId::new();
    assert_ne!(id1.to_string(), id2.to_string());
}

#[test]
fn test_memory_pool_creation() {
    let pool = MemoryPool {
        id: "test-pool".to_string(),
        name: "test".to_string(),
        total_pages: 8388608,
        page_size_bytes: 4096,
        owner_nodes: vec![],
        created_at: chrono::Utc::now(),
    };
    assert_eq!(pool.total_size_gb(), 32.0);
}

#[test]
fn test_metrics_collector() {
    let metrics = MetricsCollector::new();
    
    // Record some metrics
    metrics.record_job_submitted();
    metrics.record_job_started();
    metrics.record_job_completed();
    metrics.record_request();
    metrics.record_bytes_transferred(1024);
    
    let snapshot = metrics.get_snapshot();
    assert_eq!(snapshot.jobs_submitted, 1);
    assert_eq!(snapshot.jobs_completed, 1);
    assert_eq!(snapshot.jobs_running, 0);
    assert_eq!(snapshot.bytes_transferred, 1024);
    assert_eq!(snapshot.total_requests, 1);
    assert_eq!(snapshot.errors, 0);
    assert!((snapshot.success_rate - 100.0).abs() < f64::EPSILON);
}

#[test]
fn test_metrics_prometheus_format() {
    let metrics = MetricsCollector::new();
    metrics.record_job_submitted();
    metrics.record_job_completed();
    
    let snapshot = metrics.get_snapshot();
    let prometheus = snapshot.to_prometheus();
    
    assert!(prometheus.contains("luci_uptime_seconds"));
    assert!(prometheus.contains("luci_jobs_submitted 1"));
    assert!(prometheus.contains("luci_jobs_completed 1"));
    assert!(prometheus.contains("luci_success_rate"));
}

#[test]
fn test_metrics_failure_tracking() {
    let metrics = MetricsCollector::new();
    
    metrics.record_job_submitted();
    metrics.record_job_started();
    metrics.record_job_failed();
    
    let snapshot = metrics.get_snapshot();
    assert_eq!(snapshot.jobs_submitted, 1);
    assert_eq!(snapshot.jobs_failed, 1);
    assert_eq!(snapshot.jobs_running, 0);
    assert!((snapshot.success_rate - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_metrics_custom_counters() {
    let metrics = MetricsCollector::new();
    
    metrics.record_custom("my_counter");
    metrics.record_custom("my_counter");
    metrics.record_custom("other_counter");
    
    let snapshot = metrics.get_snapshot();
    assert_eq!(snapshot.custom_counters.get("my_counter"), Some(&2));
    assert_eq!(snapshot.custom_counters.get("other_counter"), Some(&1));
}
