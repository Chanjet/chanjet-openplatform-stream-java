use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Utc};
use cowen_common::vault::Vault;
use cowen_common::{CowenError, CowenResult};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

const JWKS_KEY: &str = "cowen:system:jwks";
const ROTATION_DAYS: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyStatus {
    ACTIVE,
    ROTATED,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Jwk {
    pub kid: String,
    pub kty: String, // "oct"
    pub alg: String, // "A256GCM"
    pub k: String,   // base64 url safe without padding
    pub status: KeyStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Jwks {
    pub keys: Vec<Jwk>,
}

#[async_trait::async_trait]
pub trait KeyProvider: Send + Sync {
    async fn get_active_key(&self) -> CowenResult<(String, Vec<u8>)>;
    async fn get_key_by_kid(&self, kid: &str) -> CowenResult<Vec<u8>>;
}

#[derive(Clone)]
pub struct JwksManager {
    vault: Arc<dyn Vault>,
    profile: String,
    jwks: Arc<RwLock<Jwks>>,
}

impl JwksManager {
    pub async fn new(vault: Arc<dyn Vault>, profile: &str) -> CowenResult<Self> {
        let manager = Self {
            vault,
            profile: profile.to_string(),
            jwks: Arc::new(RwLock::new(Jwks { keys: vec![] })),
        };
        manager.sync_and_rotate().await?;
        Ok(manager)
    }

    async fn sync_and_rotate(&self) -> CowenResult<()> {
        let mut keys = match self.vault.get_secret(&self.profile, JWKS_KEY).await {
            Ok(json_str) => {
                serde_json::from_str::<Jwks>(&json_str).unwrap_or_else(|_| Jwks { keys: vec![] })
            }
            Err(_) => Jwks { keys: vec![] }, // If not found, start empty
        };

        let now = Utc::now();
        let mut needs_new_key = true;

        if let Some(active) = keys
            .keys
            .iter()
            .find(|k| matches!(k.status, KeyStatus::ACTIVE))
        {
            if now.signed_duration_since(active.created_at).num_days() < ROTATION_DAYS {
                needs_new_key = false;
            }
        }

        if needs_new_key {
            // Rotate all to ROTATED
            for key in &mut keys.keys {
                key.status = KeyStatus::ROTATED;
            }

            // Generate new 256-bit key
            let rng = SystemRandom::new();
            let mut raw_key = [0u8; 32];
            rng.fill(&mut raw_key)
                .map_err(|_| CowenError::Internal("Failed to generate key".to_string()))?;

            let new_jwk = Jwk {
                kid: Uuid::new_v4().to_string(),
                kty: "oct".to_string(),
                alg: "A256GCM".to_string(),
                k: URL_SAFE_NO_PAD.encode(raw_key),
                status: KeyStatus::ACTIVE,
                created_at: now,
            };

            keys.keys.push(new_jwk);

            // Save to store
            if let Ok(json_str) = serde_json::to_string(&keys) {
                let _ = self
                    .vault
                    .set_secret(&self.profile, JWKS_KEY, &json_str)
                    .await;
            }
        }

        *self.jwks.write().await = keys;
        Ok(())
    }
}

#[async_trait::async_trait]
impl KeyProvider for JwksManager {
    async fn get_active_key(&self) -> CowenResult<(String, Vec<u8>)> {
        let jwks = self.jwks.read().await;
        let active = jwks
            .keys
            .iter()
            .find(|k| matches!(k.status, KeyStatus::ACTIVE))
            .ok_or_else(|| CowenError::Internal("No active key found".to_string()))?;

        let key_bytes = URL_SAFE_NO_PAD
            .decode(&active.k)
            .map_err(|_| CowenError::Internal("Invalid key encoding".to_string()))?;
        Ok((active.kid.clone(), key_bytes))
    }

    async fn get_key_by_kid(&self, kid: &str) -> CowenResult<Vec<u8>> {
        let jwks = self.jwks.read().await;
        let jwk = jwks
            .keys
            .iter()
            .find(|k| k.kid == kid)
            .ok_or_else(|| CowenError::api(format!("Key ID {} not found", kid)))?;

        let key_bytes = URL_SAFE_NO_PAD
            .decode(&jwk.k)
            .map_err(|_| CowenError::Internal("Invalid key encoding".to_string()))?;
        Ok(key_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cowen_common::config::{AppConfig, StorageConfig};
    use cowen_store::create_vault;
    use tempfile::tempdir;

    async fn setup_vault() -> Arc<dyn Vault> {
        let dir = tempdir().unwrap();
        let cfg = AppConfig {
            storage: StorageConfig {
                store: "local".to_string(),
                db_url: None,
                cache: "memory".to_string(),
                cache_url: None,
            },
            ..AppConfig::default()
        };
        create_vault(&cfg, dir.path(), "test-fp").await.unwrap()
    }

    #[tokio::test]
    async fn test_sync_and_rotate_empty_vault() {
        let vault = setup_vault().await;
        let manager = JwksManager::new(vault, "default").await.unwrap();

        let (kid, key) = manager.get_active_key().await.unwrap();
        assert!(!kid.is_empty());
        assert_eq!(key.len(), 32);

        let retrieved_key = manager.get_key_by_kid(&kid).await.unwrap();
        assert_eq!(key, retrieved_key);
    }

    #[tokio::test]
    async fn test_sync_and_rotate_recent_key() {
        let vault = setup_vault().await;
        let manager1 = JwksManager::new(vault.clone(), "default").await.unwrap();
        let (kid1, key1) = manager1.get_active_key().await.unwrap();

        // Re-initialize manager, it should not rotate the key
        let manager2 = JwksManager::new(vault.clone(), "default").await.unwrap();
        let (kid2, key2) = manager2.get_active_key().await.unwrap();

        assert_eq!(kid1, kid2);
        assert_eq!(key1, key2);
    }

    #[tokio::test]
    async fn test_sync_and_rotate_old_key() {
        let vault = setup_vault().await;
        let manager1 = JwksManager::new(vault.clone(), "default").await.unwrap();
        let (kid1, key1) = manager1.get_active_key().await.unwrap();

        // Manually manipulate the vault to make the key old
        let mut jwks: Jwks =
            serde_json::from_str(&vault.get_secret("default", JWKS_KEY).await.unwrap()).unwrap();
        jwks.keys[0].created_at = Utc::now() - chrono::Duration::days(ROTATION_DAYS + 1);
        vault
            .set_secret("default", JWKS_KEY, &serde_json::to_string(&jwks).unwrap())
            .await
            .unwrap();

        // Re-initialize, it should rotate
        let manager2 = JwksManager::new(vault.clone(), "default").await.unwrap();
        let (kid2, key2) = manager2.get_active_key().await.unwrap();

        assert_ne!(kid1, kid2);
        assert_ne!(key1, key2);

        // Old key should still be retrievable
        let old_key = manager2.get_key_by_kid(&kid1).await.unwrap();
        assert_eq!(old_key, key1);
    }
}
