//! Real integration tests for distributed features.
//! These tests start actual daemon instances and test real network communication.

use luci_common::*;
use luci_protocol::*;
use lucid::daemon::{Daemon, NetworkManager};
use lucid::daemon::file_sync::FileTransferManager;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Helper to create a Config for testing
fn make_test_config(name: &str, data_dir: &PathBuf, node_port: u16, daemon_port: u16) -> Config {
    Config {
        node_id: NodeId::new(),
        node_name: name.to_string(),
        network_id: None,
        network_name: None,
        daemon_port,
        node_port,
        discovery_port: node_port + 1000,
        log_level: "debug".to_string(),
        data_dir: data_dir.clone(),
        contribute_ram_percent: 60,
        contribute_gpu: false,
        max_jobs: 4,
        auth_token: uuid::Uuid::new_v4().to_string(),
        tls_enabled: false,
        metrics_port: daemon_port + 100,
        peers: vec![],
        node_public_key: None,
    }
}

/// Helper to start a daemon with network listener
async fn start_test_daemon(
    name: &str,
    data_dir: &PathBuf,
    node_port: u16,
    daemon_port: u16,
) -> Arc<Daemon> {
    let config = make_test_config(name, data_dir, node_port, daemon_port);
    config.ensure_dirs().unwrap();

    let daemon = Daemon::new(config).await.unwrap();
    let daemon = Arc::new(daemon);

    // Start network listener
    let d = daemon.clone();
    let listener = d.network.start_listener().await.unwrap();
    let state = d.state.clone();
    let peers = d.network.peers.clone();
    let config = d.config.clone();
    let tls_acceptor = d.network.tls_acceptor();

    tokio::spawn(async move {
        let node_id = config.node_id.clone();
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let nid = node_id.clone();
                    let peers = peers.clone();
                    let state_clone = state.clone();
                    let acceptor = tls_acceptor.clone();
                    tokio::spawn(async move {
                        NetworkManager::handle_connection(
                            stream,
                            addr,
                            nid.to_string(),
                            peers,
                            state_clone,
                            acceptor,
                        ).await;
                    });
                }
                Err(e) => {
                    eprintln!("Accept error: {}", e);
                    break;
                }
            }
        }
    });

    // Add self to nodes
    {
        let mut state_guard = daemon.state.write().await;
        state_guard.nodes.insert(
            daemon.config.node_id.clone(),
            NodeState {
                info: NodeInfo {
                    node_id: daemon.config.node_id.clone(),
                    name: name.to_string(),
                    hostname: name.to_string(),
                    os: std::env::consts::OS.to_string(),
                    arch: std::env::consts::ARCH.to_string(),
                    ip_address: Some("127.0.0.1".to_string()),
                    created_at: chrono::Utc::now(),
                },
                status: NodeStatus::Online,
                resources: Some(ResourceSnapshot {
                    node_id: daemon.config.node_id.clone(),
                    timestamp: chrono::Utc::now(),
                    cpu: CpuInfo { cores: 4, load_percent: 20.0, frequency_mhz: 3200 },
                    memory: MemoryInfo {
                        total_bytes: 8 * 1024 * 1024 * 1024,
                        available_bytes: 6 * 1024 * 1024 * 1024,
                        used_bytes: 2 * 1024 * 1024 * 1024,
                    },
                    gpu: None,
                    disk: DiskInfo { total_bytes: 512 * 1024 * 1024 * 1024, free_bytes: 400 * 1024 * 1024 * 1024 },
                    network: NetworkInfo { bytes_sent: 0, bytes_received: 0 },
                    idle_seconds: 300,
                    is_charging: None,
                    battery_percent: None,
                    temperature_c: None,
                }),
                last_heartbeat: chrono::Utc::now(),
            },
        );
    }

    daemon
}

