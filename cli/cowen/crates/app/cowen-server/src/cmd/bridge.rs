use anyhow::{Context, Result};
use chrono::Utc;
use cowen_auth::client::Client;
use cowen_auth::VaultTokenPool;
use cowen_common::config::Config;
use cowen_common::vault::Vault;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

use crate::utils::shutdown::ShutdownGate;
use tokio_util::sync::CancellationToken;

fn should_enable_webhooks(config: &Config, auth: &dyn Client) -> bool {
    !config.webhook_target.is_empty() && auth.supports_webhooks(config)
}

/// 自建模式专用流桥执行器
/// 负责处理 WebSocket 长连接、API 反向代理以及消息转发
pub async fn run(
    profile: &str,
    config: &Config,
    vault: Arc<dyn Vault>,
    proxy_port: u16,
    enable_proxy: bool,
    _is_distributed: bool,
    cancel_token: CancellationToken,
    shutdown_gate: ShutdownGate,
) -> Result<()> {
    let app_cfg = cowen_config::ConfigManager::new()?
        .load_app_config()
        .await?;
    let opts = build_client_options(profile, config, &app_cfg).await;
    let client = connector_sdk::GatewayClient::new(opts);
    let pool = Arc::new(VaultTokenPool::new(vault.clone()));
    let auth = cowen_auth::create_auth_client(pool.clone());
    let forwarder = Arc::new(cowen_capabilities::internal::forwarder::Forwarder::new(
        profile,
        config.clone(),
        &app_cfg,
        vault.clone(),
    )?);

    let enable_webhook_forwarding = should_enable_webhooks(config, &auth);
    let requires_stream = auth.supports_webhooks(config);

    setup_dispatcher(
        &client,
        requires_stream,
        enable_webhook_forwarding,
        forwarder,
        shutdown_gate.clone(),
        pool.clone(),
        profile,
        config,
    );

    let status_file = cowen_common::config::get_app_dir().join(format!("{}_status.json", profile));
    let (port_tx, port_rx) = tokio::sync::oneshot::channel();
    let connected_notify = Arc::new(tokio::sync::Notify::new());
    let client_ptr = Arc::new(client);

    spawn_status_writers(
        requires_stream,
        client_ptr.client_id().to_string(),
        status_file.clone(),
        port_rx,
        cancel_token.clone(),
    );

    tokio::select! {
        res = run_proxy_server(enable_proxy, profile.to_string(), config.clone(), vault.clone(), proxy_port, port_tx) => res,
        res = run_gateway_server(profile.to_string(), config.clone(), vault.clone(), app_cfg.clone()) => res,
        res = run_stream_client(requires_stream, client_ptr.clone(), connected_notify.clone(), status_file) => res,
        _ = run_maintenance_loop(requires_stream, pool, profile.to_string(), config.clone(), connected_notify) => Ok(()),
        _ = cancel_token.cancelled() => {
            execute_graceful_shutdown(profile, requires_stream, client_ptr, shutdown_gate, vault).await
        }
    }
}

fn setup_dispatcher(
    client: &connector_sdk::GatewayClient,
    requires_stream: bool,
    enable_webhook_forwarding: bool,
    forwarder: Arc<cowen_capabilities::internal::forwarder::Forwarder>,
    shutdown_gate: ShutdownGate,
    pool: Arc<VaultTokenPool>,
    profile: &str,
    config: &Config,
) {
    if !requires_stream {
        return;
    }
    let d = client.dispatcher();
    let mut dispatcher = d.lock().unwrap_or_else(|e| e.into_inner());

    if enable_webhook_forwarding {
        let fwd = forwarder.clone();
        let gate_for_fwd = shutdown_gate.clone();
        dispatcher.set_fallback_handler(Arc::new(move |msg| {
            let fwd_clone = fwd.clone();
            let gate_guard = gate_for_fwd.enter();
            tokio::spawn(async move {
                let _guard = gate_guard; // Keep guard alive until forwarding finishes
                let _ = fwd_clone.forward(msg).await;
            });
            true
        }));
    }

    let t_pool = pool.clone();
    let t_profile = profile.to_string();
    let t_config = config.clone();
    let gate_for_auth = shutdown_gate.clone();
    dispatcher.on_ent_auth_code(move |msg| {
        let temp_code = msg.biz_content.temp_auth_code.trim().to_string();
        let state = msg.biz_content.state.clone();
        let t_pool_inner = t_pool.clone();
        let t_profile_inner = t_profile.clone();
        let t_config_inner = t_config.clone();
        let gate_guard = gate_for_auth.enter();
        tokio::spawn(async move {
            let _guard = gate_guard;
            let auth = cowen_auth::create_auth_client(t_pool_inner);
            let event = cowen_auth::provider::PlatformEvent::TempAuthCode {
                code: temp_code,
                state,
            };
            let _ = auth
                .handle_platform_event(&t_profile_inner, &t_config_inner, event)
                .await;
        });
        true
    });

    let pk_pool = pool.clone();
    let pk_profile = profile.to_string();
    let pk_config = config.clone();
    let gate_for_ticket = shutdown_gate.clone();
    dispatcher.on_app_ticket(move |msg| {
        let ticket_val = msg.biz_content.app_ticket.trim().to_string();
        let pk_pool_inner = pk_pool.clone();
        let pk_profile_inner = pk_profile.clone();
        let pk_config_inner = pk_config.clone();
        let gate_guard = gate_for_ticket.enter();
        tokio::spawn(async move {
            let _guard = gate_guard;
            let auth = cowen_auth::create_auth_client(pk_pool_inner);
            let _ = auth
                .handle_platform_event(
                    &pk_profile_inner,
                    &pk_config_inner,
                    cowen_auth::provider::PlatformEvent::AppTicket(ticket_val),
                )
                .await;
        });
        true
    });
}

