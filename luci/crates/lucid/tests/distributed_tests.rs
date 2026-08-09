//! Tests for distributed features:
//! - Remote job execution with status reporting
//! - Distributed memory operations
//! - File sync manifest building and diffing
//! - Protocol message handling

use luci_common::*;
use luci_protocol::*;
use luci_runtime::*;
use lucid::daemon::job::JobManager;
use lucid::daemon::file_sync::FileTransferManager;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Helper to create a test job
fn make_test_job(name: &str, script_content: &str) -> Job {
    Job::new(
        name.to_string(),
        JobType::Shell,
        JobRequirements::default(),
    )
    .with_script_content(script_content.to_string())
}

/// Helper to create a job with script path
fn make_script_job(name: &str, script_path: &str) -> Job {
    Job::new(
        name.to_string(),
        JobType::Shell,
        JobRequirements::default(),
    )
    .with_script_path(script_path.to_string())
}

// ============================================================================
// TEST 1: Job Manager - Submit and Track Job Status
// ============================================================================

#[tokio::test]
async fn test_job_manager_submit_and_track() {
    let metrics = Arc::new(MetricsCollector::new());
    let job_manager = JobManager::new("test-node", 4, metrics);

    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("test.bat");
    #[cfg(windows)]
    std::fs::write(&script, "@echo off\r\necho hello").unwrap();
    #[cfg(not(windows))]
    {
        let script = temp.path().join("test.sh");
        std::fs::write(&script, "#!/bin/sh\necho hello").unwrap();
    }

    let job = make_script_job("test-job", script.to_str().unwrap());

    // Submit job
    let result = job_manager.submit_job(job.clone(), temp.path()).await;
    assert!(result.is_ok(), "Job should submit successfully");

    // Check job is tracked
    let tracked = job_manager.get_job(&job.id).await;
    assert!(tracked.is_some(), "Job should be tracked");
    let tracked = tracked.unwrap();
    assert_eq!(tracked.status, JobStatus::Running);

    // Wait for job to complete (with retries)
    let mut attempts = 0;
    loop {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let final_job = job_manager.get_job(&job.id).await.unwrap();
        if final_job.status == JobStatus::Completed || final_job.status == JobStatus::Failed {
            break;
        }
        attempts += 1;
        if attempts > 20 {
            panic!("Job did not complete within 4 seconds, last status: {:?}", final_job.status);
        }
    }
}

// ============================================================================
// TEST 2: Job Manager - Update Job Status from Remote
// ============================================================================

#[tokio::test]
async fn test_job_manager_update_status() {
    let metrics = Arc::new(MetricsCollector::new());
    let job_manager = JobManager::new("test-node", 4, metrics);

    let job = make_test_job("status-test", "echo test");

    // Create a local tracking entry (simulating remote job)
    job_manager.insert_job(job.clone()).await;

    // Update status to completed
    let result = job_manager.update_job_status(
        &job.id,
        JobStatus::Completed,
        Some("remote result".to_string()),
    ).await;
    assert!(result.is_ok());

    // Verify update
    let updated = job_manager.get_job(&job.id).await;
    assert!(updated.is_some());
    let updated = updated.unwrap();
    assert_eq!(updated.status, JobStatus::Completed);
    assert_eq!(updated.result, Some("remote result".to_string()));
    assert!(updated.completed_at.is_some());
}

// ============================================================================
// TEST 3: Job Manager - Cancel Job
// ============================================================================

#[tokio::test]
async fn test_job_manager_cancel() {
    let metrics = Arc::new(MetricsCollector::new());
    let job_manager = JobManager::new("test-node", 4, metrics);

    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("long.bat");
    #[cfg(windows)]
    std::fs::write(&script, "@echo off\r\nping -n 31 127.0.0.1 > nul").unwrap();
    #[cfg(not(windows))]
    {
        let script = temp.path().join("long.sh");
        std::fs::write(&script, "#!/bin/sh\nsleep 30").unwrap();
    }

    let job = make_script_job("cancel-test", script.to_str().unwrap());

    // Submit long-running job
    job_manager.submit_job(job.clone(), temp.path()).await.unwrap();

    // Wait a bit for job to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Cancel
    let result = job_manager.cancel_job(&job.id).await;
    assert!(result.is_ok());

    // Verify cancelled
    tokio::time::sleep(Duration::from_millis(100)).await;
    let cancelled = job_manager.get_job(&job.id).await;
    assert!(cancelled.is_some());
    assert_eq!(cancelled.unwrap().status, JobStatus::Cancelled);
}

