mod core;
mod auth;
mod cmd;
mod daemon;

use clap::Parser;
use crate::core::config::ConfigManager;
use crate::core::vault::{MultiVault, Vault};
use crate::core::security;
use crate::auth::{VaultTokenPool, AuthClient, pool::TokenPool};
use anyhow::Result;

#[derive(Parser)]
#[command(name = "cjtcr")]
#[command(version = "0.1.2")]
#[command(
    about = "畅捷通开放平台 Stream Connector 命令行工具 (CLI)",
    long_about = "畅捷通开放平台官方 CLI 治理工具。\n\n核心能力 (Core Capabilities):\n- 🔍 语义搜索 (api list --search): 基于 NLP 实现企业级 API 的智能检索与意向发现。\n- 🛡️ 自动鉴权 (init): 自动化托管 AppTicket 与 AccessToken 周期，无需手动刷新。\n- 📦 接口调用 (api [METHOD] [PATH]): 支持声明式 API 调用，自动注入安全头并实时审计。\n- 🛠️ 运维治理 (log): 全面的日志追踪、状态监控与运行记录管理。"
)]
pub struct Cli {
    #[arg(short, long, global = true, help = "配置环境名称 (缺省则使用当前激活的 Profile)")]
    pub profile: Option<String>,

    #[arg(short, long, default_value = "text", global = true, help = "输出格式 (text, json, yaml)")]
    pub format: String,

