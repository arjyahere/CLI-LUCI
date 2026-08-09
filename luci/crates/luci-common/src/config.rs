use crate::types::NodeId;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const APP_NAME: &str = "luci";

/// Configuration for a remote peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub public_key: Option<String>,
    pub cert_fingerprint: Option<String>,
    #[serde(default)]
    pub auto_connect: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub node_id: NodeId,
    pub node_name: String,
    pub network_id: Option<String>,
    pub network_name: Option<String>,
    pub daemon_port: u16,
    pub node_port: u16,
    pub discovery_port: u16,
    pub log_level: String,
    pub data_dir: PathBuf,
    pub contribute_ram_percent: u8,
    pub contribute_gpu: bool,
    pub max_jobs: usize,
    #[serde(default = "default_auth_token")]
    pub auth_token: String,
    #[serde(default)]
    pub tls_enabled: bool,
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
    #[serde(default)]
    pub peers: Vec<PeerConfig>,
    #[serde(default)]
    pub node_public_key: Option<String>,
}

fn default_auth_token() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn default_metrics_port() -> u16 {
    9530
}

impl Default for Config {
    fn default() -> Self {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        Self {
            node_id: NodeId::new(),
            node_name: hostname.clone(),
            network_id: None,
            network_name: None,
            daemon_port: 9527,
            node_port: 9528,
            discovery_port: 9529,
            log_level: "info".to_string(),
            data_dir: default_data_dir(),
            contribute_ram_percent: 60,
            contribute_gpu: true,
            max_jobs: 4,
            auth_token: default_auth_token(),
            tls_enabled: true,
            metrics_port: default_metrics_port(),
            peers: Vec::new(),
            node_public_key: None,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config at {:?}", path))?;
        let config: Self = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse config at {:?}", path))?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory at {:?}", parent))?;
        }
        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize config")?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write config to {:?}", path))?;
        Ok(())
    }

    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    pub fn jobs_dir(&self) -> PathBuf {
        self.data_dir.join("jobs")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    pub fn tls_dir(&self) -> PathBuf {
        self.data_dir.join("tls")
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.data_dir)?;
        fs::create_dir_all(self.jobs_dir())?;
        fs::create_dir_all(self.logs_dir())?;
        fs::create_dir_all(self.tls_dir())?;
        Ok(())
    }

    /// Get TLS configuration
    pub fn tls_config(&self) -> crate::tls::TlsConfig {
        crate::tls::TlsConfig::new(&self.data_dir)
    }

    /// Add a peer to the configuration
    pub fn add_peer(&mut self, peer: PeerConfig) -> Result<()> {
        // Remove existing peer with same id
        self.peers.retain(|p| p.id != peer.id);
        self.peers.push(peer);
        self.save()
    }

    /// Remove a peer by id
    pub fn remove_peer(&mut self, peer_id: &str) -> Result<()> {
        self.peers.retain(|p| p.id != peer_id);
        self.save()
    }

    /// Get a peer by id
    pub fn get_peer(&self, peer_id: &str) -> Option<&PeerConfig> {
        self.peers.iter().find(|p| p.id == peer_id)
    }

    /// Get all peers
    pub fn list_peers(&self) -> &[PeerConfig] {
        &self.peers
    }

    /// Generate Ed25519 keypair for node identity
    pub fn ensure_node_keys(&mut self) -> Result<()> {
        if self.node_public_key.is_some() {
            return Ok(());
        }

        use ed25519_dalek::SigningKey;
        use rand::RngCore;

        let mut rng = rand::thread_rng();
        let mut seed = [0u8; 32];
        rng.fill_bytes(&mut seed);

        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();

        self.node_public_key = Some(hex::encode(verifying_key.as_bytes()));
        self.save()?;

        // Save private key to file
        let key_path = self.data_dir.join("keys").join("node.key");
        if let Some(parent) = key_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&key_path, hex::encode(signing_key.to_bytes()))?;

        tracing::info!("Generated Ed25519 keypair for node");
        Ok(())
    }

    /// Load the signing key from disk
    pub fn load_signing_key(&self) -> Result<ed25519_dalek::SigningKey> {
        let key_path = self.data_dir.join("keys").join("node.key");
        let key_hex = fs::read_to_string(&key_path)
            .context("Failed to read node private key")?;
        let key_bytes = hex::decode(key_hex.trim())
            .context("Failed to decode node private key")?;
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&key_bytes);
        Ok(ed25519_dalek::SigningKey::from_bytes(&seed))
    }
}

fn config_path() -> PathBuf {
    default_config_dir().join("config.json")
}

fn default_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_NAME)
}

fn default_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_NAME)
}
