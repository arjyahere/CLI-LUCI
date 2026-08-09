use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::net::TcpStream;

const RELAY_REGISTRY_URL: &str = "https://registry.luci.dev/peers";
const HEARTBEAT_INTERVAL_SECS: u64 = 30;
const PEER_TIMEOUT_SECS: i64 = 90;

/// Peer information for cross-network discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub node_id: String,
    pub node_name: String,
    pub public_key: Option<String>,
    pub relay_addresses: Vec<String>,
    pub direct_addresses: Vec<String>,
    pub network_id: Option<String>,
    pub last_seen: DateTime<Utc>,
    pub capabilities: PeerCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PeerCapabilities {
    pub can_execute_jobs: bool,
    pub can_provide_memory: bool,
    pub can_sync_files: bool,
    pub max_concurrent_jobs: usize,
    pub available_memory_gb: f64,
    pub os: String,
    pub arch: String,
}

/// Cross-network discovery service using relay registry
pub struct CrossNetworkDiscovery {
    node_id: String,
    node_name: String,
    network_id: Option<String>,
    relay_addresses: Vec<String>,
    direct_addresses: Vec<String>,
    public_key: Option<String>,
    known_peers: Arc<RwLock<HashMap<String, PeerInfo>>>,
    relay_client: reqwest::Client,
}

impl CrossNetworkDiscovery {
    pub fn new(
        node_id: String,
        node_name: String,
        network_id: Option<String>,
        relay_addresses: Vec<String>,
        direct_addresses: Vec<String>,
        public_key: Option<String>,
    ) -> Self {
        Self {
            node_id,
            node_name,
            network_id,
            relay_addresses,
            direct_addresses,
            public_key,
            known_peers: Arc::new(RwLock::new(HashMap::new())),
            relay_client: reqwest::Client::new(),
        }
    }

    /// Register this node with the relay registry
    pub async fn register_with_relay(&self) -> Result<()> {
        let registration = PeerRegistration {
            node_id: self.node_id.clone(),
            node_name: self.node_name.clone(),
            network_id: self.network_id.clone(),
            relay_addresses: self.relay_addresses.clone(),
            direct_addresses: self.direct_addresses.clone(),
            public_key: self.public_key.clone(),
            capabilities: PeerCapabilities {
                can_execute_jobs: true,
                can_provide_memory: true,
                can_sync_files: true,
                max_concurrent_jobs: 4,
                available_memory_gb: 8.0,
                os: std::env::consts::OS.to_string(),
                arch: std::env::consts::ARCH.to_string(),
            },
        };

        let response = self.relay_client
            .post(format!("{}/register", RELAY_REGISTRY_URL))
            .json(&registration)
            .send()
            .await
            .context("Failed to connect to relay registry")?;

        if response.status().is_success() {
            tracing::info!("Successfully registered with relay registry");
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Relay registration failed: {} - {}", status, body)
        }
    }

    /// Fetch peers from the relay registry
    pub async fn fetch_peers_from_relay(&self) -> Result<Vec<PeerInfo>> {
        let mut url = format!("{}/peers", RELAY_REGISTRY_URL);
        if let Some(ref network_id) = self.network_id {
            url = format!("{}/peers?network_id={}", RELAY_REGISTRY_URL, network_id);
        }

        let response = self.relay_client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch peers from relay")?;

        if response.status().is_success() {
            let peers: Vec<PeerInfo> = response.json().await
                .context("Failed to parse relay response")?;
            
            tracing::info!("Fetched {} peers from relay registry", peers.len());
            Ok(peers)
        } else {
            tracing::warn!("Failed to fetch peers from relay: {}", response.status());
            Ok(Vec::new())
        }
    }

    /// Update local peer cache with relay data
    pub async fn sync_peers(&self) -> Result<()> {
        let relay_peers = self.fetch_peers_from_relay().await?;
        
        let mut known = self.known_peers.write().await;
        let mut new_count = 0;
        let mut updated_count = 0;

        for peer in relay_peers {
            if peer.node_id == self.node_id {
                continue; // Skip ourselves
            }

            if let Some(existing) = known.get_mut(&peer.node_id) {
                existing.last_seen = Utc::now();
                existing.relay_addresses = peer.relay_addresses;
                existing.direct_addresses = peer.direct_addresses;
                existing.capabilities = peer.capabilities;
                updated_count += 1;
            } else {
                known.insert(peer.node_id.clone(), peer);
                new_count += 1;
            }
        }

        // Remove stale peers
        let cutoff = Utc::now() - chrono::Duration::seconds(PEER_TIMEOUT_SECS);
        known.retain(|_, peer| peer.last_seen > cutoff);

        tracing::info!("Peer sync: {} new, {} updated, {} total", 
            new_count, updated_count, known.len());
        
        Ok(())
    }

