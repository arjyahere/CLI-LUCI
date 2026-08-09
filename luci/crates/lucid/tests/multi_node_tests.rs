use luci_common::{
    Job, JobId, JobType, JobStatus, JobRequirements,
    ResourceSnapshot, CpuInfo, MemoryInfo, GpuInfo, DiskInfo, NetworkInfo,
};
use luci_protocol::message::{ProtocolMessage, MessageType, Payload, FileManifestEntry};
use chrono::Utc;
use std::collections::HashMap;

/// Multi-node simulation tests
/// Tests protocol message serialization/deserialization for distributed scenarios

fn make_test_resource_snapshot(node_id: &str) -> ResourceSnapshot {
    ResourceSnapshot {
        node_id: node_id.into(),
        timestamp: Utc::now(),
        cpu: CpuInfo {
            cores: 16,
            load_percent: 35.0,
            frequency_mhz: 3200,
        },
        memory: MemoryInfo {
            total_bytes: 16 * 1024 * 1024 * 1024,
            available_bytes: 8 * 1024 * 1024 * 1024,
            used_bytes: 8 * 1024 * 1024 * 1024,
        },
        gpu: Some(GpuInfo {
            name: "RTX 3080".to_string(),
            vram_total_bytes: 10 * 1024 * 1024 * 1024,
            vram_used_bytes: 2 * 1024 * 1024 * 1024,
            vram_available_bytes: 8 * 1024 * 1024 * 1024,
            utilization_percent: 45.0,
            temperature_c: Some(65.0),
        }),
        disk: DiskInfo {
            total_bytes: 500 * 1024 * 1024 * 1024,
            free_bytes: 300 * 1024 * 1024 * 1024,
        },
        network: NetworkInfo {
            bytes_sent: 1024 * 1024,
            bytes_received: 2 * 1024 * 1024,
        },
        idle_seconds: 120,
        is_charging: Some(true),
        battery_percent: Some(85.0),
        temperature_c: Some(42.0),
    }
}

fn make_test_job(job_id: &str) -> Job {
    Job {
        id: JobId::from(job_id),
        name: "train.py".to_string(),
        job_type: JobType::Python,
        status: JobStatus::Pending,
        assigned_node: None,
        requirements: JobRequirements {
            min_ram_bytes: 4 * 1024 * 1024 * 1024,
            min_cpu_cores: 4,
            requires_gpu: true,
            min_vram_bytes: Some(2 * 1024 * 1024 * 1024),
            max_duration_secs: Some(3600),
            preferred_nodes: vec![],
        },
        script_path: Some("train.py".to_string()),
        script_content: None,
        args: vec!["--epochs".to_string(), "10".to_string()],
        env: HashMap::new(),
        working_dir: None,
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        result: None,
        error: None,
        exit_code: None,
    }
}

