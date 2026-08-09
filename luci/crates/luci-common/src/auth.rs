use anyhow::Result;
use chrono::{Utc, Duration};
use ed25519_dalek::{SigningKey, VerifyingKey};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};

use crate::Config;

/// JWT Claims for node authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (node_id)
    pub sub: String,
    /// Network ID
    pub network_id: String,
    /// Roles
    pub roles: Vec<String>,
    /// Issued at
    pub iat: usize,
    /// Expiration
    pub exp: usize,
}

/// Node authentication manager
pub struct AuthManager {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
    jwt_secret: Vec<u8>,
}

impl AuthManager {
    /// Create a new auth manager from a seed
    pub fn new(seed: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(seed);
        let verifying_key = signing_key.verifying_key();
        
        // Use first 32 bytes of signing key as JWT secret
        let jwt_secret = signing_key.to_bytes()[..32].to_vec();
        
        Self {
            signing_key,
            verifying_key,
            jwt_secret,
        }
    }

    /// Create auth manager from config
    pub fn from_config(config: &Config) -> Result<Self> {
        let signing_key = config.load_signing_key()?;
        Ok(Self::new(&signing_key.to_bytes()))
    }

    /// Get the public key as hex string
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.verifying_key.as_bytes())
    }

    /// Sign data with Ed25519
    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        use ed25519_dalek::Signer;
        self.signing_key.sign(data).to_bytes().to_vec()
    }

    /// Verify Ed25519 signature
    pub fn verify(&self, data: &[u8], signature: &[u8]) -> bool {
        use ed25519_dalek::Verifier;
        let sig_bytes: [u8; 64] = match signature.try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };
        let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);
        self.verifying_key.verify(data, &sig).is_ok()
    }

    /// Issue a JWT token for a node
    pub fn issue_token(
        &self,
        node_id: &str,
        network_id: &str,
        roles: Vec<String>,
        expires_in_hours: u64,
    ) -> Result<String> {
        let now = Utc::now();
        let claims = Claims {
            sub: node_id.to_string(),
            network_id: network_id.to_string(),
            roles,
            iat: now.timestamp() as usize,
            exp: (now + Duration::hours(expires_in_hours as i64)).timestamp() as usize,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(&self.jwt_secret),
        )?;

        Ok(token)
    }

    /// Validate a JWT token
    pub fn validate_token(&self, token: &str) -> Result<Claims> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(&self.jwt_secret),
            &Validation::default(),
        )?;

        Ok(token_data.claims)
    }

    /// Generate a challenge for mutual authentication
    pub fn generate_challenge(&self) -> (String, Vec<u8>) {
        let challenge = uuid::Uuid::new_v4().to_string();
        let signature = self.sign(challenge.as_bytes());
        (challenge, signature)
    }

    /// Verify a challenge response
    pub fn verify_challenge(
        &self,
        challenge: &str,
        signature: &[u8],
        public_key_hex: &str,
    ) -> bool {
        let public_key_bytes = match hex::decode(public_key_hex) {
            Ok(b) => b,
            Err(_) => return false,
        };
        
        let public_key_array: [u8; 32] = match public_key_bytes.try_into() {
            Ok(a) => a,
            Err(_) => return false,
        };
        
        let verifying_key = match VerifyingKey::from_bytes(&public_key_array) {
            Ok(k) => k,
            Err(_) => return false,
        };

        use ed25519_dalek::Verifier;
        let sig_bytes: [u8; 64] = match signature.try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };
        let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);

        verifying_key.verify(challenge.as_bytes(), &sig).is_ok()
    }
}

/// Peer authentication state
#[derive(Debug, Clone)]
pub struct PeerAuth {
    pub node_id: String,
    pub public_key: String,
    pub token: Option<String>,
    pub authenticated: bool,
    pub last_verified: chrono::DateTime<chrono::Utc>,
}

impl PeerAuth {
    pub fn new(node_id: String, public_key: String) -> Self {
        Self {
            node_id,
            public_key,
            token: None,
            authenticated: false,
            last_verified: Utc::now(),
        }
    }
}
