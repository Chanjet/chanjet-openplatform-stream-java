use cowen_common::{CowenResult, CowenError};
pub mod models;
pub mod pool;
pub mod client;
pub mod decorator;
pub mod provider;
pub mod lifecycle;

pub use pool::VaultTokenPool;
pub use client::AuthClient;
pub use decorator::RequestDecorator;

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
            Arc::new(provider::self_built::SelfBuiltProvider::new(pool.clone(), http_sender.clone())),
        )
        .register(
            models::AuthMode::Oauth2,
            Arc::new(provider::oauth2::OAuth2Provider::new(pool.clone(), http_sender.clone())),
        )

        .register(
            models::AuthMode::StoreApp,
            Arc::new(provider::store_app::StoreAppProvider::new(pool.clone(), http_sender)),
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
pub struct AuthProviderValidator {
    client: AuthClient,
}

impl AuthProviderValidator {
    pub fn new(client: AuthClient) -> Self {
        Self { client }
    }
}

impl cowen_store::ConfigValidator for AuthProviderValidator {
    fn validate_load(&self, profile: &str, config: &cowen_common::config::Config, is_distributed: bool, exists: bool) -> CowenResult<()> {
        if is_distributed && exists {
            let provider = self.client.provider(&config.app_mode);
            if !provider.is_allowed_in_distributed_storage() {
                let msg = format!("⚠️  Skipping profile '{}': Auth mode '{:?}' is not allowed in distributed storage scenarios (shared database/redis).", profile, config.app_mode);
                eprintln!("{}", msg);
                return Err(CowenError::Internal(format!("SKIPPED: {}", msg)));
            }
        }
        Ok(())
    }

    fn validate_save(&self, _profile: &str, config: &cowen_common::config::Config, is_distributed: bool) -> CowenResult<()> {
        if is_distributed {
            let provider = self.client.provider(&config.app_mode);
            if !provider.is_allowed_in_distributed_storage() {
                return Err(CowenError::Config(format!("Auth mode '{:?}' is not allowed in distributed storage scenarios. Please use Sidecar or SelfBuilt mode for distributed deployments.", config.app_mode)));
            }
        }
        Ok(())
    }
}