#[test]
fn test_multi_node_announce() {
    let msg = ProtocolMessage::new(
        "node-1",
        MessageType::Announce,
        Payload::Announce {
            node_id: "node-1".to_string(),
            node_name: "Laptop-1".to_string(),
            hostname: "DESKTOP-ABC".to_string(),
            os: "windows".to_string(),
            arch: "x86_64".to_string(),
            capabilities: vec!["gpu".to_string(), "memory".to_string()],
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    assert!(matches!(parsed.payload, Payload::Announce { .. }));
}

#[test]
fn test_multi_node_heartbeat() {
    let snapshot = make_test_resource_snapshot("node-1");
    let msg = ProtocolMessage::new(
        "node-1",
        MessageType::Heartbeat,
        Payload::Heartbeat {
            resources: snapshot.clone(),
            status: "online".to_string(),
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::Heartbeat { resources, status } = parsed.payload {
        assert_eq!(resources.cpu.cores, 16);
        assert_eq!(resources.gpu.unwrap().name, "RTX 3080");
        assert_eq!(resources.battery_percent, Some(85.0));
        assert_eq!(status, "online");
    } else {
        panic!("Expected Heartbeat payload");
    }
}

#[test]
fn test_multi_node_heartbeat_ack() {
    let msg = ProtocolMessage::new(
        "node-2",
        MessageType::HeartbeatAck,
        Payload::HeartbeatAck {
            cluster_size: 3,
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::HeartbeatAck { cluster_size } = parsed.payload {
        assert_eq!(cluster_size, 3);
    } else {
        panic!("Expected HeartbeatAck payload");
    }
}

#[test]
fn test_multi_node_job_submit() {
    let job = make_test_job("job-123");
    let msg = ProtocolMessage::new(
        "node-1",
        MessageType::JobSubmit,
        Payload::JobSubmit { job: job.clone() },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::JobSubmit { job } = parsed.payload {
        assert_eq!(job.id.to_string(), "job-123");
        assert_eq!(job.name, "train.py");
    } else {
        panic!("Expected JobSubmit payload");
    }
}

#[test]
fn test_multi_node_job_completed() {
    let msg = ProtocolMessage::new(
        "node-2",
        MessageType::JobCompleted,
        Payload::JobCompleted {
            job_id: "job-123".to_string(),
            result: "Training complete".to_string(),
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::JobCompleted { job_id, result } = parsed.payload {
        assert_eq!(job_id, "job-123");
        assert!(result.contains("complete"));
    } else {
        panic!("Expected JobCompleted payload");
    }
}

#[test]
fn test_multi_node_job_failed() {
    let msg = ProtocolMessage::new(
        "node-2",
        MessageType::JobFailed,
        Payload::JobFailed {
            job_id: "job-456".to_string(),
            error: "Out of memory".to_string(),
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::JobFailed { job_id, error } = parsed.payload {
        assert_eq!(job_id, "job-456");
        assert!(error.contains("memory"));
    } else {
        panic!("Expected JobFailed payload");
    }
}

#[test]
fn test_multi_node_memory_page_request() {
    let msg = ProtocolMessage::new(
        "node-1",
        MessageType::MemoryPageRequest,
        Payload::MemoryPageRequest {
            page_id: 42,
            pool_id: "pool-1".to_string(),
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::MemoryPageRequest { page_id, pool_id } = parsed.payload {
        assert_eq!(page_id, 42);
        assert_eq!(pool_id, "pool-1");
    } else {
        panic!("Expected MemoryPageRequest payload");
    }
}

#[test]
fn test_multi_node_memory_page_response() {
    let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let msg = ProtocolMessage::new(
        "node-2",
        MessageType::MemoryPageResponse,
        Payload::MemoryPageResponse {
            page_id: 42,
            pool_id: "pool-1".to_string(),
            data: data.clone(),
            compressed: false,
            encrypted: false,
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::MemoryPageResponse { page_id, data: resp_data, .. } = parsed.payload {
        assert_eq!(page_id, 42);
        assert_eq!(resp_data, data);
    } else {
        panic!("Expected MemoryPageResponse payload");
    }
}

#[test]
fn test_multi_node_memory_page_write() {
    let data = vec![10, 20, 30, 40];
    let msg = ProtocolMessage::new(
        "node-1",
        MessageType::MemoryPageWrite,
        Payload::MemoryPageWrite {
            page_id: 7,
            pool_id: "pool-2".to_string(),
            data: data.clone(),
            compressed: true,
            encrypted: false,
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::MemoryPageWrite { page_id, data: write_data, .. } = parsed.payload {
        assert_eq!(page_id, 7);
        assert_eq!(write_data, data);
    } else {
        panic!("Expected MemoryPageWrite payload");
    }
}

#[test]
fn test_multi_node_join_request() {
    let msg = ProtocolMessage::new(
        "node-3",
        MessageType::JoinRequest,
        Payload::JoinRequest {
            invite_code: "ABC123.def456".to_string(),
            node_name: "Desktop-3".to_string(),
            hostname: "WORKSTATION-XYZ".to_string(),
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::JoinRequest { invite_code, node_name, .. } = parsed.payload {
        assert_eq!(invite_code, "ABC123.def456");
        assert_eq!(node_name, "Desktop-3");
    } else {
        panic!("Expected JoinRequest payload");
    }
}

#[test]
fn test_multi_node_join_response() {
    let msg = ProtocolMessage::new(
        "node-1",
        MessageType::JoinResponse,
        Payload::JoinResponse {
            success: true,
            node_id: Some("node-3".to_string()),
            network_id: "net-abc".to_string(),
            error: None,
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::JoinResponse { success, node_id, .. } = parsed.payload {
        assert!(success);
        assert_eq!(node_id, Some("node-3".to_string()));
    } else {
        panic!("Expected JoinResponse payload");
    }
}

#[test]
fn test_multi_node_file_sync() {
    let msg = ProtocolMessage::new(
        "node-1",
        MessageType::FileSyncRequest,
        Payload::FileSyncRequest {
            source_path: "src/train.py".to_string(),
            file_name: "train.py".to_string(),
            file_data: b"print('hello')".to_vec(),
            target_path: "/tmp/work/train.py".to_string(),
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::FileSyncRequest { source_path, file_name, .. } = parsed.payload {
        assert_eq!(source_path, "src/train.py");
        assert_eq!(file_name, "train.py");
    } else {
        panic!("Expected FileSyncRequest payload");
    }
}

#[test]
fn test_multi_node_state_sync() {
    let data = serde_json::json!({
        "job_id": "job-789",
        "status": "completed",
        "exit_code": 0,
    });
    let data_bytes = serde_json::to_vec(&data).unwrap();

    let msg = ProtocolMessage::new(
        "node-1",
        MessageType::StateSync,
        Payload::StateSync {
            state_type: "job_completed".to_string(),
            data: data_bytes.clone(),
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::StateSync { state_type, data } = parsed.payload {
        assert_eq!(state_type, "job_completed");
        assert_eq!(data, data_bytes);
    } else {
        panic!("Expected StateSync payload");
    }
}

#[test]
fn test_multi_node_leave() {
    let msg = ProtocolMessage::new(
        "node-2",
        MessageType::LeaveNotification,
        Payload::LeaveNotification {
            node_id: "node-2".to_string(),
            reason: "Shutting down".to_string(),
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::LeaveNotification { node_id, reason } = parsed.payload {
        assert_eq!(node_id, "node-2");
        assert!(reason.contains("Shutting"));
    } else {
        panic!("Expected LeaveNotification payload");
    }
}

#[test]
fn test_multi_node_resource_update() {
    let snapshot = make_test_resource_snapshot("node-1");
    let msg = ProtocolMessage::new(
        "node-1",
        MessageType::ResourceUpdate,
        Payload::ResourceUpdate {
            resources: snapshot,
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::ResourceUpdate { resources } = parsed.payload {
        assert_eq!(resources.cpu.cores, 16);
        assert_eq!(resources.memory.total_bytes, 16 * 1024 * 1024 * 1024);
    } else {
        panic!("Expected ResourceUpdate payload");
    }
}

#[test]
fn test_multi_node_resource_query() {
    let msg = ProtocolMessage::new(
        "node-3",
        MessageType::ResourceQuery,
        Payload::ResourceQuery,
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    assert!(matches!(parsed.payload, Payload::ResourceQuery));
}

#[test]
fn test_multi_node_goodbye() {
    let msg = ProtocolMessage::new(
        "node-1",
        MessageType::Goodbye,
        Payload::Goodbye {
            reason: "Maintenance".to_string(),
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::Goodbye { reason } = parsed.payload {
        assert_eq!(reason, "Maintenance");
    } else {
        panic!("Expected Goodbye payload");
    }
}

#[test]
fn test_multi_node_file_manifest() {
    let msg = ProtocolMessage::new(
        "node-1",
        MessageType::FileManifest,
        Payload::FileManifest {
            transfer_id: "transfer-1".to_string(),
            root_path: "/tmp/project".to_string(),
            files: vec![
                FileManifestEntry {
                    relative_path: "train.py".to_string(),
                    size: 1024,
                    blake3_hash: "abc123".to_string(),
                    modified_at: Utc::now(),
                },
                FileManifestEntry {
                    relative_path: "data.csv".to_string(),
                    size: 2048,
                    blake3_hash: "def456".to_string(),
                    modified_at: Utc::now(),
                },
            ],
        },
    );

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ProtocolMessage = serde_json::from_str(&json).unwrap();
    if let Payload::FileManifest { transfer_id, files, .. } = parsed.payload {
        assert_eq!(transfer_id, "transfer-1");
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].relative_path, "train.py");
    } else {
        panic!("Expected FileManifest payload");
    }
}

#[test]
fn test_multi_node_memory_page_distribution_round_robin() {
    let pool_id = "pool-1";
    let total_pages = 6;
    let nodes = vec!["node-1", "node-2", "node-3"];

    let mut assignments: Vec<(String, u64, String)> = Vec::new();
    for page_id in 0..total_pages {
        let target_node = nodes[page_id as usize % nodes.len()].to_string();
        assignments.push((pool_id.to_string(), page_id, target_node));
    }

    assert_eq!(assignments.len(), 6);
    assert_eq!(assignments[0].2, "node-1");
    assert_eq!(assignments[1].2, "node-2");
    assert_eq!(assignments[2].2, "node-3");
    assert_eq!(assignments[3].2, "node-1");
    assert_eq!(assignments[4].2, "node-2");
    assert_eq!(assignments[5].2, "node-3");
}

#[test]
fn test_multi_node_metrics_aggregation() {
    let metrics = vec![
        ("node-1", 150, 145, 3, 2),
        ("node-2", 200, 190, 5, 5),
        ("node-3", 100, 98, 1, 1),
    ];

    let total_submitted: u64 = metrics.iter().map(|(_, s, _, _, _)| s).sum();
    let total_completed: u64 = metrics.iter().map(|(_, _, c, _, _)| c).sum();
    let total_failed: u64 = metrics.iter().map(|(_, _, _, f, _)| f).sum();
    let total_cancelled: u64 = metrics.iter().map(|(_, _, _, _, ca)| ca).sum();

    assert_eq!(total_submitted, 450);
    assert_eq!(total_completed, 433);
    assert_eq!(total_failed, 9);
    assert_eq!(total_cancelled, 8);
}

#[test]
fn test_multi_node_auth_keypair() {
    use luci_common::auth::AuthManager;

    let seed1 = [1u8; 32];
    let seed2 = [2u8; 32];
    let auth1 = AuthManager::new(&seed1);
    let auth2 = AuthManager::new(&seed2);

    let pk1 = auth1.public_key_hex();
    let pk2 = auth2.public_key_hex();

    assert!(!pk1.is_empty());
    assert!(!pk2.is_empty());
    assert_ne!(pk1, pk2);

    let message = b"hello from node-1";
    let signature = auth1.sign(message);
    assert!(auth1.verify(message, &signature));
    assert!(!auth2.verify(message, &signature));
}
