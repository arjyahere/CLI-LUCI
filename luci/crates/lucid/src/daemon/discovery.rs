use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use std::sync::Arc;
use tokio::sync::RwLock;

const MAX_MESSAGE_SIZE: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryMessage {
    pub node_id: String,
    pub node_name: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub tcp_port: u16,
    pub ip_addresses: Vec<String>,
    #[serde(default = "default_timestamp")]
    pub last_seen: DateTime<Utc>,
}

fn default_timestamp() -> DateTime<Utc> {
    Utc::now()
}

pub struct DiscoveryService {
    socket: Arc<UdpSocket>,
    node_id: String,
    node_name: String,
    tcp_port: u16,
    broadcast_addr: String,
    known_nodes: Arc<RwLock<Vec<DiscoveryMessage>>>,
}

impl DiscoveryService {
    pub async fn new(
        node_id: String,
        node_name: String,
        tcp_port: u16,
        discovery_port: u16,
    ) -> Result<Self> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", discovery_port)).await?;
        socket.set_broadcast(true)?;
        let broadcast_addr = format!("255.255.255.255:{}", discovery_port);

        tracing::info!("Discovery service started on UDP:{}", discovery_port);

        Ok(Self {
            socket: Arc::new(socket),
            node_id,
            node_name,
            tcp_port,
            broadcast_addr,
            known_nodes: Arc::new(RwLock::new(Vec::new())),
        })
    }

    pub fn build_message(&self) -> DiscoveryMessage {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let ip_addresses = get_local_ip_addresses();

        DiscoveryMessage {
            node_id: self.node_id.clone(),
            node_name: self.node_name.clone(),
            hostname,
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            tcp_port: self.tcp_port,
            ip_addresses,
            last_seen: Utc::now(),
        }
    }

    pub async fn broadcast_presence(&self) -> Result<()> {
        let msg = self.build_message();
        let json = serde_json::to_vec(&msg)?;

        self.socket.send_to(&json, &self.broadcast_addr).await?;
        tracing::debug!("Broadcast presence: {}", self.node_id);
        Ok(())
    }

    pub async fn listen_for_peers(self: &Arc<Self>) -> Result<()> {
        let mut buf = vec![0u8; MAX_MESSAGE_SIZE];
        loop {
            let (len, addr) = self.socket.recv_from(&mut buf).await?;
            if len == 0 {
                continue;
            }

            if let Ok(msg) = serde_json::from_slice::<DiscoveryMessage>(&buf[..len]) {
                // Ignore our own broadcasts
                if msg.node_id == self.node_id {
                    continue;
                }

                tracing::info!(
                    "Discovered node: {} ({}) at {}:{}",
                    msg.node_name,
                    msg.node_id,
                    addr.ip(),
                    msg.tcp_port
                );

                // Check if already known
                let mut known = self.known_nodes.write().await;
                if !known.iter().any(|n| n.node_id == msg.node_id) {
                    known.push(msg.clone());
                    tracing::info!("New peer added: {}", msg.node_id);
                } else {
                    // Update existing entry with fresh timestamp
                    if let Some(existing) = known.iter_mut().find(|n| n.node_id == msg.node_id) {
                        *existing = msg.clone();
                        existing.last_seen = Utc::now();
                    }
                }
            }
        }
    }

    pub async fn get_known_peers(&self) -> Vec<DiscoveryMessage> {
        self.known_nodes.read().await.clone()
    }

    pub async fn remove_stale_peers(&self, timeout_secs: u64) {
        let mut known = self.known_nodes.write().await;
        let cutoff = Utc::now() - chrono::Duration::seconds(timeout_secs as i64);
        let before_count = known.len();
        known.retain(|node| node.last_seen > cutoff);
        let removed = before_count - known.len();
        if removed > 0 {
            tracing::info!("Removed {} stale peers (timeout: {}s)", removed, timeout_secs);
        }
    }
}

fn get_local_ip_addresses() -> Vec<String> {
    let mut ips = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("ipconfig")
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let line = line.trim();
                if line.contains("IPv4 Address") || line.contains("IP Address") {
                    if let Some(ip) = line.split(':').next_back() {
                        let ip = ip.trim().to_string();
                        if !ip.is_empty() && ip != "127.0.0.1" {
                            ips.push(ip);
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(addrs) = local_ip_address::list_afinet_netifas() {
            for (_name, ip) in addrs {
                if !ip.is_loopback() {
                    ips.push(ip.to_string());
                }
            }
        }
    }

    ips
}
