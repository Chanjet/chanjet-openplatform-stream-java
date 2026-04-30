use crate::core::config::Config;
use anyhow::{Result, Context};
use connector_sdk::{GatewayClient, ClientOptions};
use std::sync::Arc;
use crate::daemon::dlq::DlqStore;
use crate::daemon::forwarder::Forwarder;
use crate::daemon::proxy::start_proxy;
use crate::auth::client::Client;
use crate::auth::VaultTokenPool;
use crate::core::vault::Vault;
use tokio::time::{sleep, Duration};

/// 自建模式专用流桥执行器
/// 负责处理 WebSocket 长连接、API 反向代理以及消息转发
pub async fn run(profile: &str, config: &Config, vault: Arc<dyn Vault>, proxy_port: u16, enable_proxy: bool) -> Result<()> {
    let options = ClientOptions {
        app_key: config.app_key.clone(),
        app_secret: config.app_secret.clone(),
        encrypt_key: Some(config.encrypt_key.clone()),
        gateway_url: config.stream_url.clone(),
    };
    let client = Arc::new(GatewayClient::new(options));
    let pool = Arc::new(VaultTokenPool::new(vault.clone()));
    let auth = crate::auth::create_auth_client(pool.clone());
    let supports_webhooks = auth.supports_webhooks(config);

    // 1. Task: Local Proxy
    let p_profile_proxy = profile.to_string();
    let p_config_proxy = config.clone();
    let proxy_task = if enable_proxy {
        tokio::spawn(async move {
            if let Err(e) = start_proxy(&p_profile_proxy, &p_config_proxy, proxy_port).await {
                tracing::error!(target: "sys", error = %e, "Local Proxy Server crashed");
            }
            std::future::pending::<()>().await;
        })
    } else {
        tokio::spawn(async move {
            tracing::info!(target: "sys", "Local Proxy Server is disabled.");
            std::future::pending::<()>().await;
        })
    };

    // 2. Setup Dispatchers (Conditional)
    if supports_webhooks {
        let dlq = Arc::new(DlqStore::new(profile, vault.clone())?);
        let forwarder = Forwarder::new(dlq, &config.webhook_target);
        let d = client.dispatcher();
        let mut dispatcher = d.lock().unwrap();

        let fwd = forwarder.clone();
        dispatcher.set_fallback_handler(Arc::new(move |msg| {
            let fwd_clone = fwd.clone();
            tokio::spawn(async move {
                fwd_clone.forward(msg).await;
            });
            true
        }));

        // 🚀 OCP: Generic Platform Event Handlers
        let t_pool = pool.clone();
        let t_profile = profile.to_string();
        let t_config = config.clone();
        
        dispatcher.on_ent_auth_code(move |msg| {
            let temp_code = msg.biz_content.temp_auth_code.trim().to_string();
            // Try to extract org_id from the message headers if available
            let org_id = msg.base.headers.get("orgId").cloned().unwrap_or_default(); 
            
            let t_pool_inner = t_pool.clone();
            let t_profile_inner = t_profile.clone();
            let t_config_inner = t_config.clone();
            
            tokio::spawn(async move {
                let auth = crate::auth::create_auth_client(t_pool_inner.clone());
                let event = crate::auth::provider::PlatformEvent::TempAuthCode {
                    code: temp_code,
                    org_id: Some(org_id),
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
            let pk_pool_inner = pk_pool.clone();
            let pk_profile_inner = pk_profile.clone();
            let pk_config_inner = pk_config.clone();
            
            tokio::spawn(async move {
                let auth = crate::auth::create_auth_client(pk_pool_inner.clone());
                let _ = auth.handle_platform_event(&pk_profile_inner, &pk_config_inner, crate::auth::provider::PlatformEvent::AppTicket(ticket_val)).await;
            });
            true
        });
    }

    // 3. Task: Main Gateway Loop (Conditional)
    let stream_task = if supports_webhooks {
        tokio::spawn(async move {
            client.start().await
        })
    } else {
        let mode = config.app_mode;
        tokio::spawn(async move {
            tracing::info!(target: "sys", "Streaming bridge is disabled for mode {:?}.", mode);
            std::future::pending::<Result<()>>().await
        })
    };

    // 4. Task: Background Maintenance (OCP Generic)
    let p_profile_m = profile.to_string();
    let p_config_m = config.clone();
    let p_pool_m = pool.clone();
    let maintenance_task = tokio::spawn(async move {
        sleep(Duration::from_secs(2)).await;
        let auth = crate::auth::create_auth_client(p_pool_m.clone());

        // 🚀 OCP: Generic Initial Push Check
        if auth.requires_initial_push(&p_config_m) && supports_webhooks {
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
    });

    tracing::info!(target: "sys", "All bridge tasks initialized. Entering watchdog mode.");

    tokio::select! {
        res = proxy_task => { 
            tracing::error!(target: "sys", "Proxy task exited unexpectedly"); 
            res.map_err(|e| anyhow::anyhow!("Proxy task panicked: {}", e)).and_then(|_| Err(anyhow::anyhow!("Proxy task stopped"))) as Result<()>
        },
        res = stream_task => { 
            if supports_webhooks {
                tracing::error!(target: "sys", "Stream task exited unexpectedly"); 
                res.map_err(|e| anyhow::anyhow!("Stream task panicked: {}", e))
                   .and_then(|r: Result<()>| r.context("Stream client crashed"))
            } else {
                Ok(())
            }
        },
        res = maintenance_task => { 
            tracing::error!(target: "sys", "Maintenance task exited unexpectedly"); 
            res.map_err(|e| anyhow::anyhow!("Maintenance task panicked: {}", e)).and_then(|_| Err(anyhow::anyhow!("Maintenance task stopped"))) as Result<()>
        },
    }
}