// ============================================================================
// TEST 4: File Transfer Manager - Build Manifest
// ============================================================================

#[tokio::test]
async fn test_file_sync_build_manifest() {
    let temp = tempfile::tempdir().unwrap();

    // Create test files
    std::fs::write(temp.path().join("file1.txt"), "content1").unwrap();
    std::fs::write(temp.path().join("file2.txt"), "content2").unwrap();
    std::fs::create_dir_all(temp.path().join("subdir")).unwrap();
    std::fs::write(temp.path().join("subdir").join("file3.txt"), "content3").unwrap();

    let manifest = FileTransferManager::build_manifest(temp.path()).await;

    assert!(manifest.is_ok());
    let manifest = manifest.unwrap();
    assert_eq!(manifest.len(), 3, "Should find 3 files");

    // Verify all files are present (normalize path separators)
    let paths: Vec<String> = manifest.iter().map(|f| {
        f.relative_path.replace('\\', "/")
    }).collect();
    assert!(paths.iter().any(|p| p == "file1.txt"), "Should contain file1.txt");
    assert!(paths.iter().any(|p| p == "file2.txt"), "Should contain file2.txt");
    assert!(paths.iter().any(|p| p.ends_with("file3.txt")), "Should contain file3.txt");

    // Verify sizes
    for entry in &manifest {
        assert!(entry.size > 0);
        assert!(!entry.blake3_hash.is_empty());
    }
}

// ============================================================================
// TEST 5: File Transfer Manager - Diff Manifests
// ============================================================================

#[tokio::test]
async fn test_file_sync_diff_manifests() {
    let local_files = vec![
        FileManifestEntry {
            relative_path: "file1.txt".to_string(),
            size: 100,
            blake3_hash: "hash1".to_string(),
            modified_at: chrono::Utc::now(),
        },
        FileManifestEntry {
            relative_path: "file2.txt".to_string(),
            size: 200,
            blake3_hash: "hash2".to_string(),
            modified_at: chrono::Utc::now(),
        },
    ];

    let remote_files = vec![
        FileManifestEntry {
            relative_path: "file1.txt".to_string(),
            size: 100,
            blake3_hash: "hash1".to_string(),
            modified_at: chrono::Utc::now(),
        },
        FileManifestEntry {
            relative_path: "file3.txt".to_string(),
            size: 300,
            blake3_hash: "hash3".to_string(),
            modified_at: chrono::Utc::now(),
        },
    ];

    let needed = FileTransferManager::diff_manifests(&local_files, &remote_files);

    // file2.txt exists locally but not remotely, so it's needed for sync
    assert!(needed.contains(&"file2.txt".to_string()));
    // file1.txt is the same, not needed
    assert!(!needed.contains(&"file1.txt".to_string()));
}

// ============================================================================
// TEST 6: Protocol Message Serialization
// ============================================================================

#[test]
fn test_protocol_message_serialization() {
    let job = make_test_job("proto-test", "echo hello");
    let msg = job_submit_message("node1", job.clone());

    // Serialize
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("JobSubmit"));
    assert!(json.contains("proto-test"));

    // Deserialize
    let deserialized: ProtocolMessage = serde_json::from_str(&json).unwrap();
    match deserialized.msg_type {
        MessageType::JobSubmit => {}
        _ => panic!("Expected JobSubmit message type"),
    }
    assert_eq!(deserialized.sender_id, "node1");
}

// ============================================================================
// TEST 7: JobCompleted Message Handling
// ============================================================================

#[test]
fn test_job_completed_message() {
    let msg = job_completed_message("node2", "job-123", "result data");

    match msg.msg_type {
        MessageType::JobCompleted => {}
        _ => panic!("Expected JobCompleted message type"),
    }
    match &msg.payload {
        Payload::JobCompleted { job_id, result } => {
            assert_eq!(job_id, "job-123");
            assert_eq!(result, "result data");
        }
        _ => panic!("Expected JobCompleted payload"),
    }
}

