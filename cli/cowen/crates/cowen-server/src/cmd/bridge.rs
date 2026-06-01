use cowen_common::config::Config;
use anyhow::{Result, Context};
use std::sync::Arc;
use cowen_auth::client::Client;
use cowen_auth::VaultTokenPool;
use cowen_common::vault::Vault;
use chrono::Utc;
use tokio::time::{sleep, Duration};

use tokio_util::sync::CancellationToken;
use crate::utils::shutdown::ShutdownGate;

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
    let app_cfg = cowen_config::ConfigManager::new()?.load_app_config().await?;
    let opts = build_client_options(profile, config, &app_cfg).await;
    let client = connector_sdk::GatewayClient::new(opts);
    let pool = Arc::new(VaultTokenPool::new(vault.clone()));
    let auth = cowen_auth::create_auth_client(pool.clone());
    let forwarder = Arc::new(crate::daemon::forwarder::Forwarder::new(profile, config.clone(), &app_cfg, vault.clone())?);

    let enable_webhook_forwarding = should_enable_webhooks(config, &auth);
    let requires_stream = auth.supports_webhooks(config);

    if requires_stream {
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
                let event = cowen_auth::provider::PlatformEvent::TempAuthCode { code: temp_code, state };
                let _ = auth.handle_platform_event(&t_profile_inner, &t_config_inner, event).await;
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
                let _ = auth.handle_platform_event(&pk_profile_inner, &pk_config_inner, cowen_auth::provider::PlatformEvent::AppTicket(ticket_val)).await;
            });
            true
        });
    }

    let status_file = cowen_common::config::get_app_dir().join(format!("{}_status.json", profile));
    let (port_tx, port_rx) = tokio::sync::oneshot::channel();
    
    let p_profile = profile.to_string();
    let p_config = config.clone();
    let p_vault = vault.clone();
    let proxy_fut = async move {
        if enable_proxy {
            if let Err(e) = crate::daemon::proxy::start_proxy(&p_profile, &p_config, p_vault, proxy_port, Some(port_tx)).await {
                return Err(anyhow::anyhow!("Proxy server crashed: {}", e));
            }
        }
        std::future::pending::<Result<()>>().await
    };

    let connected_notify = Arc::new(tokio::sync::Notify::new());
    let client_ptr = Arc::new(client);
    let client_for_stream = client_ptr.clone();
    let stream_notify = connected_notify.clone();
    let status_file_for_stream = status_file.clone();
    let stream_fut = async move {
        if requires_stream {
            client_for_stream.start_with_callback(move |state| {
                if state == connector_sdk::ConnectionState::Connected {
                    stream_notify.notify_waiters();
                }
                
                let (state_str, error_msg) = match state {
                    connector_sdk::ConnectionState::Connected => ("Connected", None),
                    connector_sdk::ConnectionState::Connecting => ("Connecting", None),
                    connector_sdk::ConnectionState::Disconnected => ("Disconnected", None),
                    connector_sdk::ConnectionState::DisconnectedWithError(err) => ("Disconnected", Some(err)),
                };
                
                let mut json = if let Ok(content) = std::fs::read_to_string(&status_file_for_stream) {
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
                let _ = std::fs::write(&status_file_for_stream, serde_json::to_string(&json).unwrap_or_default());
                
            }).await.map_err(|e| anyhow::anyhow!("{:?}", e)).context("Stream client crashed during connection")
        } else {
            std::future::pending::<Result<()>>().await
        }
    };

    let m_profile = profile.to_string();
    let m_config = config.clone();
    let m_pool = pool.clone();
    let maintenance_notify = connected_notify.clone();
    let maintenance_fut = async move {
        let auth = cowen_auth::create_auth_client(m_pool);
        if auth.requires_initial_push(&m_config).await && requires_stream {
            maintenance_notify.notified().await;
            let _ = auth.trigger_push(&m_profile, &m_config, true).await;
        }
        loop {
            let mut next_delay = Duration::from_secs(600);
            if let Err(_e) = auth.on_maintenance_tick(&m_profile, &m_config).await {
                next_delay = Duration::from_secs(60);
            } else {
                if let Ok(token) = auth.get_app_access_token(&m_profile, &m_config).await {
                    next_delay = crate::cmd::renewer::calculate_next_check_delay(token.expires_at, Utc::now());
                }
                if m_config.app_mode != cowen_common::models::AuthMode::Oauth2 {
                    match auth.get_token(&m_profile, &m_config, &reqwest::header::HeaderMap::new()).await {
                        Ok(t) => tracing::info!(target: "sys", profile = %m_profile, token = %t.value, "Bridge synced token successfully"),
                        Err(e) => tracing::warn!(target: "sys", profile = %m_profile, error = %e, "Bridge token sync failed"),
                    }
                }
            }
            sleep(next_delay).await;
        }
    };

    let client_ptr_clone = client_ptr.clone();
    let status_file_for_port = status_file.clone();
    let requires_stream_for_port = requires_stream;
    tokio::spawn(async move {
        if let Ok(p) = port_rx.await {
            let mut json = if let Ok(content) = std::fs::read_to_string(&status_file_for_port) {
                serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
            } else {
                serde_json::json!({})
            };
            
            if !requires_stream_for_port {
                json["state"] = serde_json::json!("Active");
            } else if json.get("state").is_none() {
                json["state"] = serde_json::json!("Connecting");
            }
            
            json["client_id"] = serde_json::json!(client_ptr_clone.client_id());
            json["proxy_port"] = serde_json::json!(p);
            json["updated_at"] = serde_json::json!(chrono::Utc::now().to_rfc3339());
            
            let _ = std::fs::write(&status_file_for_port, serde_json::to_string(&json).unwrap_or_default());
        }
    });

    tokio::select! {
        res = proxy_fut => res,
        res = stream_fut => res,
        _ = maintenance_fut => Ok(()),
        _ = cancel_token.cancelled() => {
            tracing::info!(target: "sys", profile = %profile, "Shutdown signal received, initiating graceful shutdown sequence.");
            
            // Phase 1: Stop accepting new streams/events
            if requires_stream {
                tracing::info!(target: "sys", profile = %profile, "Stopping stream client...");
                client_ptr.stop();
            }

            // Phase 2: Draining active tasks
            let active_count = shutdown_gate.active_count();
            if active_count > 0 {
                tracing::info!(target: "sys", profile = %profile, tasks = active_count, "Waiting for active tasks to complete (up to 10s)...");
                
                let drain_timeout = tokio::time::timeout(
                    Duration::from_secs(10), 
                    shutdown_gate.wait_for_zero()
                );
                
                match drain_timeout.await {
                    Ok(_) => tracing::info!(target: "sys", profile = %profile, "All active tasks completed gracefully."),
                    Err(_) => tracing::warn!(target: "sys", profile = %profile, "Timeout waiting for active tasks. Forcing shutdown."),
                }
            } else {
                tracing::info!(target: "sys", profile = %profile, "No active tasks, proceeding with shutdown.");
            }

            // Phase 3: Storage layer safe recycle
            tracing::info!(target: "sys", profile = %profile, "Shutting down storage connections...");
            if let Err(e) = vault.shutdown().await {
                tracing::error!(target: "sys", profile = %profile, error = %e, "Error shutting down storage");
            } else {
                tracing::info!(target: "sys", profile = %profile, "Storage connections closed gracefully.");
            }

            Ok(())
        }
    }
}

pub async fn build_client_options(profile: &str, config: &Config, app_cfg: &cowen_common::config::AppConfig) -> connector_sdk::ClientOptions {
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
        assert!(!supports, "Oauth2 mode should not support webhooks even with target set");

        // 2. SelfBuilt mode with non-empty webhook target -> should be true
        let mut config_self = Config::default_with_profile("p1");
        config_self.app_mode = AuthMode::SelfBuilt;
        config_self.webhook_target = "http://localhost:8080".to_string();
        
        let supports = should_enable_webhooks(&config_self, &auth);
        assert!(supports, "SelfBuilt mode should support webhooks with target set");

        // 3. SelfBuilt mode with empty webhook target -> should be false
        let mut config_self_empty = Config::default_with_profile("p1");
        config_self_empty.app_mode = AuthMode::SelfBuilt;
        config_self_empty.webhook_target = "".to_string();
        
        let supports = should_enable_webhooks(&config_self_empty, &auth);
        assert!(!supports, "SelfBuilt mode should not support webhooks if target is empty");
    }
}

