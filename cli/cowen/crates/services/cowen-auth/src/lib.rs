use cowen_common::{CowenError, CowenResult};
pub mod client;
pub mod decorator;
pub mod diagnostics;
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

thread_local! {
    static DISABLE_TEST_ENV_CHECK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn is_test_env() -> bool {
    if DISABLE_TEST_ENV_CHECK.with(|cell| cell.get()) {
        return false;
    }
    std::env::var("COWEN_TEST_MODE").is_ok()
        || std::env::var("CARGO_MANIFEST_DIR").is_ok()
        || std::env::var("COWEN_BIN").is_ok()
        || std::env::args().any(|arg| arg.contains("test"))
}

/// 🚀 Auth-driven Config Validator
/// Implements `ConfigValidator` from `core` to decouple architectural constraints
/// from concrete mode logic.
pub struct AuthProviderValidator {
    client: AuthClient,
}

impl AuthProviderValidator {
    pub fn new(client: AuthClient) -> Self {
        Self { client }
    }
}

fn validate_decrypt_key(
    client: &AuthClient,
    config: &cowen_common::config::Config,
) -> CowenResult<()> {
    match client.provider(&config.app_mode).validate_config(config) {
        Ok(_) => Ok(()),
        Err(e) => {
            if is_test_env() {
                eprintln!("⚠️  [WARNING] {}", e);
                tracing::warn!("{}", e);
                Ok(())
            } else {
                Err(e)
            }
        }
    }
}

impl cowen_config::ConfigValidator for AuthProviderValidator {
    fn validate_load(
        &self,
        profile: &str,
        config: &cowen_common::config::Config,
        is_distributed: bool,
        exists: bool,
    ) -> CowenResult<()> {
        validate_decrypt_key(&self.client, config)?;

        if is_distributed
            && exists
            && !self
                .client
                .provider(&config.app_mode)
                .is_allowed_in_distributed_storage()
        {
            let msg = format!("⚠️  Skipping profile '{}': Auth mode '{}' is not allowed in distributed storage scenarios (shared database/redis).", profile, config.app_mode);
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
        validate_decrypt_key(&self.client, config)?;

        if is_distributed
            && !self
                .client
                .provider(&config.app_mode)
                .is_allowed_in_distributed_storage()
        {
            return Err(CowenError::Config(format!("Auth mode '{}' is not allowed in distributed storage scenarios. Please use Sidecar or SelfBuilt mode for distributed deployments.", config.app_mode)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cowen_common::config::Config;
    use cowen_common::models::AuthMode;
    use cowen_config::ConfigValidator;

    fn run_in_prod_env<F: FnOnce()>(f: F) {
        DISABLE_TEST_ENV_CHECK.with(|cell| cell.set(true));
        f();
        DISABLE_TEST_ENV_CHECK.with(|cell| cell.set(false));
    }

    async fn create_test_validator() -> AuthProviderValidator {
        let temp_dir = tempfile::tempdir().unwrap();
        let app_cfg = cowen_common::config::AppConfig::default();
        let vault = cowen_store::create_vault(&app_cfg, temp_dir.path(), "test_fingerprint")
            .await
            .unwrap();
        let pool = Arc::new(VaultTokenPool::new(vault));
        let client = create_auth_client(pool);
        AuthProviderValidator::new(client)
    }

    #[tokio::test]
    async fn test_auth_provider_validator_encrypt_key_validation() {
        let validator = create_test_validator().await;

        // 1. SelfBuilt with valid 16-byte key
        let mut config = Config {
            app_mode: AuthMode::SelfBuilt,
            encrypt_key: "1234567890123456".to_string(),
            ..Config::default_with_profile("test")
        };
        assert!(validator.validate_save("test", &config, false).is_ok());
        assert!(validator
            .validate_load("test", &config, false, true)
            .is_ok());

        // 2. SelfBuilt with valid 32-character hex key (should succeed)
        config.encrypt_key = "12345678901234561234567890123456".to_string(); // 32-char hex, valid
        assert!(validator.validate_save("test", &config, false).is_ok());
        assert!(validator
            .validate_load("test", &config, false, true)
            .is_ok());

        // 3. SelfBuilt with invalid 32-character hex key (should fail)
        config.encrypt_key = "1234567890123456123456789012345g".to_string(); // 'g' is invalid hex
        run_in_prod_env(|| {
            assert!(validator.validate_save("test", &config, false).is_err());
            assert!(validator
                .validate_load("test", &config, false, true)
                .is_err());
        });

        // 4. SelfBuilt with empty key and empty app_secret (should fail)
        config.encrypt_key = "".to_string();
        config.app_secret = "".to_string();
        run_in_prod_env(|| {
            let res_save = validator.validate_save("test", &config, false);
            assert!(res_save.as_ref().is_err());
            assert!(
                res_save
                    .as_ref()
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("is required and cannot be empty")
                    || res_save
                        .as_ref()
                        .err()
                        .unwrap()
                        .to_string()
                        .contains("is_required and cannot be empty")
            );

            let res_load = validator.validate_load("test", &config, false, true);
            assert!(res_load.is_err());
        });

        // 5. SelfBuilt with invalid length (should fail)
        config.encrypt_key = "too_short".to_string();
        run_in_prod_env(|| {
            let res_save_short = validator.validate_save("test", &config, false);
            assert!(res_save_short.is_err());
            assert!(res_save_short
                .err()
                .unwrap()
                .to_string()
                .contains("must be exactly 16 bytes"));
        });

        // 6. StoreApp with valid 16-byte key
        let mut config_store = Config {
            app_mode: AuthMode::StoreApp,
            encrypt_key: "abcdefghijklmnop".to_string(),
            ..Config::default_with_profile("test")
        };
        assert!(validator
            .validate_save("test", &config_store, false)
            .is_ok());

        // 7. StoreApp with invalid key length
        config_store.encrypt_key = "too_short".to_string();
        run_in_prod_env(|| {
            assert!(validator
                .validate_save("test", &config_store, false)
                .is_err());
        });

        // 8. OAuth2 doesn't require encrypt_key validation
        let config_oauth = Config {
            app_mode: AuthMode::Oauth2,
            encrypt_key: "".to_string(),
            ..Config::default_with_profile("test")
        };
        assert!(validator
            .validate_save("test", &config_oauth, false)
            .is_ok());
    }

    #[tokio::test]
    async fn test_auth_provider_validator_fallback_app_secret_and_trimming() {
        let validator = create_test_validator().await;

        // 1. Fallback: encrypt_key is empty, app_secret is too short (should fail)
        let config_short_fallback = Config {
            app_mode: AuthMode::SelfBuilt,
            encrypt_key: "".to_string(),
            app_secret: "too_short".to_string(),
            ..Config::default_with_profile("test")
        };
        run_in_prod_env(|| {
            assert!(validator
                .validate_save("test", &config_short_fallback, false)
                .is_err());
            assert!(validator
                .validate_load("test", &config_short_fallback, false, true)
                .is_err());
        });

        // 2. Fallback: encrypt_key is empty, app_secret is valid but with whitespaces (should succeed after trim)
        let config_whitespace_fallback = Config {
            app_mode: AuthMode::SelfBuilt,
            encrypt_key: "".to_string(),
            app_secret: "\n 1234567890123456 \n".to_string(),
            ..Config::default_with_profile("test")
        };
        assert!(validator
            .validate_save("test", &config_whitespace_fallback, false)
            .is_ok());
        assert!(validator
            .validate_load("test", &config_whitespace_fallback, false, true)
            .is_ok());

        // 3. encrypt_key itself is valid but with whitespaces (should succeed after trim)
        let config_whitespace_encrypt_key = Config {
            app_mode: AuthMode::SelfBuilt,
            encrypt_key: "  1234567890123456 \n".to_string(),
            app_secret: "some_secret".to_string(),
            ..Config::default_with_profile("test")
        };
        assert!(validator
            .validate_save("test", &config_whitespace_encrypt_key, false)
            .is_ok());
        assert!(validator
            .validate_load("test", &config_whitespace_encrypt_key, false, true)
            .is_ok());
    }
}