// ============================================================================
// TEST 8: Memory Pool Creation
// ============================================================================

#[tokio::test]
async fn test_memory_pool_operations() {
    // Create memory pool (via daemon state simulation)
    let mut memory_pools = HashMap::new();

    let pool = MemoryPool {
        id: uuid::Uuid::new_v4().to_string(),
        name: "test-pool".to_string(),
        total_pages: 1024, // 4MB with 4KB pages
        page_size_bytes: 4096,
        owner_nodes: vec![NodeId::from("test-node")],
        created_at: chrono::Utc::now(),
    };

    memory_pools.insert(pool.id.clone(), pool.clone());

    assert_eq!(memory_pools.len(), 1);
    assert_eq!(memory_pools.get(&pool.id).unwrap().name, "test-pool");
    assert_eq!(pool.total_size_bytes(), 1024 * 4096);
}

// ============================================================================
// TEST 9: DistributedBuffer Operations
// ============================================================================

#[tokio::test]
async fn test_distributed_buffer_read_write() {
    let buffer = DistributedBuffer::new("test-buffer", 4096, "node1");

    // Write a page
    let data = bytes::Bytes::from_static(b"Hello, distributed memory!");
    buffer.write_page(0, data.clone()).await.unwrap();

    // Read it back
    let read_data = buffer.read_page(0).await.unwrap();
    assert_eq!(read_data, data);

    // Stats
    let stats = buffer.stats().await;
    assert!(stats.total_pages >= 1);
    assert!(stats.cached_pages >= 1);
}

// ============================================================================
// TEST 10: DistributedBuffer - Read/Write at Offset
// ============================================================================

#[tokio::test]
async fn test_distributed_buffer_offset_operations() {
    // Create buffer with 2 pages (8KB)
    let buffer = DistributedBuffer::new("offset-buffer", 8192, "node1");

    // Write within first page
    let data = b"offset data";
    buffer.write(0, data).await.unwrap();

    // Read back
    let read = buffer.read(0, data.len() as u64).await.unwrap();
    assert_eq!(read.as_ref(), data);
}

// ============================================================================
// TEST 11: Job Requirements - Default Values
// ============================================================================

#[test]
fn test_job_requirements_defaults() {
    let req = JobRequirements::default();
    assert_eq!(req.min_ram_bytes, 1024 * 1024 * 1024); // 1GB
    assert_eq!(req.min_cpu_cores, 1);
    assert!(!req.requires_gpu);
    assert!(req.min_vram_bytes.is_none());
    assert!(req.preferred_nodes.is_empty());
}

// ============================================================================
// TEST 12: Job Builder Pattern
// ============================================================================

#[test]
fn test_job_builder() {
    let job = Job::new(
        "builder-test".to_string(),
        JobType::Python,
        JobRequirements::default(),
    )
    .with_script_path("/path/to/script.py".to_string())
    .with_args(vec!["--verbose".to_string(), "--output".to_string(), "result.json".to_string()])
    .with_env("PYTHONPATH".to_string(), "/opt/libs".to_string());

    assert_eq!(job.name, "builder-test");
    assert_eq!(job.job_type, JobType::Python);
    assert_eq!(job.script_path, Some("/path/to/script.py".to_string()));
    assert_eq!(job.args, vec!["--verbose", "--output", "result.json"]);
    assert_eq!(job.env.get("PYTHONPATH"), Some(&"/opt/libs".to_string()));
}

// ============================================================================
// TEST 13: Metrics Collector - Job Tracking
// ============================================================================

#[test]
fn test_metrics_job_tracking() {
    let metrics = MetricsCollector::new();

    metrics.record_job_submitted();
    metrics.record_job_submitted();
    metrics.record_job_completed();
    metrics.record_job_failed();

    let snapshot = metrics.get_snapshot();
    assert_eq!(snapshot.jobs_submitted, 2);
    assert_eq!(snapshot.jobs_completed, 1);
    assert_eq!(snapshot.jobs_failed, 1);
}

// ============================================================================
// TEST 14: File Sync - Handle Manifest (Determine Needed Files)
// ============================================================================

