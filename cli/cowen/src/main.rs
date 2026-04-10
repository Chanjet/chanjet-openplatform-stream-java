mod core;
mod auth;
mod cmd;
mod daemon;

use clap::Parser;
use crate::core::config::ConfigManager;
use crate::core::vault::{MultiVault, Vault};
use crate::core::security;
use crate::core::utils::get_bin_name;
use anyhow::Result;

#[derive(Parser)]
#[command(name = env!("CARGO_BIN_NAME_OVERRIDE"))]
#[command(version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), " / ", env!("BUILD_ID"), " / ", env!("BUILD_TIME"), ")"))]
#[command(
    about = "畅捷通 (Chanjet) 开放平台官方 CLI：集安全托管、API 智能搜索与实时流式桥接于一体的生产力工具。",
    long_about = "畅捷通 (Chanjet) 开放平台官方全流程治理工具。\n\n本工具是连接企业本地业务系统与 畅捷通好业财、T+Cloud、好微、好会计 等云端核心产品的数字支点。它不仅是一个命令行界面，更是为 AI Agent 与自动化管道设计的 零信任安全网关 与 智能接口发现系统。\n\n核心能力 (Core Capabilities):\n- 🧠 意向发现 (api list --search): 内置极轻量 ONNX 神经网络推理引擎，支持通过自然语言实现 API 的语义搜索与精准锁定。\n- 🛡️ 安全编排 (init/auth): 自动化执行 AppTicket/AccessToken 握手解析，托管加密的安全凭据存储 (Vault)，自动注入签名安全头。\n- 🔄 实时流桥 (daemon): 基于 WebSocket 实现的高性能 Streaming Gateway 桥接器，支持在防火墙内安全接收云端消息推送并本地转发。\n- 📊 健壮运维 (dlq/log): 完整的死信队列 (DLQ) 处理机制与多域结构化审计日志，确保每一笔交易与推送均可回溯与自动补试。"
)]
pub struct Cli {
    #[arg(short, long, global = true, help = "配置环境名称 (缺省则使用当前激活的 Profile)")]
    pub profile: Option<String>,

    #[arg(short = 'o', long, default_value = "text", global = true, help = "输出格式 (text, json, yaml)")]
    pub format: String,

    #[arg(long, default_value = "error", global = true, help = "日志输出级别 (debug, info, warn, error)")]
    pub log_level: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// 初始化应用配置与安全凭据
    #[command(long_about = "初始化 CLI 的应用环境与安全凭据。这是治理工具的第一步。\nCLI 会引导您输入 AppKey, AppSecret 等核心参数，并将其加密存储在本地安全存储 (Vault) 中。\n\n支持基于 Profile 的多环境隔离 (default/inte/prod)。")]
    Init {
        #[arg(long, help = "开放平台 AppKey")]
        app_key: Option<String>,
        #[arg(long, help = "开放平台 AppSecret (将被安全加密存储)")]
        app_secret: Option<String>,
        #[arg(short = 'c', long, help = "自建应用证书 (Certificate)")]
        certificate: Option<String>,
        #[arg(long, help = "消息加解密密钥 (AES Encrypt Key)")]
        encrypt_key: Option<String>,
        #[arg(long, help = "本地 Webhook 接收地址")]
        webhook_target: Option<String>,
        #[arg(long, help = "OpenAPI 基础 URL 覆盖")]
        openapi_url: Option<String>,
        #[arg(long, help = "Stream Gateway 基础 URL 覆盖")]
        stream_url: Option<String>,
    },
    /// 调用开放平台 API 或管理接口规范
    #[command(long_about = "调用开放平台 API 或管理接口规范。\n\n此命令支持两种模式：\n1. 直接调用 API: 提供 [METHOD] (如 GET, POST) 和 [PATH] (如 /v1/user) 直接发起请求。\n   CLI 会自动处理鉴权 Token 注入、请求签名与审计记录。\n2. 子命令管理: 使用 'list' 或 'spec' 进行 API 的检索、语义搜索与文档查看。")]
    Api {
        #[arg(help = "HTTP Method (e.g. GET, POST)")]
        method: Option<String>,
        #[arg(help = "API Path (e.g. /v1/user)")]
        path: Option<String>,
        #[arg(short = 'd', long = "data", help = "HTTP 请求体数据 (JSON格式)")]
        data: Option<String>,
        #[arg(short = 'f', long = "file", help = "从文件读取请求体数据 (JSON格式)")]
        data_file: Option<String>,

        #[command(subcommand)]
        action: Option<ApiCommands>,
    },
    /// 管理身份认证与凭据 (Token/Ticket)
    Auth {
        #[command(subcommand)]
        action: AuthCommands,
    },
    /// 管理 CLI 后台守护进程 (桥接、代理与转发)
    Daemon {
        #[command(subcommand)]
        action: DaemonCommands,
    },
    /// 检查 CLI 的整体运行状态
    Status,
    /// 查看当前环境的配置详情
    Config,
    /// 重置当前环境的配置状态
    #[command(long_about = "清除当前 Profile 下的所有本地配置、缓存与安全凭据 (Vault)。\n重置后，您需要重新运行 'init' 命令才能再次使用此 Profile。此操作不可逆。")]
    Reset,
    /// 生成或自动安装命令行自动补全脚本 (Bash, Zsh, Fish)
    Completion {
        /// 指定 Shell 类型: bash, zsh, fish
        #[arg(value_enum)]
        shell: Option<clap_complete::Shell>,

        /// 自动安装补全脚本到当前用户的配置中
        #[arg(long)]
        install: bool,

        /// 从当前用户的配置中卸载补全脚本
        #[arg(long)]
        uninstall: bool,
    },
    /// 管理当前生效的配置 Profile
    Profile {
        #[command(subcommand)]
        action: ProfileCommands,
    },
    /// 管理死信队列 (DLQ) 中的异常事件
    Dlq {
        #[command(subcommand)]
        action: DlqCommands,
    },
    /// 查看并追踪 CLI 运行日志
    Log {
        #[command(subcommand)]
        action: LogCommands,
    },
}

