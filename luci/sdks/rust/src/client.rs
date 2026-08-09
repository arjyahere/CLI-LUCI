use anyhow::Result;
use luci_common::{IpcCommand, IpcRequest, IpcResponse};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Luci Client - Interface to the Luci daemon.
#[derive(Debug, Clone)]
pub struct LuciClient {
    host: String,
    port: u16,
}

impl LuciClient {
    /// Create a new Luci client.
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
        }
    }

    /// Create a client connected to localhost daemon.
    pub fn local(port: u16) -> Self {
        Self::new("127.0.0.1", port)
    }

    /// Send a command to the daemon.
    pub fn send_command(&self, command: IpcCommand) -> Result<IpcResponse> {
        let request = IpcRequest::new(command);
        let request_json = serde_json::to_string(&request)?;

        let addr = format!("{}:{}", self.host, self.port);
        let mut stream = TcpStream::connect(&addr)?;

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

    /// Check if daemon is running.
    pub fn ping(&self) -> bool {
        self.send_command(IpcCommand::Ping).is_ok()
    }

    /// Get daemon status.
    pub fn status(&self) -> Result<serde_json::Value> {
        let response = self.send_command(IpcCommand::GetDaemonStatus)?;
        match response {
            IpcResponse::Success { payload } => Ok(payload),
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    /// Create a new network.
    pub fn create_network(&self, name: &str) -> Result<serde_json::Value> {
        let response = self.send_command(IpcCommand::CreateNetwork {
            name: name.to_string(),
        })?;
        match response {
            IpcResponse::Success { payload } => Ok(payload),
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    /// Join an existing network.
    pub fn join_network(&self, invite_code: &str) -> Result<()> {
        let response = self.send_command(IpcCommand::JoinNetwork {
            invite_code: invite_code.to_string(),
        })?;
        match response {
            IpcResponse::Success { .. } => Ok(()),
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    /// List all devices.
    pub fn list_devices(&self) -> Result<Vec<serde_json::Value>> {
        let response = self.send_command(IpcCommand::ListDevices)?;
        match response {
            IpcResponse::Success { payload } => {
                let devices = payload["devices"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                Ok(devices)
            }
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    /// Get combined resources.
    pub fn resources(&self) -> Result<serde_json::Value> {
        let response = self.send_command(IpcCommand::GetResources)?;
        match response {
            IpcResponse::Success { payload } => Ok(payload),
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    /// Run a script or command.
    pub fn run(
        &self,
        script: &str,
        args: Vec<String>,
        ram: Option<u64>,
        cpu: Option<usize>,
        node: Option<String>,
    ) -> Result<String> {
        let is_file = script.ends_with(".py")
            || script.ends_with(".js")
            || script.ends_with(".mjs")
            || script.ends_with(".sh")
            || script.ends_with(".bash");

        let (job_type, script_path, script_content) = if is_file {
            let jt = if script.ends_with(".py") {
                "python"
            } else if script.ends_with(".js") || script.ends_with(".mjs") {
                "nodejs"
            } else {
                "shell"
            };
            (jt.to_string(), Some(script.to_string()), None)
        } else {
            ("custom".to_string(), None, Some(script.to_string()))
        };

        let response = self.send_command(IpcCommand::RunJob {
            name: script.to_string(),
            job_type,
            script_path,
            script_content,
            args,
            ram,
            cpu,
            node,
            requires_gpu: false,
            min_vram_bytes: None,
            max_duration_secs: None,
        })?;

        match response {
            IpcResponse::Success { payload } => {
                let job_id = payload["job_id"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                Ok(job_id)
            }
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    /// Get job status.
    pub fn job_status(&self, job_id: &str) -> Result<serde_json::Value> {
        let response = self.send_command(IpcCommand::GetJobStatus {
            job_id: job_id.to_string(),
        })?;
        match response {
            IpcResponse::Success { payload } => Ok(payload),
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    /// List all jobs.
    pub fn list_jobs(&self) -> Result<Vec<serde_json::Value>> {
        let response = self.send_command(IpcCommand::ListJobs)?;
        match response {
            IpcResponse::Success { payload } => {
                let jobs = payload["jobs"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                Ok(jobs)
            }
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    /// Cancel a job.
    pub fn cancel_job(&self, job_id: &str) -> Result<()> {
        let response = self.send_command(IpcCommand::CancelJob {
            job_id: job_id.to_string(),
        })?;
        match response {
            IpcResponse::Success { .. } => Ok(()),
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    /// Create a memory pool.
    pub fn create_memory_pool(
        &self,
        name: &str,
        size_gb: f64,
    ) -> Result<serde_json::Value> {
        let response = self.send_command(IpcCommand::CreateMemoryPool {
            name: name.to_string(),
            size_gb,
        })?;
        match response {
            IpcResponse::Success { payload } => Ok(payload),
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    /// List memory pools.
    pub fn list_memory_pools(&self) -> Result<Vec<serde_json::Value>> {
        let response = self.send_command(IpcCommand::ListMemoryPools)?;
        match response {
            IpcResponse::Success { payload } => {
                let pools = payload["pools"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                Ok(pools)
            }
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    /// Get discovered peers.
    pub fn list_peers(&self) -> Result<Vec<serde_json::Value>> {
        let response = self.send_command(IpcCommand::GetPeers)?;
        match response {
            IpcResponse::Success { payload } => {
                let peers = payload["peers"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                Ok(peers)
            }
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    /// Get health status.
    pub fn health(&self) -> Result<serde_json::Value> {
        let response = self.send_command(IpcCommand::GetHealth)?;
        match response {
            IpcResponse::Success { payload } => Ok(payload),
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    /// Get metrics.
    pub fn metrics(&self) -> Result<serde_json::Value> {
        let response = self.send_command(IpcCommand::GetMetrics)?;
        match response {
            IpcResponse::Success { payload } => Ok(payload),
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    /// Read a remote memory page from another node.
    pub fn read_remote_memory_page(
        &self,
        pool_id: &str,
        page_id: u64,
        target_node: &str,
    ) -> Result<Vec<u8>> {
        let response = self.send_command(IpcCommand::ReadRemoteMemoryPage {
            pool_id: pool_id.to_string(),
            page_id,
            target_node: target_node.to_string(),
        })?;
        match response {
            IpcResponse::Success { payload } => {
                let data_b64 = payload["data"].as_str().unwrap_or("");
                let data = base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    data_b64,
                )?;
                Ok(data)
            }
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }

    /// Write a remote memory page to another node.
    pub fn write_remote_memory_page(
        &self,
        pool_id: &str,
        page_id: u64,
        data: Vec<u8>,
        target_node: &str,
    ) -> Result<()> {
        let data_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &data,
        );
        let response = self.send_command(IpcCommand::WriteRemoteMemoryPage {
            pool_id: pool_id.to_string(),
            page_id,
            data: data_b64,
            target_node: target_node.to_string(),
        })?;
        match response {
            IpcResponse::Success { .. } => Ok(()),
            IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }
}
