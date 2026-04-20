use crate::core::config::Config;
use anyhow::{Result, Context};
use connector_sdk::{GatewayClient, ClientOptions};
use std::sync::Arc;
use crate::daemon::dlq::DlqStore;
use crate::daemon::forwarder::Forwarder;
use crate::daemon::proxy::start_proxy;
use crate::auth::client::Client;
use crate::auth::{VaultTokenPool, AuthClient, pool::TokenPool, models::Ticket};
use crate::core::vault::Vault;
use chrono::Utc;
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
    let _auth = AuthClient::new(pool.as_ref());
    
    let dlq = Arc::new(DlqStore::new(profile)?);
    let forwarder = Forwarder::new(dlq, &config.webhook_target);

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

    // 2. Setup Dispatchers
    {
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

        let p_pool = pool.clone();
        let p_profile = profile.to_string();
        let p_config = config.clone();
        
        dispatcher.on_app_ticket(move |msg| {
            let ticket_val = msg.biz_content.app_ticket.trim();
            tracing::info!(target: "stream", "AppTicket received from platform");
            
            let ticket = Ticket {
                value: ticket_val.to_string(),
                created_at: Utc::now(),
            };
            
            if let Err(e) = p_pool.set_app_ticket(&p_profile, &ticket) {
                tracing::error!(target: "sys", error = %e, "Failed to save ticket to vault");
            } else {
                tracing::info!(target: "sys", "AppTicket saved to vault correctly");
                let inner_pool = p_pool.clone();
                let inner_profile = p_profile.clone();
                let inner_config = p_config.clone();
                tokio::spawn(async move {
                    let auth = AuthClient::new(inner_pool.as_ref());
                    if let Err(e) = auth.get_app_access_token(&inner_profile, &inner_config).await {
                        tracing::error!(target: "sys", error = %e, "Automatic token refresh failed");
                    } else {
                        tracing::info!(target: "sys", "AccessToken proactively refreshed");
                    }
                });
            }
            true
        });
    }

    // 3. Task: Stream Bridge
    let stream_client = client.clone();
    let stream_task = tokio::spawn(async move {
        if let Err(e) = stream_client.start().await {
            tracing::error!(target: "sys", error = %e, "Stream Bridge loop terminated with error");
            Err(e)
        } else {
            Ok(())
        }
    });

    // 4. Task: Maintenance (AppTicket Trigger)
    let p_profile_m = profile.to_string();
    let p_config_m = config.clone();
    let p_pool_m = pool.clone();
    let maintenance_task = tokio::spawn(async move {
        sleep(Duration::from_secs(2)).await;
        let auth = AuthClient::new(p_pool_m.as_ref());

        if p_pool_m.get_app_ticket(&p_profile_m).is_err() {
            tracing::info!(target: "sys", "Initial AppTicket missing. Requesting platform push...");
            let _ = auth.trigger_push(&p_profile_m, &p_config_m, true).await;
        }

        loop {
            tracing::info!(target: "sys", "Running bridge credential maintenance check...");
            match auth.get_app_access_token(&p_profile_m, &p_config_m).await {
                Ok(token) => {
                    let remaining = token.expires_at.signed_duration_since(Utc::now());
                    if remaining < chrono::Duration::minutes(15) {
                        let _ = auth.refresh_app_access_token(&p_profile_m, &p_config_m).await;
                    }
                }
                Err(_) => {
                    let _ = auth.trigger_push(&p_profile_m, &p_config_m, false).await;
                }
            }
            sleep(Duration::from_secs(600)).await;
        }
    });

    tracing::info!(target: "sys", "All bridge tasks initialized. Entering watchdog mode.");

    tokio::select! {
        res = proxy_task => { 
            tracing::error!(target: "sys", "Proxy task exited unexpectedly"); 
            res.map_err(|e| anyhow::anyhow!("Proxy task panicked: {}", e)).and_then(|_| Err(anyhow::anyhow!("Proxy task stopped")))
        },
        res = stream_task => { 
            tracing::error!(target: "sys", "Stream task exited unexpectedly"); 
            res.map_err(|e| anyhow::anyhow!("Stream task panicked: {}", e))
               .and_then(|r| r.context("Stream client crashed"))
        },
        res = maintenance_task => { 
            tracing::error!(target: "sys", "Maintenance task exited unexpectedly"); 
            res.map_err(|e| anyhow::anyhow!("Maintenance task panicked: {}", e)).and_then(|_| Err(anyhow::anyhow!("Maintenance task stopped")))
        },
    }
}