#[derive(clap::Subcommand)]
pub enum ProfileCommands {
    /// 切换并设置为默认 Profile
    Use {
        #[arg(help = "要切换的 Profile 名称")]
        name: String,
    },
    /// 显示当前默认生效的 Profile
    Current,
    /// 列出所有可用的 Profile
    List,
}

#[derive(clap::Subcommand)]
pub enum ApiCommands {
    /// 列出并搜索开放平台可用的业务接口 (支持语义搜索)
    List {
        #[arg(short, long, help = "基于您的意图语义搜索 API 接口")]
        search: Option<String>,
        #[arg(long, default_value_t = 1, help = "分页页码")]
        page: usize,
        #[arg(short = 'n', long, default_value_t = 20, help = "每页数量")]
        page_size: usize,
        #[arg(short, long, help = "强制从云端同步最新的 OpenAPI 规约")]
        refresh: bool,
    },
    /// 获取指定 API 的 OpenAPI 3.0 规范或详细离线文档
    Spec {
        method: String,
        path: String,
        #[arg(long, help = "显示原始 OpenAPI JSON 规约片段")]
        raw: bool,
    },
}

#[derive(clap::Subcommand)]
pub enum AuthCommands {
    /// 检查当前环境的身份认证状态
    Status,
    /// 重置当前配置环境的所有凭据与安全设置
    Reset,
    /// 触发 AppTicket 推送与令牌刷新
    Login {
        /// 强制清除本地 AccessToken 缓存并重新触发网络刷新
        #[arg(short, long)]
        force: bool,
    },
    /// 查看令牌
    Token,
}

#[derive(clap::Subcommand)]
pub enum DaemonCommands {
    /// 启动后台服务 (包括 Stream 桥接、反向代理与转发器)
    Start {
        #[arg(long, default_value_t = 8080)]
        proxy_port: u16,

        /// 启用本地 OpenAPI 反向代理服务器
        #[arg(long)]
        enable_proxy: bool,
        
        /// 在前台运行 (阻塞模式)
        #[arg(long)]
        foreground: bool,
    },
    /// 停止正在运行的守护进程
    Stop,
    /// 重启守护进程
    Restart {
        #[arg(long, default_value_t = 8080)]
        proxy_port: u16,

        /// 启用本地 OpenAPI 反向代理服务器
        #[arg(long)]
        enable_proxy: bool,

        /// 重启所有正在运行的守护进程
        #[arg(short, long)]
        all: bool,
    },
}

#[derive(clap::Subcommand)]
pub enum DlqCommands {
    /// 列举死信队列中的异常事件
    List,
    /// 重试特定的异常事件
    Retry {
        #[arg(help = "DLQ 记录 ID")]
        id: String,
    },
    /// 清空所有 DLQ 记录
    Purge,
}

#[derive(clap::Subcommand)]
pub enum LogCommands {
    /// 列出所有可用的日志 domain
    List,
    /// 在终端实时查看并追踪指定 domain 的日志
    View {
        #[arg(help = "日志域: sys (系统), audit (审计), stream (流转), dlq (死信)", default_value = "sys")]
        domain: String,
        /// 是否开启实时追踪模式 (等同于 tail -f)
        #[arg(short, long)]
        follow: bool,
        /// 显示最后 N 行
        #[arg(short = 'n', long, default_value_t = 10)]
        lines: usize,
    },
}


#[cfg(test)]
mod security_tests;

