pub(crate) mod core;
pub(crate) mod cmd;

use clap::Parser;
use cowen_config::ConfigManager;
use cowen_common::security;
use cowen_common::utils::get_bin_name;
use anyhow::Result;
use std::io::Write;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = option_env!("CARGO_BIN_NAME_OVERRIDE").unwrap_or("cowen"))]
#[command(version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), " / ", env!("BUILD_TIME"), ")"))]
#[command(
    about = "畅捷通 (Chanjet) 开放平台官方 CLI：集安全托管、API 智能搜索与实时流式桥接于一体的生产力工具。",
    long_about = "畅捷通 (Chanjet) 开放平台官方全流程治理工具。\n\n本工具是连接企业本地业务系统与 畅捷通好业财、T+Cloud、好微、好会计 等云端核心产品的数字支点。它不仅是一个命令行界面，更是为 AI Agent 与自动化管道设计的 零信任安全网关 与 智能接口发现系统。\n\n核心能力 (Core Capabilities):\n- 🧠 意向发现 (api list --search): 内置极轻量 ONNX 神经网络推理引擎，支持通过自然语言实现 API 的语义搜索与精准锁定。\n- 🛡️ 安全编排 (init/auth): 自动化执行 AppTicket/AccessToken 握手解析，托管加密的安全凭据存储 (Vault)，自动注入签名安全头。\n- 🔄 实时流桥 (daemon): 基于 WebSocket 实现的高性能 Streaming Gateway 桥接器，支持在防火墙内安全接收云端消息推送并本地转发。\n- 📊 健壮运维 (dlq/log): 完整的死信队列 (DLQ) 处理机制与多域结构化审计日志，确保每一笔交易与推送均可回溯与自动补试。"
)]
pub struct Cli {
    #[arg(short, long, global = true, env = "COWEN_PROFILE", help = "配置环境名称 (缺省则使用当前激活的 Profile)")]
    pub profile: Option<String>,

    #[arg(short = 'o', long, default_value = "text", global = true, env = "COWEN_FORMAT", help = "输出格式 (text, json, yaml)")]
    pub format: String,

    #[arg(long, default_value = "error", global = true, env = "COWEN_LOG_LEVEL", help = "日志输出级别 (debug, info, warn, error)")]
    pub log_level: String,

    #[arg(long, global = true, help = "禁用遥测数据上报")]
    pub no_telemetry: bool,

    #[arg(long, global = true, help = "禁用 AI/语义搜索功能")]
    pub no_ai: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// 初始化应用配置与安全凭据
    #[command(long_about = "初始化 CLI 的应用环境与安全凭据。这是治理工具的第一步。\nCLI 会引导您输入 AppKey, AppSecret 等核心参数，并将其加密存储在本地安全存储 (Vault) 中。\n\n支持基于 Profile 的多环境隔离 (default/inte/prod)。")]
    Init {
        #[arg(long, env = "COWEN_APP_KEY", help = "开放平台 AppKey")]
        app_key: Option<String>,
        #[arg(long, env = "COWEN_APP_SECRET", help = "开放平台 AppSecret (将被安全加密存储)")]
        app_secret: Option<String>,
        #[arg(short = 'c', long, env = "COWEN_CERTIFICATE", help = "自建应用证书 (Certificate)")]
        certificate: Option<String>,
        #[arg(long, env = "COWEN_ENCRYPT_KEY", help = "消息加解密密钥 (AES Encrypt Key)")]
        encrypt_key: Option<String>,
        #[arg(long, env = "COWEN_WEBHOOK_TARGET", help = "本地 Webhook 接收地址")]
        webhook_target: Option<String>,
        #[arg(long, env = "COWEN_OPENAPI_URL", help = "OpenAPI 基础 URL 覆盖")]
        openapi_url: Option<String>,
        #[arg(long, env = "COWEN_STREAM_URL", help = "Stream Gateway 基础 URL 覆盖")]
        stream_url: Option<String>,
        #[arg(long, env = "COWEN_APP_MODE", help = "应用模式: self_built (自建应用), oauth2 (OAuth2应用)")]
        app_mode: Option<String>,
        #[arg(long, env = "COWEN_PROXY_PORT", help = "本地代理监听端口")]
        proxy_port: Option<u16>,
    },
    /// 调用开放平台 API 或管理接口规范
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
    Status {
        /// 扫描并输出所有存在的 Profile 的状态
        #[arg(short, long)]
        all: bool,
    },
    /// 管理当前环境的配置
    Config {
        #[command(subcommand)]
        action: Option<ConfigCommands>,
    },
    /// 重置当前环境的配置状态
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
    /// 管理并配置全局存储后端与缓存 (此命令为全局操作，不受 -p 参数影响)
    Store {
        #[command(subcommand)]
        action: StoreCommands,
    },
    /// 管理并检查系统整体状态
    System {
        #[command(subcommand)]
        action: SystemCommands,
    },
}

