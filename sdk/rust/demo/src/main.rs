use connector_sdk::{GatewayClient, ClientOptions};
use dotenvy::dotenv;
use std::env;
use tokio::signal;
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Initialize Logging
    tracing_subscriber::fmt::init();

    // 2. Load .env
    if let Err(_) = dotenv() {
        tracing::warn!("No .env file found, using system environment variables");
    }

    let app_key = env::var("APP_KEY").expect("APP_KEY must be set");
    let app_secret = env::var("APP_SECRET").expect("APP_SECRET must be set");
    let encrypt_key = env::var("ENCRYPT_KEY").ok();
    let gateway_url = env::var("GATEWAY_URL").unwrap_or_default();

    let options = ClientOptions {
        app_key,
        app_secret,
        encrypt_key,
        gateway_url,
    };

    // 3. Create Client and Register Handlers
    let client = GatewayClient::new(options);
    
    {
        let d = client.dispatcher();
        let mut dispatcher = d.lock().unwrap();
        
        dispatcher.on_app_ticket(|msg| {
            println!("🎫 [Rust Demo] 收到应用票据: {}", msg.biz_content.app_ticket);
            true
        });

        dispatcher.on_ent_auth_code(|msg| {
            println!("🔑 [Rust Demo] 收到临时授权码: {}", msg.biz_content.temp_auth_code);
            true
        });

        dispatcher.on_order_status(|msg| {
            println!("💰 [Rust Demo] 收到订单支付成功消息: {}", msg.biz_content.order_no);
            true
        });
    }

    // 4. Start Client
    tracing::info!("🚀 [Rust Demo] 正在启动 Rust SDK Demo...");
    client.start().await?;

    // 5. Wait for Signal
    signal::ctrl_c().await?;
    tracing::info!("Stopping...");
    client.stop();

    Ok(())
}