#[tokio::main]
async fn main() {
    // CAPTURE PANICS: Ensure background crashes are recorded
    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload().downcast_ref::<&str>().cloned()
            .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("no message");
        tracing::error!(target: "sys", "FATAL PANIC: {}", payload);
    }));

    if let Err(e) = run().await {
        tracing::error!(target: "sys", error = %e, "CLI execution failed");
        eprintln!("❌ Error: {}", e);
        std::process::exit(1);
    }
    
    // Give a tiny grace period for background telemetry tasks to finish
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let bin_name = get_bin_name();

    // 1. Core Paths
    let app_dir = crate::core::config::get_app_dir();
    let log_dir = app_dir.join("logs");

    // 2. Load Config to get Log Settings
    let cfg_mgr = ConfigManager::new()?;
    let active_profile = cli.profile.clone().unwrap_or_else(|| cfg_mgr.get_default_profile());
    
    // Load config partially or use default if it fails
    let mut config = cfg_mgr.load(&active_profile).unwrap_or_else(|_| crate::core::config::Config::default_with_profile(&active_profile));

    // Override config log level if CLI provides one
    config.log.level = cli.log_level.clone();

    // 3. Initialize Telemetry (Structured & Rotated Logging)
    let _guards = crate::core::telemetry::init_telemetry(log_dir, &config.log)?;
    tracing::info!(target: "sys", "{} starting (version {})", bin_name, env!("CARGO_PKG_VERSION"));
    tracing::info!(target: "sys", profile = %active_profile, "active profile loaded");

    // 4. Check for Activation (First Run)
    let marker_path = app_dir.join(".telemetry_marker");
    if !marker_path.exists() {
        // Ensure app_dir exists before creating marker
        if !app_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&app_dir) {
                tracing::error!(target: "sys", "Failed to create app directory {:?}: {}", app_dir, e);
            }
        }
        crate::core::telemetry::report_event(&config, "cli_first_run".to_string(), serde_json::json!({}));
        if let Err(e) = std::fs::File::create(&marker_path) {
            tracing::error!(target: "sys", "Failed to create telemetry marker {:?}: {}", marker_path, e);
        } else {
            tracing::debug!(target: "sys", "Created telemetry marker at {:?}", marker_path);
        }
    }

    // 5. Report Command Run
    let cmd_name = match &cli.command {
        Commands::Api { .. } => "api",
        Commands::Auth { .. } => "auth",
        Commands::Daemon { .. } => "daemon",
        Commands::Init { .. } => "init",
        Commands::Status => "status",
        Commands::Config => "config",
        Commands::Reset => "reset",
        Commands::Completion { .. } => "completion",
        Commands::Profile { .. } => "profile",
        Commands::Dlq { .. } => "dlq",
        Commands::Log { .. } => "log",
    };
    crate::core::telemetry::report_event(&config, "command_run".to_string(), serde_json::json!({ "cmd": cmd_name }));

    let fingerprint = security::get_machine_fingerprint()?;
    let seal_path = app_dir.join(".seal");
    
    let vault: std::sync::Arc<dyn Vault> = std::sync::Arc::new(MultiVault::new(seal_path, &fingerprint)?);
    
    // Inject secrets from vault into config
    if let Ok(secret) = vault.get(&active_profile, "app_secret") {
        config.app_secret = secret;
    }
    if let Ok(cert) = vault.get(&active_profile, "certificate") {
        config.certificate = cert;
    }
    if let Ok(encrypt_key) = vault.get(&active_profile, "encrypt_key") {
        config.encrypt_key = encrypt_key;
    }

    // 2. Initialize Auth
    let token_pool = crate::auth::VaultTokenPool::new(vault.clone());
    let auth_cli = crate::auth::AuthClient::new(&token_pool);

    // 4. Automatic Shell Completion Installation (One-time check)
    if crate::cmd::completion::is_auto_install_needed() {
        let _ = crate::cmd::completion::install_completion(None);
    }

    // 5. Ensure daemon is running and up to date with this CLI binary
    if !matches!(&cli.command, Commands::Daemon { .. } | Commands::Reset | Commands::Init { .. }) {
        let _ = crate::cmd::system::ensure_daemon_running(&active_profile, &config, &cfg_mgr).await;
    }

    // 6. Execute Command
    match &cli.command {
        Commands::Init { 
            app_key, 
            app_secret, 
            certificate,
            encrypt_key,
            webhook_target,
            openapi_url,
            stream_url,
        } => {
            cmd::init::execute(
                &active_profile, 
                &cfg_mgr, 
                vault.as_ref(), 
                app_key, 
                app_secret, 
                certificate,
                encrypt_key,
                webhook_target,
                openapi_url,
                stream_url,
            ).await?;
        }
        Commands::Api { method, path, data, data_file, action } => {
            if let Some(act) = action {
                match act {
                    ApiCommands::List { search, page, page_size, refresh } => {
                        cmd::api::list(&active_profile, &config, &auth_cli, search, *page, *page_size, &cli.format, *refresh).await?;
                    }
                    ApiCommands::Spec { method, path, raw } => {
                        cmd::api::spec(&active_profile, &config, &auth_cli, method, path, *raw).await?;
                    }
                }
            } else if let (Some(m), Some(p)) = (method, path) {
                cmd::api::call(&active_profile, &config, &auth_cli, m, p, data, data_file, &cli.format).await?;
            } else {
                println!("Usage: {} api [METHOD] [PATH] or use subcommands (list, spec)", bin_name);
            }
        },

        Commands::Auth { action } => match action {
            AuthCommands::Status => {
                cmd::system::status(&active_profile, &cfg_mgr, vault.as_ref(), &cli.format).await?;
            }
            AuthCommands::Reset => {
                cmd::system::reset(&active_profile, Some(vault.as_ref())).await?;
            }
            AuthCommands::Login { force } => {
                cmd::auth::login(&active_profile, &config, &auth_cli, *force).await?;
            }
            AuthCommands::Token => {
                cmd::auth::token(&active_profile, &config, &auth_cli, &cli.format).await?;
            }
        },
        Commands::Daemon { action } => match action {
            DaemonCommands::Start { proxy_port, enable_proxy, foreground } => {
                cmd::daemon::start(&active_profile, &config, *proxy_port, *enable_proxy, *foreground).await?;
            }
            DaemonCommands::Stop => {
                cmd::daemon::stop(&active_profile).await?;
            }
            DaemonCommands::Restart { proxy_port, enable_proxy, all } => {
                cmd::daemon::restart(&active_profile, &config, *proxy_port, *enable_proxy, *all, &cfg_mgr).await?;
            }
        },
        Commands::Status => {
            cmd::system::status(&active_profile, &cfg_mgr, vault.as_ref(), &cli.format).await?;
        }
        Commands::Config => {
            cmd::system::config(&active_profile, &cfg_mgr, &cli.format).await?;
        }
        Commands::Reset => {
            cmd::system::reset(&active_profile, Some(vault.as_ref())).await?;
        }
        Commands::Completion { shell, install, uninstall } => {
            if *uninstall {
                match crate::cmd::completion::uninstall_completion() {
                    Ok(_) => println!("✅ Auto-completion successfully uninstalled. Please restart your terminal."),
                    Err(e) => eprintln!("❌ Failed to uninstall auto-completion: {}", e),
                }
            } else if *install {
                match crate::cmd::completion::install_completion(*shell) {
                    Ok(_) => println!("✅ Auto-completion successfully installed."),
                    Err(e) => eprintln!("❌ Failed to install auto-completion: {}", e),
                }
            } else if let Some(s) = shell {
                use clap::CommandFactory;
                let mut cmd = Cli::command();
                clap_complete::generate(*s, &mut cmd, &bin_name, &mut std::io::stdout());
            } else {
                println!("Usage: {} completion [SHELL] or {} completion --install", bin_name, bin_name);
            }
        }
        Commands::Profile { action } => match action {
            ProfileCommands::Use { name } => {
                cfg_mgr.set_default_profile(name)?;
                println!("✅ Set default profile to '{}'", name);
            }
            ProfileCommands::Current => {
                println!("{}", cfg_mgr.get_default_profile());
            }
            ProfileCommands::List => {
                let profiles = cfg_mgr.list_profiles()?;
                let current = cfg_mgr.get_default_profile();
                
                if cli.format == "json" || cli.format == "yaml" {
                    crate::core::utils::render(&profiles, &cli.format)?;
                } else {
                    println!("\n📂 Available Profiles:");
                    for p in profiles {
                        if p == current {
                            println!("  * \x1b[32m{:<20}\x1b[0m (current)", p);
                        } else {
                            println!("    {:<20}", p);
                        }
                    }
                    println!();
                }
            }
        },
        Commands::Dlq { action } => match action {
            DlqCommands::List => {
                cmd::dlq::list(&active_profile, &cli.format).await?;
            }
            DlqCommands::Retry { id } => {
                cmd::dlq::retry(&active_profile, &config, id).await?;
            }
            DlqCommands::Purge => {
                cmd::dlq::purge(&active_profile).await?;
            }
        },
        Commands::Log { action } => match action {
            LogCommands::List => {
                cmd::log::list(&active_profile).await?;
            }
            LogCommands::View { domain, follow, lines } => {
                cmd::log::view(&active_profile, domain, *follow, *lines).await?;
            }
        }
    }

    Ok(())
}
