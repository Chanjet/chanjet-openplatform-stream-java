use jsonwebtoken::{decode, encode, errors::Error, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IpcRole {
    Admin,
    Plugin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcClaims {
    /// Subject (e.g. "cli", "plugin:search_engine")
    pub sub: String,
    pub role: IpcRole,
    /// Authorized scopes. Admin has ["*"]. Plugins have specific ones like ["config:read", "api:execute"].
    pub scopes: Vec<String>,
    /// Issued at
    pub iat: usize,
    /// Expiration time (Optional, since ephemeral keys invalidate tokens anyway, but good practice)
    pub exp: usize,
}

impl IpcClaims {
    pub fn new(sub: String, role: IpcRole, scopes: Vec<String>, lifetime_seconds: usize) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        Self {
            sub,
            role,
            scopes,
            iat: now,
            exp: now + lifetime_seconds,
        }
    }
}

/// Generates a random 256-bit (32 byte) secret for HMAC-SHA256
pub fn generate_ephemeral_secret() -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let mut secret = vec![0u8; 32];
    rng.fill(secret.as_mut_slice());
    secret
}

static DAEMON_JWT_SECRET: OnceLock<Vec<u8>> = OnceLock::new();

/// Sets the global daemon JWT secret (called once on startup)
pub fn set_global_daemon_secret(secret: Vec<u8>) {
    let _ = DAEMON_JWT_SECRET.set(secret);
}

/// Gets the global daemon JWT secret
pub fn get_global_daemon_secret() -> Option<&'static Vec<u8>> {
    DAEMON_JWT_SECRET.get()
}

/// Sign a JWT with the given secret
pub fn sign_jwt(claims: &IpcClaims, secret: &[u8]) -> Result<String, Error> {
    let header = Header::default(); // HS256 by default
    encode(&header, claims, &EncodingKey::from_secret(secret))
}

/// Verify a JWT with the given secret
pub fn verify_jwt(token: &str, secret: &[u8]) -> Result<IpcClaims, Error> {
    let mut validation = Validation::default();
    validation.leeway = 60; // 60 seconds leeway for clock skew
    let token_data = decode::<IpcClaims>(token, &DecodingKey::from_secret(secret), &validation)?;
    Ok(token_data.claims)
}