#[tokio::test]
async fn test_file_sync_handle_manifest() {
    let temp = tempfile::tempdir().unwrap();

    // Create local files
    std::fs::write(temp.path().join("existing.txt"), "local content").unwrap();
    std::fs::write(temp.path().join("same.txt"), "same content").unwrap();

    // Remote manifest with different files
    let remote_files = vec![
        FileManifestEntry {
            relative_path: "existing.txt".to_string(),
            size: 13,
            blake3_hash: "different_hash".to_string(), // Different hash = needs sync
            modified_at: chrono::Utc::now(),
        },
        FileManifestEntry {
            relative_path: "new_file.txt".to_string(),
            size: 10,
            blake3_hash: "new_hash".to_string(),
            modified_at: chrono::Utc::now(),
        },
    ];

    let needed = FileTransferManager::handle_manifest(
        "transfer-1",
        temp.path().to_str().unwrap(),
        &remote_files,
        temp.path(),
    ).await;

    // existing.txt has different hash, should be needed
    // new_file.txt doesn't exist locally, should be needed
    assert!(needed.contains(&"existing.txt".to_string()));
    assert!(needed.contains(&"new_file.txt".to_string()));
}

// ============================================================================
// TEST 15: Node Scoring
// ============================================================================

#[test]
fn test_node_scoring() {
    let node = NodeState {
        info: NodeInfo {
            node_id: NodeId::from("node1"),
            name: "test-node".to_string(),
            hostname: "test-host".to_string(),
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            ip_address: Some("192.168.1.100".to_string()),
            created_at: chrono::Utc::now(),
        },
        status: NodeStatus::Idle,
        resources: Some(ResourceSnapshot {
            node_id: NodeId::from("node1"),
            timestamp: chrono::Utc::now(),
            cpu: CpuInfo {
                cores: 8,
                load_percent: 20.0,
                frequency_mhz: 3200,
            },
            memory: MemoryInfo {
                total_bytes: 16 * 1024 * 1024 * 1024,
                available_bytes: 12 * 1024 * 1024 * 1024,
                used_bytes: 4 * 1024 * 1024 * 1024,
            },
            gpu: Some(GpuInfo {
                name: "RTX 4090".to_string(),
                vram_total_bytes: 24 * 1024 * 1024 * 1024,
                vram_used_bytes: 2 * 1024 * 1024 * 1024,
                vram_available_bytes: 22 * 1024 * 1024 * 1024,
                utilization_percent: 15.0,
                temperature_c: Some(45.0),
            }),
            disk: DiskInfo {
                total_bytes: 512 * 1024 * 1024 * 1024,
                free_bytes: 400 * 1024 * 1024 * 1024,
            },
            network: NetworkInfo {
                bytes_sent: 1000000,
                bytes_received: 5000000,
            },
            idle_seconds: 600,
            is_charging: Some(true),
            battery_percent: Some(85.0),
            temperature_c: Some(42.0),
        }),
        last_heartbeat: chrono::Utc::now(),
    };

    assert!(node.is_available());
    let score = node.score();
    assert!(score > 0.0, "Node should have positive score");
}

// ============================================================================
// TEST 16: IPC Command Serialization
// ============================================================================

#[test]
fn test_ipc_command_serialization() {
    let cmd = IpcCommand::RunJob {
        name: "test-run".to_string(),
        job_type: "python".to_string(),
        script_path: Some("/path/to/script.py".to_string()),
        script_content: None,
        args: vec!["--input".to_string(), "data.csv".to_string()],
        ram: Some(4 * 1024 * 1024 * 1024),
        cpu: Some(4),
        node: Some("gpu-node".to_string()),
        requires_gpu: true,
        min_vram_bytes: Some(8 * 1024 * 1024 * 1024),
        max_duration_secs: Some(3600),
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: IpcCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        IpcCommand::RunJob { name, job_type, .. } => {
            assert_eq!(name, "test-run");
            assert_eq!(job_type, "python");
        }
        _ => panic!("Expected RunJob"),
    }
}

// ============================================================================
// TEST 17: File Sync - Build and Compare Manifests
// ============================================================================

