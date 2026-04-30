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
            Arc::new(provider::self_built::SelfBuiltProvider::with_sender(pool.clone(), http_sender.clone())),
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
pub fn create_auth_client_with_vault(vault: Arc<dyn crate::core::vault::Vault>) -> AuthClient {
    let pool = Arc::new(pool::VaultTokenPool::new(vault));
    create_auth_client(pool)
}