    /// Connect to a peer via relay (for NAT traversal)
    pub async fn connect_via_relay(
        &self,
        peer_id: &str,
        relay_address: &str,
    ) -> Result<TcpStream> {
        tracing::info!("Connecting to peer {} via relay {}", peer_id, relay_address);
        
        let stream = TcpStream::connect(relay_address).await
            .context(format!("Failed to connect to relay at {}", relay_address))?;
        
        // Send peer identification
        let _identification = RelayConnectRequest {
            from_node: self.node_id.clone(),
            to_node: peer_id.to_string(),
            auth_token: String::new(), // Will be filled by auth layer
        };
        
        // For now, just return the stream
        // In production, we'd send the identification and wait for relay handshake
        Ok(stream)
    }

    /// Get best address to connect to a peer (direct if possible, relay otherwise)
    pub async fn get_peer_address(&self, peer_id: &str) -> Option<String> {
        let known = self.known_peers.read().await;
        if let Some(peer) = known.get(peer_id) {
            // Try direct address first
            if let Some(addr) = peer.direct_addresses.first() {
                return Some(addr.clone());
            }
            // Fall back to relay
            if let Some(addr) = peer.relay_addresses.first() {
                return Some(format!("relay:{}", addr));
            }
        }
        None
    }

    /// List all known peers
    pub async fn list_peers(&self) -> Vec<PeerInfo> {
        self.known_peers.read().await.values().cloned().collect()
    }

    /// Get peer count by status
    pub async fn get_peer_stats(&self) -> PeerStats {
        let known = self.known_peers.read().await;
        let mut stats = PeerStats {
            total_peers: known.len(),
            ..Default::default()
        };

        for peer in known.values() {
            if peer.capabilities.can_execute_jobs {
                stats.execute_capable += 1;
            }
            if peer.capabilities.can_provide_memory {
                stats.memory_capable += 1;
            }
            if peer.capabilities.can_sync_files {
                stats.file_sync_capable += 1;
            }
        }

        stats
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerRegistration {
    pub node_id: String,
    pub node_name: String,
    pub network_id: Option<String>,
    pub relay_addresses: Vec<String>,
    pub direct_addresses: Vec<String>,
    pub public_key: Option<String>,
    pub capabilities: PeerCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConnectRequest {
    pub from_node: String,
    pub to_node: String,
    pub auth_token: String,
}

#[derive(Debug, Default)]
pub struct PeerStats {
    pub total_peers: usize,
    pub execute_capable: usize,
    pub memory_capable: usize,
    pub file_sync_capable: usize,
}

/// Discovery manager that combines LAN and cross-network discovery
pub struct DiscoveryManager {
    lan_discovery: Option<Arc<crate::daemon::discovery::DiscoveryService>>,
    cross_network: Option<Arc<CrossNetworkDiscovery>>,
}

impl Default for DiscoveryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscoveryManager {
    pub fn new() -> Self {
        Self {
            lan_discovery: None,
            cross_network: None,
        }
    }

    pub fn with_lan(mut self, discovery: crate::daemon::discovery::DiscoveryService) -> Self {
        self.lan_discovery = Some(Arc::new(discovery));
        self
    }

    pub fn with_cross_network(mut self, discovery: CrossNetworkDiscovery) -> Self {
        self.cross_network = Some(Arc::new(discovery));
        self
    }

    pub async fn start(&self) -> Result<()> {
        // Start LAN discovery
        if let Some(ref lan) = self.lan_discovery {
            let lan = lan.clone();
            tokio::spawn(async move {
                let _ = lan.listen_for_peers().await;
            });
        }

        // Start cross-network discovery
        if let Some(ref cross) = self.cross_network {
            let cross = cross.clone();
            tokio::spawn(async move {
                loop {
                    if let Err(e) = cross.sync_peers().await {
                        tracing::error!("Cross-network sync failed: {}", e);
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(HEARTBEAT_INTERVAL_SECS)).await;
                }
            });
        }

        Ok(())
    }

    pub async fn get_all_peers(&self) -> Vec<PeerInfo> {
        let mut peers = Vec::new();

        if let Some(ref cross) = self.cross_network {
            peers.extend(cross.list_peers().await);
        }

        peers
    }
}