async fn run_proxy_server(
    enable_proxy: bool,
    profile: String,
    config: Config,
    vault: Arc<dyn Vault>,
    proxy_port: u16,
    port_tx: tokio::sync::oneshot::Sender<u16>,
) -> Result<()> {
    if enable_proxy {
        if let Err(e) = crate::daemon::proxy::start_proxy(
            &profile,
            &config,
            vault,
            proxy_port,
            Some(port_tx),
            cowen_common::config::get_app_dir(),
        )
        .await
        {
            return Err(anyhow::anyhow!("Proxy server crashed: {}", e));
        }
    }
    std::future::pending::<Result<()>>().await
}

/// PRD v0.5.0 — Identity-Aware Gateway (Ingress reverse proxy) runner.
///
/// If the config has a `gateway` block AND the app_mode is `store-app`,
/// starts the gateway server. Otherwise, returns an error or hangs.
async fn run_gateway_server(
    profile: String,
    config: Config,
    vault: Arc<dyn Vault>,
    app_cfg: cowen_common::config::AppConfig,
) -> Result<()> {
    if let Some(ref gw_config) = config.gateway {
        // Mode validation guard (PRD §2): Gateway is ONLY for store-app
        if config.app_mode != cowen_common::models::AuthMode::StoreApp {
            return Err(anyhow::anyhow!(
                "Gateway configuration is only supported in 'store-app' mode. \
                 Current mode: '{}'. Cowen refuses to start.",
                config.app_mode
            ));
        }

        let pool = Arc::new(VaultTokenPool::new(vault.clone()));
        let auth_client: Arc<dyn Client> = Arc::new(cowen_auth::create_auth_client(pool));

        let (gw_port_tx, _gw_port_rx) = tokio::sync::oneshot::channel();

        if let Err(e) = cowen_gateway::start_gateway(
            &profile,
            &config,
            gw_config,
            &app_cfg,
            auth_client,
            vault.clone(),
            Some(gw_port_tx),
        )
        .await
        {
            return Err(anyhow::anyhow!("Gateway server crashed: {}", e));
        }
    }
    // No gateway configured — hang indefinitely (select! will cancel us)
    std::future::pending::<Result<()>>().await
}

async fn run_stream_client(
    requires_stream: bool,
    client: Arc<connector_sdk::GatewayClient>,
    connected_notify: Arc<tokio::sync::Notify>,
    status_file: std::path::PathBuf,
) -> Result<()> {
    if requires_stream {
        client
            .start_with_callback(move |state| {
                if state == connector_sdk::ConnectionState::Connected {
                    connected_notify.notify_waiters();
                }

                let (state_str, error_msg) = match state {
                    connector_sdk::ConnectionState::Connected => ("Connected", None),
                    connector_sdk::ConnectionState::Connecting => ("Connecting", None),
                    connector_sdk::ConnectionState::Disconnected => ("Disconnected", None),
                    connector_sdk::ConnectionState::DisconnectedWithError(err) => {
                        ("Disconnected", Some(err))
                    }
                };

                let mut json = if let Ok(content) = std::fs::read_to_string(&status_file) {
                    serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };

                json["state"] = serde_json::json!(state_str);
                if let Some(err) = error_msg {
                    json["error"] = serde_json::json!(err);
                } else if let Some(obj) = json.as_object_mut() {
                    obj.remove("error");
                }
                json["updated_at"] = serde_json::json!(chrono::Utc::now().to_rfc3339());
                let _ = std::fs::write(
                    &status_file,
                    serde_json::to_string(&json).unwrap_or_default(),
                );
            })
            .await
            .map_err(|e| anyhow::anyhow!("{:?}", e))
            .context("Stream client crashed during connection")
    } else {
        std::future::pending::<Result<()>>().await
    }
}