#[derive(clap::Subcommand)]
pub enum ConfigCommands {
    /// 设置配置项的值 (e.g., cowen config set log.level debug)
    Set {
        #[arg(help = "配置项名称 (目前支持: log.level)")]
        key: String,
        #[arg(help = "配置项的新值")]
        value: String,
    },
}

#[derive(clap::Subcommand)]
pub enum ProfileCommands {
    /// Switch to a different profile
    Use {
        /// Profile name to activate
        name: String,
    },
    /// Show the currently active profile
    Current,
    /// List all configured profiles
    List,
    /// Rename an existing profile
    Rename {
        /// Current profile name
        old_name: String,
        /// New profile name
        new_name: String,
    },
}

#[derive(clap::Subcommand)]
pub enum SystemCommands {
    /// Check system overall status (Daemon, Store, Auth, AI)
    Status {
        /// Show detailed status for all profiles
        #[arg(short, long)]
        all: bool,
    },
}

#[derive(clap::Subcommand)]
pub enum StoreCommands {
    Set {
        /// Store type (e.g. sqlite, mysql, postgres, redis, local, innerdb)
        #[arg(
            long, 
            env = "COWEN_STORE_TYPE",
            long_help = "Primary storage engine type.\n\nValues:\n  - sqlite / innerdb: Local SQLite database\n  - mysql / postgres: Distributed SQL database\n  - redis: High-performance key-value store\n  - local: Legacy flat-file storage"
        )]
        store: Option<String>,
        /// Database connection URL
        #[arg(
            long, 
            env = "COWEN_DB_URL",
            long_help = "Connection URL for the selected store.\n\nExamples:\n  - sqlite:data/cowen.db\n  - postgres://user:pass@localhost:5432/cowen\n  - mysql://user:pass@localhost:3306/cowen\n  - redis://localhost:6379"
        )]
        db_url: Option<String>,
        /// Cache store type (e.g. redis, memory)
        #[arg(long, env = "COWEN_CACHE_TYPE")]
        cache: Option<String>,
        /// Cache connection URL (e.g. redis://localhost:6379)
        #[arg(long, env = "COWEN_CACHE_URL")]
        cache_url: Option<String>,
    },
    Status,
    Migrate {
        /// Target Store URL to migrate to.
        #[arg(
            long, 
            value_name = "URL",
            long_help = "Target Store URL to migrate to.\n\nSupported formats:\n  - sqlite:path/to/db.sqlite (e.g. sqlite:data/cowen.db)\n  - mysql://user:pass@host:port/db\n  - postgres://user:pass@host:port/db\n  - redis://host:port\n  - local (Legacy file-based store)\n  - innerdb (Default managed SQLite)"
        )]
        to: String,
        /// Migration mode
        #[arg(long, value_enum, default_value = "clone")]
        mode: cowen_store::migration::MigrationMode,
    },
}

#[derive(clap::Subcommand)]
pub enum ApiCommands {
    /// List available APIs from the current specification
    List {
        /// Optional search term to filter APIs
        #[arg(short, long)]
        search: Option<String>,
        /// Page number for results
        #[arg(long, default_value_t = 1)]
        page: usize,
        /// Number of results per page
        #[arg(short = 'n', long, default_value_t = 20)]
        page_size: usize,
        /// Force refresh the local specification from the platform
        #[arg(short, long)]
        refresh: bool,
    },
    /// Show detailed specification for a specific API
    Spec {
        /// HTTP method (GET, POST, etc.)
        method: String,
        /// API path
        path: String,
        /// Show raw JSON specification
        #[arg(long)]
        raw: bool,
    },
}

#[derive(clap::Subcommand)]
pub enum AuthCommands {
    /// Check current authentication status
    Status,
    /// Reset authentication state
    Reset,
    /// Clear local session and logout
    Logout,
    /// Perform interactive or forced login
    Login {
        /// Force re-authentication even if token is valid
        #[arg(short, long)]
        force: bool,
        /// Internal use only for finalizing async login flows
        #[arg(long, hide = true)]
        finalize: Option<String>,
    },
    /// Retrieve or refresh the current access token
    Token {
        /// Proactively refresh the token from the platform
        #[arg(short, long)]
        refresh: bool,
    },
}

