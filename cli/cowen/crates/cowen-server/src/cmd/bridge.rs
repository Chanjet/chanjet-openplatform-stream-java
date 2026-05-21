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
    let opts = connector_sdk::ClientOptions {
        app_key: config.app_key.clone(),
        app_secret: config.app_secret.clone(),
        gateway_url: config.openapi_url.clone(),
        exclusive: config.exclusive.unwrap_or(false),
        ..Default::default()
    };
    let client = connector_sdk::GatewayClient::new(opts);
    let pool = Arc::new(VaultTokenPool::new(vault.clone()));
    let forwarder = Arc::new(crate::daemon::forwarder::Forwarder::new(profile, config.clone(), vault.clone())?);

    let supports_webhooks = !config.webhook_target.is_empty();

    if supports_webhooks {
        let d = client.dispatcher();
        let mut dispatcher = d.lock().unwrap();

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

        let t_pool = pool.clone();
        let t_profile = profile.to_string();
        let t_config = config.clone();
        let gate_for_auth = shutdown_gate.clone();
        dispatcher.on_ent_auth_code(move |msg| {
            let temp_code = msg.biz_content.temp_auth_code.trim().to_string();
            let state = Some(msg.biz_content.state.clone());
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
        if supports_webhooks {
            client_for_stream.start_with_callback(move |state| {
                if state == connector_sdk::ConnectionState::Connected {
                    stream_notify.notify_waiters();
                }
                
                let state_str = match state {
                    connector_sdk::ConnectionState::Connected => "Connected",
                    connector_sdk::ConnectionState::Connecting => "Connecting",
                    connector_sdk::ConnectionState::Disconnected => "Disconnected",
                };
                
                let mut json = if let Ok(content) = std::fs::read_to_string(&status_file_for_stream) {
                    serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };
                
                json["state"] = serde_json::json!(state_str);
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
        if auth.requires_initial_push(&m_config).await && supports_webhooks {
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
    let supports_webhooks_for_port = supports_webhooks;
    tokio::spawn(async move {
        if let Ok(p) = port_rx.await {
            let mut json = if let Ok(content) = std::fs::read_to_string(&status_file_for_port) {
                serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
            } else {
                serde_json::json!({})
            };
            
            if !supports_webhooks_for_port {
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
            if supports_webhooks {
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
