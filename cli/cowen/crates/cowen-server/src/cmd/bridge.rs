use cowen_common::config::Config;
use anyhow::{Result, Context};
use connector_sdk::{GatewayClient, ClientOptions};
use std::sync::Arc;
use crate::daemon::dlq::DlqStore;
use crate::daemon::forwarder::Forwarder;
use crate::daemon::proxy::start_proxy;
use cowen_auth::client::Client;
use cowen_auth::VaultTokenPool;
use cowen_common::vault::Vault;
use tokio::time::{sleep, Duration};

/// 自建模式专用流桥执行器
/// 负责处理 WebSocket 长连接、API 反向代理以及消息转发
pub async fn run(profile: &str, config: &Config, vault: Arc<dyn Vault>, proxy_port: u16, enable_proxy: bool, is_distributed: bool) -> Result<()> {
    let exclusive = config.exclusive
        .unwrap_or_else(|| !is_distributed && config.app_mode != cowen_common::models::AuthMode::Oauth2);

    let options = ClientOptions {
        app_key: config.app_key.clone(),
        app_secret: config.app_secret.clone(),
        encrypt_key: Some(config.encrypt_key.clone()),
        gateway_url: config.stream_url.clone(),
        exclusive,
        ..Default::default()
    };
    let client = Arc::new(GatewayClient::new(options));
    let pool = Arc::new(VaultTokenPool::new(vault.clone()));
    let auth = cowen_auth::create_auth_client(pool.clone());
    let supports_webhooks = auth.supports_webhooks(config);

    // 2. Setup Dispatchers (Conditional)
    if supports_webhooks {
        let forwarder = Forwarder::new(profile, config.clone(), vault.clone());
        let d = client.dispatcher();
        let mut dispatcher = d.lock().unwrap();

        let fwd = forwarder.clone();
        dispatcher.set_fallback_handler(Arc::new(move |msg| {
            let fwd_clone = fwd.clone();
            tokio::spawn(async move {
                let _ = fwd_clone.forward(msg).await;
            });
            true
        }));

        // 🚀 OCP: Generic Platform Event Handlers
        let t_pool = pool.clone();
        let t_profile = profile.to_string();
        let t_config = config.clone();
        
        dispatcher.on_ent_auth_code(move |msg| {
            let temp_code = msg.biz_content.temp_auth_code.trim().to_string();
            let state = Some(msg.biz_content.state.clone());
            
            let t_pool_inner = t_pool.clone();
            let t_profile_inner = t_profile.clone();
            let t_config_inner = t_config.clone();
            
            tokio::spawn(async move {
                let auth = cowen_auth::create_auth_client(t_pool_inner.clone());
                let event = cowen_auth::provider::PlatformEvent::TempAuthCode {
                    code: temp_code,
                    state,
                };
                let _ = auth.handle_platform_event(&t_profile_inner, &t_config_inner, event).await;
            });
            true
        });

        let pk_pool = pool.clone();
        let pk_profile = profile.to_string();
        let pk_config = config.clone();
        dispatcher.on_app_ticket(move |msg| {
            let ticket_val = msg.biz_content.app_ticket.trim().to_string();
            tracing::info!(target: "stream", "CALLBACK: AppTicket received in dispatcher (masked: {}...)", &ticket_val[..5]);
            
            let pk_pool_inner = pk_pool.clone();
            let pk_profile_inner = pk_profile.clone();
            let pk_config_inner = pk_config.clone();
            
            tokio::spawn(async move {
                let auth = cowen_auth::create_auth_client(pk_pool_inner.clone());
                let _ = auth.handle_platform_event(&pk_profile_inner, &pk_config_inner, cowen_auth::provider::PlatformEvent::AppTicket(ticket_val)).await;
            });
            true
        });
    }

    tracing::info!(target: "sys", "All bridge tasks initialized. Entering watchdog mode.");

    // Define futures directly for select! (NO tokio::spawn here for core tasks)
    // This ensures that when the run() future is dropped (e.g. during reload),
    // all component tasks are also cancelled.
    
    let p_profile_proxy = profile.to_string();
    let p_config_proxy = config.clone();
    let proxy_fut = async move {
        if enable_proxy {
            if let Err(e) = start_proxy(&p_profile_proxy, &p_config_proxy, proxy_port).await {
                tracing::error!(target: "sys", error = %e, "Local Proxy Server crashed");
                return Err(anyhow::anyhow!("Proxy server crashed: {}", e));
            }
        } else {
            tracing::info!(target: "sys", "Local Proxy Server is disabled.");
        }
        std::future::pending::<Result<()>>().await
    };

    let p_profile_s = profile.to_string();
    let stream_fut = async move {
        if supports_webhooks {
            let status_file = cowen_common::config::get_app_dir().join(format!("{}_status.json", p_profile_s));

            // Use the SDK's internal reconnection loop, but hook into its status callbacks
            let now = chrono::Utc::now().to_rfc3339();
            let _ = std::fs::write(&status_file, format!("{{\"state\":\"Starting\", \"updated_at\":\"{}\"}}", now));
            
            let res = client.start_with_callback(move |state| {
                let state_str = match state {
                    connector_sdk::ConnectionState::Connecting => "Connecting",
                    connector_sdk::ConnectionState::Connected => "Connected",
                    connector_sdk::ConnectionState::Disconnected => "Disconnected",
                };
                tracing::info!(target: "stream", profile = %p_profile_s, state = %state_str, "Bridge connection state changed");
                let now = chrono::Utc::now().to_rfc3339();
                let _ = std::fs::write(&status_file, format!("{{\"state\":\"{}\", \"updated_at\":\"{}\"}}", state_str, now));
            }).await;

            if let Err(e) = res {
                tracing::error!(target: "sys", error = %e, "Stream client loop terminated");
                return Err(e);
            }
            Ok(())
        } else {
            let mode = config.app_mode;
            tracing::info!(target: "sys", "Streaming bridge is disabled for mode {:?}.", mode);
            std::future::pending::<Result<()>>().await
        }
    };

    let p_profile_h = profile.to_string();
    let heartbeat_fut = async move {
        if supports_webhooks {
            let status_file = cowen_common::config::get_app_dir().join(format!("{}_status.json", p_profile_h));
            loop {
                sleep(Duration::from_secs(30)).await;
                if let Ok(content) = std::fs::read_to_string(&status_file) {
                    if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&content) {
                        let now = chrono::Utc::now().to_rfc3339();
                        if let Some(obj) = json.as_object_mut() {
                            obj.insert("updated_at".to_string(), serde_json::Value::String(now));
                            let _ = std::fs::write(&status_file, serde_json::to_string(&json).unwrap_or_default());
                        }
                    }
                }
            }
        } else {
            std::future::pending::<()>().await
        }
    };

    let p_profile_m = profile.to_string();
    let p_config_m = config.clone();
    let p_pool_m = pool.clone();
    let maintenance_fut = async move {
        sleep(Duration::from_secs(2)).await;
        let auth = cowen_auth::create_auth_client(p_pool_m.clone());

        // 🚀 OCP: Generic Initial Push Check
        if auth.requires_initial_push(&p_config_m).await && supports_webhooks {
            tracing::info!(target: "sys", "Initial credential missing. Requesting platform push...");
            let _ = auth.trigger_push(&p_profile_m, &p_config_m, true).await;
        }

        loop {
            tracing::info!(target: "sys", "Running bridge credential maintenance check...");
            // 🚀 OCP: Generic Maintenance Tick
            if let Err(e) = auth.on_maintenance_tick(&p_profile_m, &p_config_m).await {
                tracing::warn!(target: "sys", error = %e, "Maintenance tick failed");
            }
            sleep(Duration::from_secs(600)).await;
        }
    };

    tokio::select! {
        res = proxy_fut => { 
            tracing::error!(target: "sys", "Proxy task exited unexpectedly"); 
            res.context("Proxy task stopped")
        },
        res = stream_fut => { 
            if supports_webhooks {
                tracing::error!(target: "sys", "Stream task exited unexpectedly"); 
                res.context("Stream client crashed")
            } else {
                Ok(())
            }
        },
        _ = maintenance_fut => { 
            tracing::error!(target: "sys", "Maintenance task exited unexpectedly"); 
            Err(anyhow::anyhow!("Maintenance task stopped"))
        },
        _ = heartbeat_fut => {
            Ok(())
        },
    }
}
