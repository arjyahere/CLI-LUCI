use anyhow::Result;
use luci_common::{IpcCommand, IpcRequest, IpcResponse};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub struct IpcClient {
    port: u16,
}

impl IpcClient {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub fn send(&self, command: IpcCommand) -> Result<IpcResponse> {
        let request = IpcRequest::new(command);
        let request_json = serde_json::to_string(&request)?;

        let addr = format!("127.0.0.1:{}", self.port);
        let mut stream = TcpStream::connect(&addr).map_err(|e| {
            if e.kind() == std::io::ErrorKind::ConnectionRefused {
                luci_common::Error::DaemonNotRunning
            } else {
                luci_common::Error::Network(format!("Failed to connect to daemon: {}", e))
            }
        })?;

        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;

        // Send length-prefixed message
        let len = request_json.len() as u32;
        stream.write_all(&len.to_le_bytes())?;
        stream.write_all(request_json.as_bytes())?;

        // Read length-prefixed response
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf)?;
        let response_len = u32::from_le_bytes(len_buf) as usize;

        if response_len > 10 * 1024 * 1024 {
            return Err(anyhow::anyhow!("Response too large: {} bytes", response_len));
        }

        let mut response_buf = vec![0u8; response_len];
        stream.read_exact(&mut response_buf)?;

        let response: IpcResponse = serde_json::from_slice(&response_buf)?;
        Ok(response)
    }

    pub fn is_daemon_running(&self) -> bool {
        self.send(IpcCommand::Ping).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = IpcClient::new(9527);
        assert_eq!(client.port, 9527);
    }

    #[test]
    fn test_client_creation_custom_port() {
        let client = IpcClient::new(8080);
        assert_eq!(client.port, 8080);
    }

    #[test]
    fn test_send_fails_when_daemon_not_running() {
        let client = IpcClient::new(1);
        let result = client.send(IpcCommand::Ping);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_daemon_running_returns_false_when_not_running() {
        let client = IpcClient::new(1);
        assert!(!client.is_daemon_running());
    }

    #[test]
    fn test_request_serialization() {
        let request = IpcRequest::new(IpcCommand::Ping);
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("Ping"));
    }

    #[test]
    fn test_response_success() {
        let response = IpcResponse::ok();
        assert!(response.is_success());
        assert!(response.payload().is_some());
        assert!(response.error_message().is_none());
    }

    #[test]
    fn test_response_error() {
        let response = IpcResponse::error("test error".to_string());
        assert!(!response.is_success());
        assert!(response.payload().is_none());
        assert_eq!(response.error_message(), Some("test error"));
    }

    #[test]
    fn test_response_with_payload() {
        let response = IpcResponse::success(serde_json::json!({"key": "value"}));
        assert!(response.is_success());
        let payload = response.payload().unwrap();
        assert_eq!(payload["key"], "value");
    }
}