#[derive(clap::Subcommand)]
pub enum DaemonCommands {
    /// Start the background daemon service
    Start {
        /// Override the proxy listening port
        #[arg(long)]
        proxy_port: Option<u16>,
        /// Override the monitor listening port
        #[arg(long)]
        monitor_port: Option<u16>,
        /// Force enable API proxying
        #[arg(long)]
        enable_proxy: bool,
        /// Force disable API proxying
        #[arg(long)]
        no_proxy: bool,
        /// Run in foreground instead of background
        #[arg(long)]
        foreground: bool,
        /// Start daemons for all configured profiles
        #[arg(short, long)]
        all: bool,
    },
    /// Stop the background daemon service
    Stop {
        /// Stop daemons for all running profiles
        #[arg(short, long)]
        all: bool,
    },
    /// Restart the background daemon service
    Restart {
        /// Override the proxy listening port
        #[arg(long)]
        proxy_port: Option<u16>,
        /// Force enable API proxying
        #[arg(long)]
        enable_proxy: bool,
        /// Force disable API proxying
        #[arg(long)]
        no_proxy: bool,
        /// Restart daemons for all configured profiles
        #[arg(short, long)]
        all: bool,
    },
    /// Manage OS-level system services (e.g. systemd/launchd)
    Service { #[command(subcommand)] action: ServiceCommands },
}

#[derive(clap::Subcommand)]
pub enum ServiceCommands {
    /// Install the daemon as an OS system service (systemd/launchd)
    Install,
    /// Uninstall the OS system service
    Uninstall,
    /// Check the OS system service status
    Status,
}

#[derive(clap::Subcommand)]
pub enum DlqCommands {
    /// List events currently in the Dead Letter Queue
    List,
    /// Retry processing a specific event by ID
    Retry {
        /// Event ID to retry
        id: String,
    },
    /// Clear all events from the Dead Letter Queue
    Purge,
}

#[derive(clap::Subcommand)]
pub enum LogCommands {
    /// List available log files or domains
    List,
    /// View or follow log content
    View {
        /// Log domain to view (e.g. sys, audit, stream, dlq). Use 'cowen log list' to see all available files.
        #[arg(default_value = "sys")]
        domain: String,
        /// Follow log output in real-time
        #[arg(short, long)]
        follow: bool,
        /// Number of lines to show from the end
        #[arg(short = 'n', long, default_value_t = 10)]
        lines: usize,
    },
}

pub async fn run(cli: Cli) -> Result<()> {
    let _bin_name = get_bin_name();

    // 1. Core Paths
    let app_dir = cowen_common::config::get_app_dir();
    let log_dir = app_dir.join("logs");

    // 2. Load Config to get Log Settings
    let cfg_mgr = ConfigManager::new().map_err(|e| anyhow::anyhow!(e))?;
    let mut app_config = cfg_mgr.load_app_config().await.map_err(|e| anyhow::anyhow!(e))?;

    let fingerprint = security::get_machine_fingerprint().map_err(|e| anyhow::anyhow!(e))?;
    let vault = cowen_store::create_vault(&app_config, &app_dir, &fingerprint).await.map_err(|e| anyhow::anyhow!(e))?;
    let _ = cfg_mgr.set_vault(vault.clone());

    let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
    let _ = cfg_mgr.set_validator(std::sync::Arc::new(cowen_auth::AuthProviderValidator::new(auth_cli.clone())));

    let mut active_profile = cli.profile.clone().unwrap_or_else(|| cfg_mgr.get_default_profile());

    if matches!(&cli.command, Commands::Init { .. }) {
        if cli.profile.is_none() {
            active_profile = cfg_mgr.get_next_profile_name().await.map_err(|e| anyhow::anyhow!(e))?;
            println!("🪄 No profile name provided. Automatically generating new profile: \x1b[1;32m{}\x1b[0m", active_profile);
        }
    }
    
    let mut config = match cfg_mgr.load(&active_profile).await {
        Ok(cfg) => cfg,
        Err(e) if e.to_string().contains("SKIPPED:") => return Err(anyhow::anyhow!(e)),
        Err(e) => {
            let is_lifecycle_cmd = matches!(&cli.command, Commands::Reset | Commands::Init { .. } | Commands::Profile { .. });
            if cfg_mgr.exists(&active_profile).await && !is_lifecycle_cmd {
                return Err(anyhow::anyhow!("Failed to load existing profile '{}': {}. Try 'cowen reset -p {}' if the config is corrupted.", active_profile, e, active_profile));
            }
            cowen_common::Config::default_with_profile(&active_profile)
        }
    };

    config.apply_env_overrides();

    if cli.no_telemetry { config.telemetry_enabled = false; }
    if cli.no_ai { config.ai_enabled = false; }

    let (vault_tx, vault_rx) = tokio::sync::watch::channel(None);
    let telemetry_control = match core::telemetry::init_telemetry(log_dir, &active_profile, &config.log, vault_rx) {
        Ok(control) => Some(Arc::new(control)),
        Err(e) => {
            eprintln!("⚠️ Warning: Telemetry system failed to initialize: {}. Continuing without structured logging.", e);
            None
        }
    };
    let _guards = telemetry_control.as_ref().map(|c| &c.guards);

    tracing::info!(target: "sys", "cowen starting (version {})", env!("CARGO_PKG_VERSION"));

    let cmd_name = match &cli.command {
        Commands::Api { .. } => "api",
        Commands::Auth { .. } => "auth",
        Commands::Daemon { .. } => "daemon",
        Commands::Init { .. } => "init",
        Commands::Status { .. } => "status",
        Commands::Config { .. } => "config",
        Commands::Reset => "reset",
        Commands::Completion { .. } => "completion",
        Commands::Profile { .. } => "profile",
        Commands::Dlq { .. } => "dlq",
        Commands::Log { .. } => "log",
        Commands::Store { .. } => "store",
        Commands::System { .. } => "system",
    };
    core::telemetry::report_event(&config, "command_run".to_string(), serde_json::json!({ "cmd": cmd_name }));

    if let Commands::Store { action } = &cli.command {
        match action {
            StoreCommands::Set { store, db_url, cache, cache_url } => {
                cmd::store::set(&mut app_config, &cfg_mgr, store, db_url, cache, cache_url).await?;
                return Ok(());
            }
            StoreCommands::Status => {
                cmd::store::status(&app_config).await?;
                return Ok(());
            }
            _ => {} 
        }
    }

    let _ = vault_tx.send(Some(vault.clone()));

    if let Commands::Store { action: StoreCommands::Migrate { to, mode } } = &cli.command {
        cmd::store::migrate(&cfg_mgr, to, *mode).await?;
        return Ok(());
    }

    if std::env::var("COWEN_SKIP_COMPLETION_INSTALL").is_err() && cmd::completion::is_auto_install_needed() {
        let _ = cmd::completion::install_completion(None);
    }

    let daemon_svc = Arc::new(cowen_server::ServerDaemonService::new(cfg_mgr.clone()));

    // --- Daemon Lifecycle Enforcement ---
    // 1. Version Sync: Ensure all CURRENTLY RUNNING daemons match the CLI version.
    // We only skip this during explicit stop, reset, or init operations.
    let skip_version_sync = match cmd_name {
        "reset" | "init" => true,
        "daemon" => matches!(&cli.command, Commands::Daemon { action: DaemonCommands::Stop { .. } }),
        _ => false,
    } || std::env::var("COWEN_SKIP_DAEMON_RECOVERY").is_ok();
    
    if !skip_version_sync {
        let _ = cmd::system::enforce_daemon_version_sync(&active_profile, &cfg_mgr, vault.clone()).await;
    }

    // 2. Auto-recovery: "确保必要的后台进程正在运行"
    // We skip auto-recovery for lifecycle/management commands to avoid starting a daemon 
    // that the user is explicitly trying to stop, reset or initialize.
    // However, diagnostic commands like 'status' SHOULD trigger recovery to maintain the "always-on" promise.
    let skip_recovery = match cmd_name {
        "daemon" | "reset" | "init" | "config" | "profile" | "dlq" | "log" => true,
        "auth" => matches!(&cli.command, Commands::Auth { action: AuthCommands::Logout | AuthCommands::Reset }),
        _ => false,
    } || std::env::var("COWEN_SKIP_DAEMON_RECOVERY").is_ok();
    
    if !skip_recovery {
        let _ = cmd::system::ensure_daemon_running(&active_profile, &config, &cfg_mgr, vault.clone(), &auth_cli).await;
    }

    match &cli.command {
        Commands::Init { app_key, app_secret, certificate, encrypt_key, webhook_target, openapi_url, stream_url, app_mode, proxy_port } => {
            let ctx = cmd::init::InitContext {
                app_key: app_key.clone(),
                app_secret: app_secret.clone(),
                certificate: certificate.clone(),
                encrypt_key: encrypt_key.clone(),
                webhook_target: webhook_target.clone(),
                openapi_url: openapi_url.clone(),
                stream_url: stream_url.clone(),
                app_mode: app_mode.clone(),
                proxy_port: proxy_port.clone(),
                auto_start: true,
            };
            cmd::init::execute(&active_profile, &cfg_mgr, &mut app_config, vault.clone(), ctx, Some(daemon_svc.clone())).await?;
        }
        Commands::Api { method, path, data, data_file, action } => {
            if let Some(act) = action {
                match act {
                    ApiCommands::List { search, page, page_size, refresh } => {
                        cmd::api::list(&active_profile, &config, &auth_cli, search, *page, *page_size, &cli.format, *refresh, vault.clone()).await?;
                    }
                    ApiCommands::Spec { method, path, raw } => {
                        cmd::api::spec(&active_profile, &config, &auth_cli, method, path, *raw).await?;
                    }
                }
            } else if let (Some(m), Some(p)) = (method, path) {
                cmd::api::call(&active_profile, &config, &auth_cli, m, p, data, data_file, &cli.format).await?;
            } else {
                println!("Usage: cowen api [METHOD] [PATH] or use subcommands (list, spec)");
            }
        }
        Commands::Auth { action } => match action {
            AuthCommands::Status => cmd::system::status(&active_profile, &cfg_mgr, vault.clone(), &cli.format, false).await?,
            AuthCommands::Reset | AuthCommands::Logout => cmd::auth::logout(&active_profile, &config, &auth_cli).await?,
            AuthCommands::Login { force, finalize } => cmd::auth::login(&active_profile, &config, &auth_cli, *force, finalize.as_deref(), Some(daemon_svc.clone())).await?,
            AuthCommands::Token { refresh } => cmd::auth::token(&active_profile, &config, &auth_cli, &cli.format, *refresh).await?,
        }
        Commands::Daemon { action } => match action {
            DaemonCommands::Start { proxy_port, monitor_port, enable_proxy, no_proxy, foreground, all } => {
                let mut updated_config = config.clone();
                let mut changed = false;
                if let Some(p) = proxy_port { if updated_config.proxy_port != *p { updated_config.proxy_port = *p; changed = true; } }
                if let Some(m) = monitor_port { if updated_config.monitor_port != *m { updated_config.monitor_port = *m; changed = true; } }
                if *enable_proxy { if !updated_config.proxy_enabled { updated_config.proxy_enabled = true; changed = true; } }
                else if *no_proxy { if updated_config.proxy_enabled { updated_config.proxy_enabled = false; changed = true; } }
                if changed && !*all { cfg_mgr.save(&active_profile, &mut updated_config).await.map_err(|e| anyhow::anyhow!(e))?; }
                
                if *foreground {
                    cmd::daemon::start(&active_profile, &updated_config, updated_config.proxy_port, updated_config.proxy_enabled, true, *all, &cfg_mgr, vault.clone(), telemetry_control.clone()).await?;
                } else {
                    cmd::daemon::restart(&active_profile, &updated_config, updated_config.proxy_port, updated_config.proxy_enabled, *all, &cfg_mgr, vault.clone(), telemetry_control.clone()).await?;
                }
            }
            DaemonCommands::Stop { all } => cmd::daemon::stop(&active_profile, *all, &cfg_mgr).await?,
            DaemonCommands::Restart { proxy_port, enable_proxy, no_proxy, all } => {
                let mut updated_config = config.clone();
                let mut changed = false;
                if let Some(p) = proxy_port { if updated_config.proxy_port != *p { updated_config.proxy_port = *p; changed = true; } }
                if *enable_proxy { if !updated_config.proxy_enabled { updated_config.proxy_enabled = true; changed = true; } }
                else if *no_proxy { if updated_config.proxy_enabled { updated_config.proxy_enabled = false; changed = true; } }
                if changed && !*all { cfg_mgr.save(&active_profile, &mut updated_config).await.map_err(|e| anyhow::anyhow!(e))?; }
                cmd::daemon::restart(&active_profile, &updated_config, updated_config.proxy_port, updated_config.proxy_enabled, *all, &cfg_mgr, vault.clone(), telemetry_control.clone()).await?;
            }
            DaemonCommands::Service { action } => match action {
                ServiceCommands::Install => cmd::daemon::service::execute(cmd::daemon::service::ServiceAction::Install).await?,
                ServiceCommands::Uninstall => cmd::daemon::service::execute(cmd::daemon::service::ServiceAction::Uninstall).await?,
                ServiceCommands::Status => cmd::daemon::service::execute(cmd::daemon::service::ServiceAction::Status).await?,
            }
        }
        Commands::Status { all } => cmd::system::status(&active_profile, &cfg_mgr, vault.clone(), &cli.format, *all).await?,
        Commands::System { action } => match action { SystemCommands::Status { all } => cmd::system::status(&active_profile, &cfg_mgr, vault.clone(), &cli.format, *all).await? }
        Commands::Config { action } => match action {
            Some(ConfigCommands::Set { key, value }) => {
                let mut updated_config = config.clone();
                if key == "log.level" {
                    updated_config.log.level = value.to_lowercase();
                    cfg_mgr.save(&active_profile, &mut updated_config).await.map_err(|e| anyhow::anyhow!(e))?;
                    println!("✅ Successfully updated log.level to '{}'", value);
                } else if key == "monitor.port" {
                    if let Ok(p) = value.parse::<u16>() {
                        updated_config.monitor_port = p;
                        cfg_mgr.save(&active_profile, &mut updated_config).await.map_err(|e| anyhow::anyhow!(e))?;
                        println!("✅ Successfully updated monitor.port to '{}'", value);
                    } else {
                        return Err(anyhow::anyhow!("Invalid port number: {}", value));
                    }
                } else {
                    return Err(anyhow::anyhow!("Unsupported configuration key: {}. Supported: 'log.level', 'monitor.port'.", key));
                }
            }
            None => cmd::system::config(&active_profile, &cfg_mgr, &cli.format).await?,
        },
        Commands::Reset => cmd::system::reset(&active_profile, Some(vault.as_ref()), &cfg_mgr, Some(cowen_common::events::event_bus())).await?,
        Commands::Completion { shell, install, uninstall } => {
            if *uninstall { cmd::completion::uninstall_completion()?; }
            else if *install { cmd::completion::install_completion(*shell)?; }
            else if let Some(s) = shell {
                let mut buf = Vec::new();
                cmd::completion::generate_completion(*s, &mut buf)?;
                let _ = std::io::stdout().write_all(&buf);
            }
        }
        Commands::Profile { action } => match action {
            ProfileCommands::Use { name } => { cfg_mgr.set_default_profile(name).map_err(|e| anyhow::anyhow!(e))?; println!("✅ Set default profile to '{}'", name); }
            ProfileCommands::Current => println!("{}", cfg_mgr.get_default_profile()),
            ProfileCommands::List => {
                let profiles = cfg_mgr.list_profiles().await.map_err(|e| anyhow::anyhow!(e))?;
                let current = cfg_mgr.get_default_profile();
                if cli.format == "json" || cli.format == "yaml" { cowen_common::utils::render(&profiles, &cli.format).map_err(|e| anyhow::anyhow!(e))?; }
                else {
                    println!("\n📂 Available Profiles:");
                    for p in profiles { if p == current { println!("  * \x1b[32m{:<20}\x1b[0m (current)", p); } else { println!("    {:<20}", p); } }
                }
            }
            ProfileCommands::Rename { old_name, new_name } => cmd::system::rename_profile(old_name, new_name, &cfg_mgr, vault.clone(), cowen_common::events::event_bus()).await?,
        }
        Commands::Dlq { action } => match action {
            DlqCommands::List => cmd::dlq::list(&active_profile, &config, &cli.format, vault.clone()).await?,
            DlqCommands::Retry { id } => cmd::dlq::retry(&active_profile, &config, id.clone(), vault.clone()).await?,
            DlqCommands::Purge => cmd::dlq::purge(&active_profile, &config, vault.clone()).await?,
        }
        Commands::Log { action } => match action {
            LogCommands::List => cmd::log::list(&active_profile, vault.clone()).await?,
            LogCommands::View { domain, follow, lines } => cmd::log::view(&active_profile, domain, *follow, *lines, vault.clone()).await?,
        }
        Commands::Store { .. } => unreachable!(),
    }
    Ok(())
}
