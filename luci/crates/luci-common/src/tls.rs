use anyhow::{Context, Result};
use rcgen::{CertificateParams, KeyPair};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::path::{Path, PathBuf};

/// TLS configuration for a Luci node
#[derive(Clone)]
pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub ca_path: Option<PathBuf>,
}

impl TlsConfig {
    /// Create TLS config with default paths
    pub fn new(data_dir: &Path) -> Self {
        let tls_dir = data_dir.join("tls");
        Self {
            cert_path: tls_dir.join("node.crt"),
            key_path: tls_dir.join("node.key"),
            ca_path: None,
        }
    }

    /// Ensure TLS directory and certificates exist, generating if needed
    pub fn ensure_certs(&self) -> Result<()> {
        let tls_dir = self.cert_path.parent().context("Invalid cert path")?;
        std::fs::create_dir_all(tls_dir)
            .context("Failed to create TLS directory")?;

        if !self.cert_path.exists() || !self.key_path.exists() {
            self.generate_self_signed()?;
        }

        Ok(())
    }

    /// Generate a self-signed certificate for this node
    fn generate_self_signed(&self) -> Result<()> {
        let key_pair = KeyPair::generate().context("Failed to generate key pair")?;

        let mut params = CertificateParams::new(vec!["luci-node.local".to_string()])
            .context("Failed to create certificate params")?;

        // Add SANs for common addresses
        params
            .subject_alt_names
            .push(rcgen::SanType::IpAddress(std::net::IpAddr::V4(
                std::net::Ipv4Addr::new(127, 0, 0, 1),
            )));
        params
            .subject_alt_names
            .push(rcgen::SanType::IpAddress(std::net::IpAddr::V4(
                std::net::Ipv4Addr::new(0, 0, 0, 0),
            )));

        let cert = params
            .self_signed(&key_pair)
            .context("Failed to generate certificate")?;

        // Write certificate
        std::fs::write(&self.cert_path, cert.pem())
            .context("Failed to write certificate")?;

        // Write private key
        let key_der = key_pair.serialize_der();
        let pkcs8 = PrivatePkcs8KeyDer::from(key_der);
        let key_pem = format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----",
            base64_encode(pkcs8.secret_pkcs8_der())
        );
        std::fs::write(&self.key_path, key_pem)
            .context("Failed to write private key")?;

        // Set restrictive permissions on key (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&self.key_path, perms)?;
        }

        tracing::info!("Generated self-signed TLS certificate at {:?}", self.cert_path);
        Ok(())
    }

    /// Load the TLS server config
    pub fn load_server_config(&self) -> Result<rustls::ServerConfig> {
        let certs = self.load_certs()?;
        let key = self.load_private_key()?;

        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .context("Failed to create TLS server config")?;

        Ok(config)
    }

    /// Load the TLS client config
    pub fn load_client_config(&self) -> Result<rustls::ClientConfig> {
        let mut root_store = rustls::RootCertStore::empty();

        // Load our CA or self-signed cert as trusted
        let certs = self.load_certs()?;
        for cert in &certs {
            root_store
                .add(cert.clone())
                .context("Failed to add cert to root store")?;
        }

        let config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        Ok(config)
    }

    /// Load certificates from disk
    fn load_certs(&self) -> Result<Vec<CertificateDer<'static>>> {
        let cert_file = std::fs::File::open(&self.cert_path)
            .context("Failed to open certificate file")?;
        let mut reader = std::io::BufReader::new(cert_file);

        let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut reader)
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to parse certificates")?;

        if certs.is_empty() {
            anyhow::bail!("No certificates found in {:?}", self.cert_path);
        }

        Ok(certs)
    }

    /// Load private key from disk
    fn load_private_key(&self) -> Result<PrivateKeyDer<'static>> {
        let key_file = std::fs::File::open(&self.key_path)
            .context("Failed to open private key file")?;
        let mut reader = std::io::BufReader::new(key_file);

        let key = rustls_pemfile::private_key(&mut reader)
            .context("Failed to parse private key")?
            .context("No private key found")?;

        Ok(key)
    }

    /// Get the certificate fingerprint (SHA-256) for peer verification
    pub fn cert_fingerprint(&self) -> Result<String> {
        let certs = self.load_certs()?;
        let cert = &certs[0];

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(cert.as_ref());
        let hash = hasher.finalize();

        Ok(hex::encode(hash))
    }
}

/// Simple base64 encoder
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