#[tokio::test]
async fn test_file_sync_full_workflow() {
    let source_dir = tempfile::tempdir().unwrap();
    let target_dir = tempfile::tempdir().unwrap();

    // Create source files
    std::fs::write(source_dir.path().join("a.txt"), "aaa").unwrap();
    std::fs::write(source_dir.path().join("b.txt"), "bbb").unwrap();

    // Create target files (one different, one missing)
    std::fs::write(target_dir.path().join("a.txt"), "different").unwrap();

    // Build manifests
    let source_manifest = FileTransferManager::build_manifest(source_dir.path()).await.unwrap();
    let target_manifest = FileTransferManager::build_manifest(target_dir.path()).await.unwrap();

    // diff_manifests(local, remote) returns files in local that differ from remote
    // For syncing source -> target: local=source, remote=target
    let needed = FileTransferManager::diff_manifests(&source_manifest, &target_manifest);

    // a.txt exists in both but content differs (different blake3 hash)
    // b.txt exists in source but not in target
    assert!(needed.contains(&"a.txt".to_string()), "a.txt should be needed (different content), got: {:?}", needed);
    assert!(needed.contains(&"b.txt".to_string()), "b.txt should be needed (missing), got: {:?}", needed);
}

// ============================================================================
// TEST 18: Memory Page Write and Ack
// ============================================================================

#[test]
fn test_memory_page_write_and_ack() {
    // Simulate memory page write message
    let msg = ProtocolMessage::new(
        "node1",
        MessageType::MemoryPageWrite,
        Payload::MemoryPageWrite {
            page_id: 0,
            pool_id: "pool-1".to_string(),
            data: vec![1, 2, 3, 4],
            compressed: false,
            encrypted: false,
        },
    );

    // Verify serialization
    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: ProtocolMessage = serde_json::from_str(&json).unwrap();

    match deserialized.payload {
        Payload::MemoryPageWrite { page_id, pool_id, data, .. } => {
            assert_eq!(page_id, 0);
            assert_eq!(pool_id, "pool-1");
            assert_eq!(data, vec![1, 2, 3, 4]);
        }
        _ => panic!("Expected MemoryPageWrite"),
    }
}

// ============================================================================
// TEST 19: Execution Result
// ============================================================================

#[tokio::test]
async fn test_execution_result() {
    let metrics = Arc::new(MetricsCollector::new());
    let job_manager = JobManager::new("test-node", 4, metrics);

    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("result_test.bat");
    #[cfg(windows)]
    std::fs::write(&script, "@echo off\r\necho hello").unwrap();
    #[cfg(not(windows))]
    {
        let script = temp.path().join("result_test.sh");
        std::fs::write(&script, "#!/bin/sh\necho hello").unwrap();
    }

    let job = make_script_job("result-test", script.to_str().unwrap());
    job_manager.submit_job(job.clone(), temp.path()).await.unwrap();

    // Wait for completion with retries
    let mut attempts = 0;
    loop {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let j = job_manager.get_job(&job.id).await.unwrap();
        if j.status == JobStatus::Completed || j.status == JobStatus::Failed {
            break;
        }
        attempts += 1;
        if attempts > 20 {
            panic!("Job did not complete within 4 seconds, last status: {:?}", j.status);
        }
    }

    let final_job = job_manager.get_job(&job.id).await.unwrap();
    assert!(
        final_job.status == JobStatus::Completed || final_job.status == JobStatus::Failed,
        "Job should complete or fail, got {:?}",
        final_job.status
    );
    // Result or error should be set
    assert!(
        final_job.result.is_some() || final_job.error.is_some(),
        "Either result or error should be set"
    );
}

// ============================================================================
// TEST 20: Protocol Messages for File Sync
// ============================================================================

#[test]
fn test_file_sync_messages() {
    let msg = file_manifest_message(
        "node1",
        "transfer-123",
        "/path/to/files",
        vec![
            FileManifestEntry {
                relative_path: "a.txt".to_string(),
                size: 100,
                blake3_hash: "hash-a".to_string(),
                modified_at: chrono::Utc::now(),
            },
        ],
    );

    match msg.payload {
        Payload::FileManifest { transfer_id, root_path, files } => {
            assert_eq!(transfer_id, "transfer-123");
            assert_eq!(root_path, "/path/to/files");
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].relative_path, "a.txt");
        }
        _ => panic!("Expected FileManifest"),
    }
}