async fn run_maintenance_loop(
    requires_stream: bool,
    pool: Arc<VaultTokenPool>,
    profile: String,
    config: Config,
    connected_notify: Arc<tokio::sync::Notify>,
) -> Result<()> {
    let auth = cowen_auth::create_auth_client(pool);
    if auth.requires_initial_push(&config).await && requires_stream {
        connected_notify.notified().await;
        let _ = auth.trigger_push(&profile, &config, true).await;
    }
    loop {
        let next_delay = process_maintenance_tick(&auth, &profile, &config).await;
        sleep(next_delay).await;
    }
}

async fn process_maintenance_tick(
    auth: &cowen_auth::AuthClient,
    profile: &str,
    config: &Config,
) -> Duration {
    if let Err(_e) = auth.on_maintenance_tick(profile, config).await {
        return Duration::from_secs(60);
    }
    let mut next_delay = Duration::from_secs(600);
    if let Ok(token) = auth.get_app_access_token(profile, config).await {
        next_delay = crate::cmd::renewer::calculate_next_check_delay(token.expires_at, Utc::now());
    }
    if config.app_mode != cowen_common::models::AuthMode::Oauth2 {
        match auth
            .get_token(profile, config, &reqwest::header::HeaderMap::new())
            .await
        {
            Ok(t) => {
                tracing::info!(target: "sys", profile = %profile, token = %t.value, "Bridge synced token successfully")
            }
            Err(e) => {
                tracing::warn!(target: "sys", profile = %profile, error = %e, "Bridge token sync failed")
            }
        }
    }
    next_delay
}

fn spawn_status_writers(
    requires_stream: bool,
    client_id: String,
    status_file: std::path::PathBuf,
    port_rx: tokio::sync::oneshot::Receiver<u16>,
    cancel_token: CancellationToken,
) {
    let status_file_for_port = status_file.clone();
    tokio::spawn(async move {
        if let Ok(p) = port_rx.await {
            let mut json = if let Ok(content) = std::fs::read_to_string(&status_file_for_port) {
                serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
            } else {
                serde_json::json!({})
            };

            if !requires_stream {
                json["state"] = serde_json::json!("Active");
            } else if json.get("state").is_none() {
                json["state"] = serde_json::json!("Connecting");
            }

            json["client_id"] = serde_json::json!(client_id);
            json["proxy_port"] = serde_json::json!(p);
            json["updated_at"] = serde_json::json!(chrono::Utc::now().to_rfc3339());

            let _ = std::fs::write(
                &status_file_for_port,
                serde_json::to_string(&json).unwrap_or_default(),
            );
        }
    });

    let status_file_for_heartbeat = status_file.clone();
    tokio::spawn(async move {
        if requires_stream {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(30)) => {
                        if let Ok(content) = std::fs::read_to_string(&status_file_for_heartbeat) {
                            if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&content) {
                                json["updated_at"] = serde_json::json!(chrono::Utc::now().to_rfc3339());
                                let _ = std::fs::write(&status_file_for_heartbeat, serde_json::to_string(&json).unwrap_or_default());
                            }
                        }
                    }
                    _ = cancel_token.cancelled() => {
                        break;
                    }
                }
            }
        }
    });
}

async fn execute_graceful_shutdown(
    profile: &str,
    requires_stream: bool,
    client_ptr: Arc<connector_sdk::GatewayClient>,
    shutdown_gate: ShutdownGate,
    vault: Arc<dyn Vault>,
) -> Result<()> {
    tracing::info!(target: "sys", profile = %profile, "Shutdown signal received, initiating graceful shutdown sequence.");

    stop_stream_client(profile, requires_stream, client_ptr);
    drain_active_tasks(profile, shutdown_gate).await;
    shutdown_storage_layer(profile, vault).await;

    Ok(())
}

fn stop_stream_client(
    profile: &str,
    requires_stream: bool,
    client_ptr: Arc<connector_sdk::GatewayClient>,
) {
    if requires_stream {
        tracing::info!(target: "sys", profile = %profile, "Stopping stream client...");
        client_ptr.stop();
    }
}

async fn drain_active_tasks(profile: &str, shutdown_gate: ShutdownGate) {
    let active_count = shutdown_gate.active_count();
    if active_count > 0 {
        tracing::info!(target: "sys", profile = %profile, tasks = active_count, "Waiting for active tasks to complete (up to 10s)...");

        let drain_timeout =
            tokio::time::timeout(Duration::from_secs(10), shutdown_gate.wait_for_zero());

        match drain_timeout.await {
            Ok(_) => {
                tracing::info!(target: "sys", profile = %profile, "All active tasks completed gracefully.")
            }
            Err(_) => {
                tracing::warn!(target: "sys", profile = %profile, "Timeout waiting for active tasks. Forcing shutdown.")
            }
        }
    } else {
        tracing::info!(target: "sys", profile = %profile, "No active tasks, proceeding with shutdown.");
    }
}

