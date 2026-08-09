use luci_common::{Config, IpcCommand, IpcRequest, IpcResponse};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn send_request(port: u16, command: IpcCommand) -> Option<IpcResponse> {
    let request = IpcRequest::new(command);
    let request_json = serde_json::to_string(&request).ok()?;

    let addr = format!("127.0.0.1:{}", port);
    let mut stream = TcpStream::connect(&addr).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok()?;

    let len = request_json.len() as u32;
    stream.write_all(&len.to_le_bytes()).ok()?;
    stream.write_all(request_json.as_bytes()).ok()?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).ok()?;
    let response_len = u32::from_le_bytes(len_buf) as usize;

    if response_len > 10 * 1024 * 1024 {
        return None;
    }

    let mut response_buf = vec![0u8; response_len];
    stream.read_exact(&mut response_buf).ok()?;

    serde_json::from_slice(&response_buf).ok()
}

fn daemon_port() -> u16 {
    Config::load().unwrap_or_default().daemon_port
}

#[test]
fn test_daemon_ping() {
    let port = daemon_port();
    let response = send_request(port, IpcCommand::Ping);
    match response {
        Some(r) => assert!(r.is_success()),
        None => {
            panic!("Daemon not running on port {}", port);
        }
    }
}

#[test]
fn test_get_status() {
    let port = daemon_port();
    let response = send_request(port, IpcCommand::GetDaemonStatus);
    match response {
        Some(r) => assert!(r.is_success()),
        None => {
            panic!("Daemon not running on port {}", port);
        }
    }
}

#[test]
fn test_list_devices() {
    let port = daemon_port();
    let response = send_request(port, IpcCommand::ListDevices);
    match response {
        Some(r) => assert!(r.is_success()),
        None => {
            panic!("Daemon not running on port {}", port);
        }
    }
}

#[test]
fn test_get_resources() {
    let port = daemon_port();
    let response = send_request(port, IpcCommand::GetResources);
    match response {
        Some(r) => assert!(r.is_success()),
        None => {
            panic!("Daemon not running on port {}", port);
        }
    }
}

#[test]
fn test_create_memory_pool() {
    let port = daemon_port();
    let response = send_request(port, IpcCommand::CreateMemoryPool {
        name: "test-pool".to_string(),
        size_gb: 8.0,
    });
    match response {
        Some(r) => assert!(r.is_success()),
        None => {
            panic!("Daemon not running on port {}", port);
        }
    }
}

#[test]
fn test_list_memory_pools() {
    let port = daemon_port();
    let response = send_request(port, IpcCommand::ListMemoryPools);
    match response {
        Some(r) => assert!(r.is_success()),
        None => {
            panic!("Daemon not running on port {}", port);
        }
    }
}