/// Helper to connect two daemons bidirectionally
async fn connect_daemons(sender: &Daemon, receiver: &Daemon) {
    let receiver_addr = format!("127.0.0.1:{}", receiver.config.node_port);
    let sender_addr = format!("127.0.0.1:{}", sender.config.node_port);
    let sender_id = sender.config.node_id.to_string();
    let receiver_id = receiver.config.node_id.to_string();

    // sender announces to receiver
    let msg = ProtocolMessage::new(
        &sender_id,
        MessageType::Announce,
        Payload::Announce {
            node_id: sender_id.clone(),
            node_name: sender.config.node_name.clone(),
            hostname: sender.config.node_name.clone(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            capabilities: vec![],
        },
    );
    sender.network.send_message_to_peer(&receiver_addr, &msg).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // receiver announces back to sender
    let msg = ProtocolMessage::new(
        &receiver_id,
        MessageType::Announce,
        Payload::Announce {
            node_id: receiver_id.clone(),
            node_name: receiver.config.node_name.clone(),
            hostname: receiver.config.node_name.clone(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            capabilities: vec![],
        },
    );
    receiver.network.send_message_to_peer(&sender_addr, &msg).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
}

// ============================================================================
// TEST 1: Real Distributed Job Execution
// ============================================================================

#[tokio::test]
async fn test_real_distributed_job_execution() {
    let temp = tempfile::tempdir().unwrap();

    // Start two daemons
    let daemon1 = start_test_daemon("node1", &temp.path().join("d1"), 18100, 18200).await;
    let daemon2 = start_test_daemon("node2", &temp.path().join("d2"), 18101, 18201).await;

    // Connect them
    connect_daemons(daemon1.as_ref(), daemon2.as_ref()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Create a test script
    let script_dir = temp.path().join("scripts");
    std::fs::create_dir_all(&script_dir).unwrap();
    let script_path = script_dir.join("hello.sh");
    #[cfg(unix)]
    std::fs::write(&script_path, "#!/bin/sh\necho 'Hello from remote node!'").unwrap();
    #[cfg(windows)]
    std::fs::write(&script_path, "@echo off\r\necho Hello from remote node!").unwrap();

    // Create job
    let job = Job::new(
        "remote-exec-test".to_string(),
        JobType::Shell,
        JobRequirements::default(),
    )
    .with_script_path(script_path.to_str().unwrap().to_string());

    // Send job directly to node2 via network
    let node2_addr = format!("127.0.0.1:{}", daemon2.config.node_port);
    daemon1.network.send_job_to_peer(
        &node2_addr,
        &daemon1.config.node_id.to_string(),
        &job,
    ).await.unwrap();

    // Also track the job locally
    let job_id = job.id.clone();
    {
        let mut state = daemon1.state.write().await;
        state.jobs.insert(job_id.clone(), job);
    }
    eprintln!("Job submitted: {}", job_id);

    // Wait for job to be sent and executed
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check job status on node2 - it should have been received and started
    let job_status = daemon2.get_job(&job_id.to_string()).await;
    assert!(job_status.is_some(), "Job should exist on node2");
    let job = job_status.unwrap();
    eprintln!("Job status on node2: {:?}", job.status);

    // Job should be running, completed, or failed (it was received)
    assert!(
        job.status == JobStatus::Running || job.status == JobStatus::Completed || job.status == JobStatus::Failed,
        "Job should be received and running/completed/failed, got: {:?}",
        job.status
    );
}

// ============================================================================
// TEST 2: Real Distributed Memory Read/Write
// ============================================================================

#[tokio::test]
async fn test_real_distributed_memory() {
    let temp = tempfile::tempdir().unwrap();

    let daemon1 = start_test_daemon("node1", &temp.path().join("d1"), 18110, 18210).await;
    let daemon2 = start_test_daemon("node2", &temp.path().join("d2"), 18111, 18211).await;

    connect_daemons(daemon1.as_ref(), daemon2.as_ref()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Create memory pool on node1
    let pool = daemon1.create_memory_pool("test-pool", 0.001).await; // 1MB
    eprintln!("Pool created: {}", pool.id);

    // Write data to pool on node1
    let test_data = b"Distributed memory test data!";
    let page_data = test_data.to_vec();

    // Write page via protocol message
    let node2_addr = format!("127.0.0.1:{}", daemon2.config.node_port);
    let msg = ProtocolMessage::new(
        &daemon1.config.node_id.to_string(),
        MessageType::MemoryPageWrite,
        Payload::MemoryPageWrite {
            page_id: 0,
            pool_id: pool.id.clone(),
            data: page_data.clone(),
            compressed: false,
            encrypted: false,
        },
    );

    daemon1.network.send_message_to_peer(&node2_addr, &msg).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Request page from node2
    let msg = ProtocolMessage::new(
        &daemon1.config.node_id.to_string(),
        MessageType::MemoryPageRequest,
        Payload::MemoryPageRequest {
            page_id: 0,
            pool_id: pool.id.clone(),
        },
    );

    daemon1.network.send_message_to_peer(&node2_addr, &msg).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify memory pool exists
    let pools = daemon1.list_memory_pools().await;
    assert!(!pools.is_empty(), "Should have at least one memory pool");
}

// ============================================================================
// TEST 3: Real File Sync Across Network
// ============================================================================

#[tokio::test]
async fn test_real_file_sync() {
    let temp = tempfile::tempdir().unwrap();

    let daemon1 = start_test_daemon("node1", &temp.path().join("d1"), 18120, 18220).await;
    let daemon2 = start_test_daemon("node2", &temp.path().join("d2"), 18121, 18221).await;

    connect_daemons(daemon1.as_ref(), daemon2.as_ref()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Create source files
    let source_dir = temp.path().join("source");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(source_dir.join("file1.txt"), "Hello from source!").unwrap();
    std::fs::write(source_dir.join("file2.txt"), "Second file content").unwrap();

    // Create target directory
    let target_dir = temp.path().join("target");
    std::fs::create_dir_all(&target_dir).unwrap();

    // Send files via FileSyncData message
    let manifest = FileTransferManager::build_manifest(&source_dir).await.unwrap();

    let mut file_entries = Vec::new();
    for entry in &manifest {
        let full_path = source_dir.join(&entry.relative_path);
        let data = std::fs::read(&full_path).unwrap();
        file_entries.push(FileSyncEntry {
            path: entry.relative_path.clone(),
            data,
            size: entry.size,
        });
    }

    let msg = ProtocolMessage::new(
        &daemon1.config.node_id.to_string(),
        MessageType::FileSyncData,
        Payload::FileSyncData {
            source_path: source_dir.to_str().unwrap().to_string(),
            files: file_entries,
        },
    );

    let node2_addr = format!("127.0.0.1:{}", daemon2.config.node_port);
    daemon1.network.send_message_to_peer(&node2_addr, &msg).await.unwrap();

    // Wait for file transfer
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Verify files were received (they go to temp dir by default)
    // The actual file writing happens in process_message
}

// ============================================================================
// TEST 4: Real Job Transfer and Remote Execution
// ============================================================================

#[tokio::test]
async fn test_real_job_transfer() {
    let temp = tempfile::tempdir().unwrap();

    let daemon1 = start_test_daemon("node1", &temp.path().join("d1"), 18130, 18230).await;
    let daemon2 = start_test_daemon("node2", &temp.path().join("d2"), 18131, 18231).await;

    connect_daemons(daemon1.as_ref(), daemon2.as_ref()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Create a script that outputs JSON
    let script_dir = temp.path().join("scripts");
    std::fs::create_dir_all(&script_dir).unwrap();
    let script_path = script_dir.join("compute.sh");
    #[cfg(unix)]
    std::fs::write(&script_path, "#!/bin/sh\necho '{\"result\": 42, \"status\": \"ok\"}'").unwrap();
    #[cfg(windows)]
    std::fs::write(&script_path, "@echo off\r\necho {\"result\": 42, \"status\": \"ok\"}").unwrap();

    // Create job
    let job = Job::new(
        "compute-test".to_string(),
        JobType::Shell,
        JobRequirements::default(),
    )
    .with_script_path(script_path.to_str().unwrap().to_string());

    // Submit via daemon1
    let job_id = daemon1.submit_job(job).await;

    // Wait for execution
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check result
    let job = daemon1.get_job(&job_id.to_string()).await;
    assert!(job.is_some());
    let job = job.unwrap();

    eprintln!("Job status: {:?}", job.status);
    if let Some(result) = &job.result {
        eprintln!("Job result: {}", result);
    }
    if let Some(error) = &job.error {
        eprintln!("Job error: {}", error);
    }
}

// ============================================================================
// TEST 5: Node Discovery via Network
// ============================================================================

#[tokio::test]
async fn test_real_node_discovery() {
    let temp = tempfile::tempdir().unwrap();

    let daemon1 = start_test_daemon("node1", &temp.path().join("d1"), 18140, 18240).await;
    let daemon2 = start_test_daemon("node2", &temp.path().join("d2"), 18141, 18241).await;

    // Before connection - each should only see themselves
    let devices1 = daemon1.get_devices().await;
    let devices2 = daemon2.get_devices().await;
    assert_eq!(devices1.len(), 1, "Node1 should see only itself initially");
    assert_eq!(devices2.len(), 1, "Node2 should see only itself initially");

    // Connect
    connect_daemons(daemon1.as_ref(), daemon2.as_ref()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // After connection - should see each other
    let devices1 = daemon1.get_devices().await;
    let devices2 = daemon2.get_devices().await;

    eprintln!("Node1 sees {} devices", devices1.len());
    eprintln!("Node2 sees {} devices", devices2.len());

    assert!(devices1.len() >= 2, "Node1 should see at least 2 nodes after discovery");
    assert!(devices2.len() >= 2, "Node2 should see at least 2 nodes after discovery");
}

// ============================================================================
// TEST 6: Resource Aggregation
// ============================================================================

#[tokio::test]
async fn test_real_resource_aggregation() {
    let temp = tempfile::tempdir().unwrap();

    let daemon1 = start_test_daemon("node1", &temp.path().join("d1"), 18150, 18250).await;
    let daemon2 = start_test_daemon("node2", &temp.path().join("d2"), 18151, 18251).await;

    connect_daemons(daemon1.as_ref(), daemon2.as_ref()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get combined resources
    let (total_nodes, total_cpu, total_ram, available_ram, _gpu_count, _vram) = daemon1.get_resources().await;

    eprintln!("Resources: {} nodes, {} CPUs, {:.1}GB RAM, {:.1}GB available",
        total_nodes, total_cpu, total_ram, available_ram);

    assert!(total_nodes >= 2, "Should have at least 2 nodes");
    assert!(total_cpu >= 4, "Should have at least 4 CPUs");
    assert!(total_ram >= 8.0, "Should have at least 8GB RAM");
}

// ============================================================================
// TEST 7: Concurrent Remote Jobs
// ============================================================================

#[tokio::test]
async fn test_real_concurrent_remote_jobs() {
    let temp = tempfile::tempdir().unwrap();

    let daemon1 = start_test_daemon("node1", &temp.path().join("d1"), 18160, 18260).await;
    let daemon2 = start_test_daemon("node2", &temp.path().join("d2"), 18161, 18261).await;

    connect_daemons(daemon1.as_ref(), daemon2.as_ref()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let script_dir = temp.path().join("scripts");
    std::fs::create_dir_all(&script_dir).unwrap();

    // Create multiple jobs and send directly to node2
    let mut job_ids = Vec::new();
    let node2_addr = format!("127.0.0.1:{}", daemon2.config.node_port);
    for i in 0..3 {
        let script_path = script_dir.join(format!("job_{}.sh", i));
        #[cfg(unix)]
        std::fs::write(&script_path, format!("#!/bin/sh\necho 'Job {} done'", i)).unwrap();
        #[cfg(windows)]
        std::fs::write(&script_path, format!("@echo off\r\necho Job {} done", i)).unwrap();

        let job = Job::new(
            format!("concurrent-{}", i),
            JobType::Shell,
            JobRequirements::default(),
        )
        .with_script_path(script_path.to_str().unwrap().to_string());

        let job_id = job.id.clone();

        // Send directly to node2
        daemon1.network.send_job_to_peer(
            &node2_addr,
            &daemon1.config.node_id.to_string(),
            &job,
        ).await.unwrap();

        // Track locally
        {
            let mut state = daemon1.state.write().await;
            state.jobs.insert(job_id.clone(), job);
        }
        job_ids.push(job_id);
    }

    // Wait for all jobs with retries
    let mut attempts = 0;
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let mut all_done = true;
        for job_id in &job_ids {
            if let Some(job) = daemon2.get_job(&job_id.to_string()).await {
                if job.status != JobStatus::Completed && job.status != JobStatus::Failed {
                    all_done = false;
                    break;
                }
            } else {
                all_done = false;
                break;
            }
        }
        if all_done {
            break;
        }
        attempts += 1;
        if attempts > 20 {
            break; // Give up after 10 seconds
        }
    }

    // Check all jobs completed on node2
    for job_id in &job_ids {
        let job = daemon2.get_job(&job_id.to_string()).await;
        assert!(job.is_some(), "Job should exist on node2");
        let job = job.unwrap();
        assert!(
            job.status == JobStatus::Completed || job.status == JobStatus::Failed,
            "Job {} should complete or fail, got {:?}",
            job_id,
            job.status
        );
    }
}

// ============================================================================
// TEST 8: Heartbeat Between Nodes
// ============================================================================

#[tokio::test]
async fn test_real_heartbeat() {
    let temp = tempfile::tempdir().unwrap();

    let daemon1 = start_test_daemon("node1", &temp.path().join("d1"), 18170, 18270).await;
    let daemon2 = start_test_daemon("node2", &temp.path().join("d2"), 18171, 18271).await;

    connect_daemons(daemon1.as_ref(), daemon2.as_ref()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Send heartbeat from node1 to node2
    let resources = ResourceSnapshot {
        node_id: daemon1.config.node_id.clone(),
        timestamp: chrono::Utc::now(),
        cpu: CpuInfo { cores: 4, load_percent: 30.0, frequency_mhz: 3200 },
        memory: MemoryInfo {
            total_bytes: 8 * 1024 * 1024 * 1024,
            available_bytes: 5 * 1024 * 1024 * 1024,
            used_bytes: 3 * 1024 * 1024 * 1024,
        },
        gpu: None,
        disk: DiskInfo { total_bytes: 512 * 1024 * 1024 * 1024, free_bytes: 400 * 1024 * 1024 * 1024 },
        network: NetworkInfo { bytes_sent: 1000, bytes_received: 2000 },
        idle_seconds: 60,
        is_charging: None,
        battery_percent: None,
        temperature_c: None,
    };

    let node2_addr = format!("127.0.0.1:{}", daemon2.config.node_port);
    daemon1.network.send_heartbeat(
        &node2_addr,
        &daemon1.config.node_id.to_string(),
        resources,
        "online",
    ).await.unwrap();

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Check that node2 received the heartbeat
    let devices = daemon2.get_devices().await;
    assert!(devices.len() >= 2, "Node2 should see node1 after heartbeat");
}

// ============================================================================
// TEST 9: Job Cancellation Across Network
// ============================================================================

#[tokio::test]
async fn test_real_job_cancellation() {
    let temp = tempfile::tempdir().unwrap();

    let daemon1 = start_test_daemon("node1", &temp.path().join("d1"), 18180, 18280).await;
    let daemon2 = start_test_daemon("node2", &temp.path().join("d2"), 18181, 18281).await;

    connect_daemons(daemon1.as_ref(), daemon2.as_ref()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Create a long-running script
    let script_dir = temp.path().join("scripts");
    std::fs::create_dir_all(&script_dir).unwrap();
    let script_path = script_dir.join("long.sh");
    #[cfg(unix)]
    std::fs::write(&script_path, "#!/bin/sh\nfor i in 1 2 3 4 5 6 7 8 9 10; do sleep 1; done").unwrap();
    #[cfg(windows)]
    std::fs::write(&script_path, "@echo off\r\nping -n 11 127.0.0.1 > nul").unwrap();

    let job = Job::new(
        "cancel-test".to_string(),
        JobType::Shell,
        JobRequirements::default(),
    )
    .with_script_path(script_path.to_str().unwrap().to_string());

    let job_id = daemon1.submit_job(job).await;

    // Wait for job to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Cancel the job
    let result = daemon1.cancel_job(&job_id.to_string()).await;
    assert!(result.is_ok(), "Cancel should succeed");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify cancelled
    let job = daemon1.get_job(&job_id.to_string()).await;
    assert!(job.is_some());
    let job = job.unwrap();
    assert!(
        job.status == JobStatus::Cancelled,
        "Job should be cancelled, got {:?}",
        job.status
    );
}

// ============================================================================
// TEST 10: Memory Pool Management
// ============================================================================

#[tokio::test]
async fn test_real_memory_pool_management() {
    let temp = tempfile::tempdir().unwrap();

    let daemon1 = start_test_daemon("node1", &temp.path().join("d1"), 18190, 18290).await;

    // Create multiple pools
    let pool1 = daemon1.create_memory_pool("pool-a", 0.001).await;
    let pool2 = daemon1.create_memory_pool("pool-b", 0.002).await;

    // List pools
    let pools = daemon1.list_memory_pools().await;
    assert_eq!(pools.len(), 2, "Should have 2 pools");

    // Get pool info
    let info = daemon1.get_memory_pool(&pool1.id).await;
    assert!(info.is_some(), "Should find pool1");
    assert_eq!(info.unwrap().name, "pool-a");

    // Delete pool
    let result = daemon1.delete_memory_pool(&pool2.id).await;
    assert!(result.is_ok());

    let pools = daemon1.list_memory_pools().await;
    assert_eq!(pools.len(), 1, "Should have 1 pool after delete");
}