async fn shutdown_storage_layer(profile: &str, vault: Arc<dyn Vault>) {
    tracing::info!(target: "sys", profile = %profile, "Shutting down storage connections...");
    if let Err(e) = vault.shutdown().await {
        tracing::error!(target: "sys", profile = %profile, error = %e, "Error shutting down storage");
    } else {
        tracing::info!(target: "sys", profile = %profile, "Storage connections closed gracefully.");
    }
}

pub async fn build_client_options(
    profile: &str,
    config: &Config,
    app_cfg: &cowen_common::config::AppConfig,
) -> connector_sdk::ClientOptions {
    let mut dlq_provider: Option<std::sync::Arc<dyn connector_sdk::dlq::DlqProvider>> = None;

    let app_dir = cowen_common::config::get_app_dir();
    let dlq_db_path = app_dir.join(format!("{}_dlq.db", profile));
    match cowen_store::dlq_store::sqlite::SqliteDlqProvider::new(&dlq_db_path).await {
        Ok(provider) => dlq_provider = Some(std::sync::Arc::new(provider)),
        Err(e) => tracing::warn!("Failed to initialize DLQ store at {:?}: {}", dlq_db_path, e),
    }

    connector_sdk::ClientOptions {
        app_key: config.app_key.clone(),
        app_secret: config.app_secret.clone(),
        encrypt_key: if config.encrypt_key.is_empty() {
            None
        } else {
            Some(config.encrypt_key.clone())
        },
        gateway_url: app_cfg.stream_url.clone(), // 👈 修正为正确的 stream_url
        reconnect_interval: Duration::from_secs(1),
        max_backoff: Duration::from_secs(60),
        exclusive: config.exclusive.unwrap_or(false),
        dlq_provider,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cowen_common::config::Config;
    use cowen_common::models::AuthMode;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_build_client_options_uses_stream_url() {
        let config = Config {
            app_key: "test_key".to_string(),
            app_secret: "test_secret".to_string(),
            ..Config::default_with_profile("test")
        };
        let app_cfg = cowen_common::config::AppConfig {
            openapi_url: "https://openapi.chanjet.com".to_string(),
            stream_url: "https://stream-open.chanapp.chanjet.com".to_string(),
            ..Default::default()
        };

        let opts = build_client_options("test", &config, &app_cfg).await;
        // 断言它应该取 stream_url
        assert_eq!(opts.gateway_url, "https://stream-open.chanapp.chanjet.com");
    }

    #[tokio::test]
    async fn test_build_client_options_populates_encrypt_key() {
        let config = Config {
            app_key: "test_key".to_string(),
            app_secret: "test_secret".to_string(),
            encrypt_key: "1234567890123456".to_string(),
            ..Config::default_with_profile("test")
        };
        let app_cfg = cowen_common::config::AppConfig::default();
        let opts = build_client_options("test", &config, &app_cfg).await;
        assert_eq!(opts.encrypt_key, Some("1234567890123456".to_string()));
    }

    #[tokio::test]
    async fn test_should_enable_webhooks_for_different_modes() {
        let tmp = tempdir().unwrap();
        let store = Arc::new(cowen_store::FileStore::new(tmp.path(), Some("fingerprint")).unwrap());
        let vault = Arc::new(cowen_store::StoreVault::new(store.clone(), store.clone()));
        let auth = cowen_auth::create_auth_client_with_vault(vault);

        // 1. Oauth2 mode with non-empty webhook target -> should be false
        let mut config_oauth2 = Config::default_with_profile("p1");
        config_oauth2.app_mode = AuthMode::Oauth2;
        config_oauth2.webhook_target = "http://localhost:8080".to_string();

        let supports = should_enable_webhooks(&config_oauth2, &auth);
        assert!(
            !supports,
            "Oauth2 mode should not support webhooks even with target set"
        );

        // 2. SelfBuilt mode with non-empty webhook target -> should be true
        let mut config_self = Config::default_with_profile("p1");
        config_self.app_mode = AuthMode::SelfBuilt;
        config_self.webhook_target = "http://localhost:8080".to_string();

        let supports = should_enable_webhooks(&config_self, &auth);
        assert!(
            supports,
            "SelfBuilt mode should support webhooks with target set"
        );

        // 3. SelfBuilt mode with empty webhook target -> should be false
        let mut config_self_empty = Config::default_with_profile("p1");
        config_self_empty.app_mode = AuthMode::SelfBuilt;
        config_self_empty.webhook_target = "".to_string();

        let supports = should_enable_webhooks(&config_self_empty, &auth);
        assert!(
            !supports,
            "SelfBuilt mode should not support webhooks if target is empty"
        );
    }
}
