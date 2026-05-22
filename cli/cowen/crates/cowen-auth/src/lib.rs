use cowen_common::{CowenError, CowenResult};
pub mod client;
pub mod decorator;
pub mod lifecycle;
pub mod models;
pub mod pool;
pub mod provider;

pub use client::AuthClient;
pub use decorator::RequestDecorator;
pub use pool::VaultTokenPool;

use std::sync::Arc;

/// Factory: creates a fully-registered AuthClient.
/// This is the **single registration point** for all AuthProvider implementations.
/// Adding a new AuthMode only requires adding one `.register()` call here.
pub fn create_auth_client(pool: Arc<dyn pool::TokenPool + Send + Sync>) -> AuthClient {
    let builder = AuthClient::builder(pool.clone());
    let http_sender = builder.http_sender.clone();

    builder
        .register(
            models::AuthMode::SelfBuilt,
            Arc::new(provider::self_built::SelfBuiltProvider::new(
                pool.clone(),
                http_sender.clone(),
            )),
        )
        .register(
            models::AuthMode::Oauth2,
            Arc::new(provider::oauth2::OAuth2Provider::new(
                pool.clone(),
                http_sender.clone(),
            )),
        )
        .register(
            models::AuthMode::StoreApp,
            Arc::new(provider::store_app::StoreAppProvider::new(
                pool.clone(),
                http_sender,
            )),
        )
        .build()
}

/// Convenience factory: creates AuthClient from a Vault reference.
pub fn create_auth_client_with_vault(vault: Arc<dyn cowen_common::vault::Vault>) -> AuthClient {
    let pool = Arc::new(pool::VaultTokenPool::new(vault));
    create_auth_client(pool)
}

/// 🚀 Auth-driven Config Validator
/// Implements `ConfigValidator` from `core` to decouple architectural constraints
/// from concrete mode logic.
pub struct AuthProviderValidator;

impl AuthProviderValidator {
    pub fn new(_client: AuthClient) -> Self {
        Self
    }
}

impl cowen_store::ConfigValidator for AuthProviderValidator {
    fn validate_load(
        &self,
        profile: &str,
        config: &cowen_common::config::Config,
        is_distributed: bool,
        exists: bool,
    ) -> CowenResult<()> {
        if config.app_mode == cowen_common::models::AuthMode::SelfBuilt || config.app_mode == cowen_common::models::AuthMode::StoreApp {
            let key_len = if config.encrypt_key.len() == 32 {
                if hex::decode(&config.encrypt_key).is_ok() {
                    16
                } else {
                    32
                }
            } else {
                config.encrypt_key.len()
            };
            if key_len != 16 {
                return Err(CowenError::Config(format!(
                    "encrypt_key is required and must be exactly 16 bytes (or 32-character hex) for SelfBuilt or StoreApp modes, got {} bytes",
                    config.encrypt_key.len()
                )));
            }
        }

        if is_distributed && exists && config.app_mode == cowen_common::models::AuthMode::Oauth2 {
            let msg = format!("⚠️  Skipping profile '{}': Auth mode 'Oauth2' is not allowed in distributed storage scenarios (shared database/redis).", profile);
            eprintln!("{}", msg);
            return Err(CowenError::Internal(format!("SKIPPED: {}", msg)));
        }
        Ok(())
    }

    fn validate_save(
        &self,
        _profile: &str,
        config: &cowen_common::config::Config,
        is_distributed: bool,
    ) -> CowenResult<()> {
        if config.app_mode == cowen_common::models::AuthMode::SelfBuilt || config.app_mode == cowen_common::models::AuthMode::StoreApp {
            let key_len = if config.encrypt_key.len() == 32 {
                if hex::decode(&config.encrypt_key).is_ok() {
                    16
                } else {
                    32
                }
            } else {
                config.encrypt_key.len()
            };
            if key_len != 16 {
                return Err(CowenError::Config(format!(
                    "encrypt_key is required and must be exactly 16 bytes (or 32-character hex) for SelfBuilt or StoreApp modes, got {} bytes",
                    config.encrypt_key.len()
                )));
            }
        }

        if is_distributed && config.app_mode == cowen_common::models::AuthMode::Oauth2 {
            return Err(CowenError::Config("Auth mode 'Oauth2' is not allowed in distributed storage scenarios. Please use Sidecar or SelfBuilt mode for distributed deployments.".to_string()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cowen_common::config::Config;
    use cowen_common::models::AuthMode;
    use cowen_store::ConfigValidator;

    #[test]
    fn test_auth_provider_validator_encrypt_key_validation() {
        let validator = AuthProviderValidator;

        // 1. SelfBuilt with valid 16-byte key
        let mut config = Config {
            app_mode: AuthMode::SelfBuilt,
            encrypt_key: "1234567890123456".to_string(),
            ..Config::default_with_profile("test")
        };
        assert!(validator.validate_save("test", &config, false).is_ok());
        assert!(validator.validate_load("test", &config, false, true).is_ok());

        // 2. SelfBuilt with valid 32-character hex key (should succeed)
        config.encrypt_key = "12345678901234561234567890123456".to_string(); // 32-char hex, valid
        assert!(validator.validate_save("test", &config, false).is_ok());
        assert!(validator.validate_load("test", &config, false, true).is_ok());

        // 3. SelfBuilt with invalid 32-character hex key (should fail)
        config.encrypt_key = "1234567890123456123456789012345g".to_string(); // 'g' is invalid hex
        assert!(validator.validate_save("test", &config, false).is_err());
        assert!(validator.validate_load("test", &config, false, true).is_err());

        // 4. SelfBuilt with empty key (should fail)
        config.encrypt_key = "".to_string();
        let res_save = validator.validate_save("test", &config, false);
        assert!(res_save.is_err());
        assert!(res_save.err().unwrap().to_string().contains("encrypt_key is required and must be exactly 16 bytes"));

        let res_load = validator.validate_load("test", &config, false, true);
        assert!(res_load.is_err());

        // 5. SelfBuilt with invalid length (should fail)
        config.encrypt_key = "too_short".to_string();
        let res_save_short = validator.validate_save("test", &config, false);
        assert!(res_save_short.is_err());
        assert!(res_save_short.err().unwrap().to_string().contains("encrypt_key is required and must be exactly 16 bytes"));

        // 6. StoreApp with valid 16-byte key
        let mut config_store = Config {
            app_mode: AuthMode::StoreApp,
            encrypt_key: "abcdefghijklmnop".to_string(),
            ..Config::default_with_profile("test")
        };
        assert!(validator.validate_save("test", &config_store, false).is_ok());

        // 7. StoreApp with invalid key length
        config_store.encrypt_key = "too_short".to_string();
        assert!(validator.validate_save("test", &config_store, false).is_err());

        // 8. OAuth2 doesn't require encrypt_key validation
        let config_oauth = Config {
            app_mode: AuthMode::Oauth2,
            encrypt_key: "".to_string(),
            ..Config::default_with_profile("test")
        };
        assert!(validator.validate_save("test", &config_oauth, false).is_ok());
    }
}

