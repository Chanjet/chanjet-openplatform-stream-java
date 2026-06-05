use cli::{Cli, Commands};
use protocol::{AppState, JsonRpcNotification, JsonRpcRequest};
use server::handle_request;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use clap::Parser;

mod cli;
mod protocol;
mod client;
mod schema;
mod handlers;
mod server;
mod capabilities;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Config) => {
            let config_json = serde_json::json!({
                "mcpServers": {
                    "cowen-mcp": {
                        "command": "cowen",
                        "args": ["plugins", "run", "cowen-mcp-plugin", "server"]
                    }
                }
            });
            println!("{}", serde_json::to_string_pretty(&config_json)?);
        }
        Some(Commands::Server) => {
            let app_state = AppState::new(cli.profile);

            let mut reader = BufReader::new(tokio::io::stdin());
            let mut writer = tokio::io::stdout();
            let mut line = String::new();

            while reader.read_line(&mut line).await? > 0 {
                let req: Result<JsonRpcRequest, _> = serde_json::from_str(&line);
                if let Ok(req) = req {
                    let (resp, should_notify) = handle_request(req, &app_state).await;

                    if let Some(r) = resp {
                        let resp_str = serde_json::to_string(&r)?;
                        writer.write_all(resp_str.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                        writer.flush().await?;
                    }

                    if should_notify {
                        let notification = JsonRpcNotification {
                            jsonrpc: "2.0".to_string(),
                            method: "notifications/tools/list_changed".to_string(),
                        };
                        let notif_str = serde_json::to_string(&notification)?;
                        writer.write_all(notif_str.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                        writer.flush().await?;
                    }
                }
                line.clear();
            }
        }
        None => {
            use clap::CommandFactory;
            Cli::command().print_help()?;
            println!();
        }
    }

    Ok(())
}