    #[arg(short, long, default_value = "info", global = true, help = "日志输出级别 (debug, info, warn, error)")]
    pub log_level: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// 初始化应用配置与安全凭据
    #[command(long_about = "初始化 CLI 的应用环境与安全凭据。这是使用 CLI 治理工具的第一步。\nCLI 会引导您输入 AppKey, AppSecret 等核心参数，并将其加密存储在本地安全存储 (Vault) 中。\n\n支持基于 Profile 的多环境隔离 (default/inte/prod)。")]
    Init {
        #[arg(long, help = "开放平台 AppKey")]
        app_key: Option<String>,
        #[arg(long, help = "开放平台 AppSecret (将被安全加密存储)")]
        app_secret: Option<String>,
        #[arg(long, help = "自建应用证书 (Certificate)")]
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
    #[command(long_about = "全面检查 CLI 的运行状态，包括配置、鉴权凭据与后台进程。\n默认列出所有已初始化的环境。指定 --profile 可查看特定环境的诊断详情。")]
    Status,
    /// 查看当前环境的配置详情
    Config,
    /// 重置当前环境的配置状态
    #[command(long_about = "清除当前 Profile 下的所有本地配置、缓存与安全凭据 (Vault)。\n重置后，您需要重新运行 'init' 命令才能再次使用此 Profile。此操作不可逆。")]
    Reset,
    /// 生成或自动安装命令行自动补全脚本 (Bash, Zsh, Fish)
    Completion {
        /// 指定 Shell 类型 (打印脚本时需要)
        #[arg(value_enum)]
        shell: Option<clap_complete::Shell>,

        /// 自动安装补全脚本到当前用户的配置中
        #[arg(long)]
        install: bool,
    },
    /// 管理当前生效的配置 Profile
    Profile {
        #[command(subcommand)]
        action: ProfileCommands,
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
}

#[derive(clap::Subcommand)]
pub enum ApiCommands {
    /// 列出并搜索开放平台可用的业务接口 (支持语义搜索)
    List {
        #[arg(short, long, help = "基于您的意图语义搜索 API 接口")]
        search: Option<String>,
        #[arg(short = 'n', long, default_value_t = 5, help = "返回语义搜索结果的 Top-N 数量")]
        top: usize,
        #[arg(long, default_value_t = 1, help = "分页页码")]
        page: usize,
        #[arg(long, default_value_t = 20, help = "每页数量")]
        page_size: usize,
        #[arg(short, long, default_value = "text", help = "输出格式 (text, json, yaml)")]
        format: String,
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
    #[command(long_about = "检查当前 Profile 下 AppKey, Certificate 与 AppSecret 的完整性。\n此命令仅关注凭据层，不检查 Daemon 状态。若需查看 CLI 整体健康度，请直接运行 'cjtc status'。")]
    Status,
    /// 重置当前配置环境的所有凭据与安全设置
    Reset,
    /// 触发 AppTicket 推送
    Login,
    /// 查看令牌
    Token,
}

#[derive(clap::Subcommand)]
pub enum DaemonCommands {
    /// 启动 cjtc 后台服务 (包括 Stream 桥接、反向代理与转发器)
    Start {
        #[arg(long, default_value_t = 8080)]
        proxy_port: u16,
        
        /// 在前台运行 (阻塞模式)
        #[arg(long)]
        foreground: bool,
    },
    /// 停止正在运行的 cjtc 守护进程
    Stop,
}


#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // 1. Core Paths
    let app_dir = crate::core::config::get_app_dir();
    let log_dir = app_dir.join("log");

    // 2. Load Config to get Log Settings
    let cfg_mgr = ConfigManager::new()?;
    let active_profile = cli.profile.clone().unwrap_or_else(|| cfg_mgr.get_default_profile());
    
    // Load config partially or use default if it fails
    let mut config = cfg_mgr.load(&active_profile).unwrap_or_else(|_| crate::core::config::Config::default_with_profile(&active_profile));

    // Override config log level if CLI provides one (default is "info", but we check if it was explicitly set)
    // Actually, clap default_value means it's always "info". 
    // We can check if it matches the default or if we want to always let CLI take precedence.
    // Usually CLI should take precedence if user provides it.
    config.log.level = cli.log_level.clone();

    // 3. Initialize Telemetry (Structured & Rotated Logging)
    let _guards = crate::core::telemetry::init_telemetry(log_dir, &config.log)?;
    tracing::info!(target: "sys", "cjtcr starting (version {})", env!("CARGO_PKG_VERSION"));
    tracing::info!(target: "sys", profile = %active_profile, "active profile loaded");

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
    let token_pool = VaultTokenPool::new(vault.clone());
    let auth_cli = AuthClient::new(&token_pool);

    // 3. Execute Command
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
        Commands::Api { action, method, path, data } => {
            if let Some(act) = action {
                match act {
                    ApiCommands::List { search, top, page, page_size, format } => {
                        cmd::api::list(&active_profile, &config, &auth_cli, search, *top, *page, *page_size, format).await?;
                    }
                    ApiCommands::Spec { method, path, raw } => {
                        cmd::api::spec(&active_profile, &config, &auth_cli, method, path, *raw).await?;
                    }
                }
            } else if let (Some(m), Some(p)) = (method, path) {
                cmd::api::call(&active_profile, &config, &auth_cli, m, p, data).await?;
            } else {
                println!("Usage: cjtc api [METHOD] [PATH] or use subcommands (list, spec)");
            }
        },
        Commands::Auth { action } => match action {
            AuthCommands::Status => {
                cmd::system::status(&active_profile, &cfg_mgr, vault.as_ref()).await?;
            }
            AuthCommands::Reset => {
                cmd::system::reset(&active_profile, &cfg_mgr, vault.as_ref()).await?;
            }
            AuthCommands::Login => {
                cmd::auth::login(&active_profile, &config, &auth_cli).await?;
            }
            AuthCommands::Token => {
                cmd::auth::token(&active_profile, &auth_cli).await?;
            }
        },
        Commands::Daemon { action } => match action {
            DaemonCommands::Start { proxy_port, foreground } => {
                cmd::daemon::start(&active_profile, &config, *proxy_port, *foreground).await?;
            }
            DaemonCommands::Stop => {
                cmd::daemon::stop(&active_profile).await?;
            }
        },
        Commands::Status => {
            cmd::system::status(&active_profile, &cfg_mgr, vault.as_ref()).await?;
        }
        Commands::Config => {
            cmd::system::config(&active_profile, &cfg_mgr).await?;
        }
        Commands::Reset => {
            cmd::system::reset(&active_profile, &cfg_mgr, vault.as_ref()).await?;
        }
        Commands::Completion { shell, install } => {
            if *install {
                let _ = crate::cmd::completion::install_completion();
            } else if let Some(s) = shell {
                use clap::CommandFactory;
                let mut cmd = Cli::command();
                clap_complete::generate(*s, &mut cmd, "cjtc", &mut std::io::stdout());
            } else {
                println!("Usage: cjtc completion [SHELL] or cjtc completion --install");
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
        }
    }

    Ok(())
}

// Helper to handle trait vs struct for Vault
