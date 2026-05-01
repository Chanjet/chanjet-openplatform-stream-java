/// Compile-time string obfuscation macro. Stores XOR-obfuscated bytes
/// in the binary, deobfuscating at runtime. Prevents `strings` extraction.
macro_rules! obfs {
    ($s:expr) => {{
        const _LEN: usize = $s.len();
        const fn _obfs_bytes(s: &[u8]) -> [u8; 256] {
            let seed = (s.len() as u8).wrapping_mul(0x37).wrapping_add(0x5A);
            let mut out = [0u8; 256];
            let mut i = 0;
            while i < s.len() {
                let key = seed.wrapping_add(i as u8).wrapping_mul(0x6D);
                out[i] = s[i] ^ key;
                i += 1;
            }
            out
        }
        const _OBFS: [u8; 256] = _obfs_bytes($s.as_bytes());
        const _SEED: u8 = ($s.len() as u8).wrapping_mul(0x37).wrapping_add(0x5A);
        $crate::core::obfs::deobfs(&_OBFS[.._LEN], _SEED)
    }};
}

mod core;
mod auth;
mod cmd;
mod daemon;

use clap::Parser;
use crate::core::config::ConfigManager;
use crate::core::security;
use crate::core::utils::get_bin_name;
use anyhow::Result;
use std::io::Write;

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
    Status {
        /// 扫描并输出所有存在的 Profile 的状态
        #[arg(short, long)]
        all: bool,
    },
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
    /// 管理并配置全局存储后端与缓存
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
    /// 重命名现有的 Profile
    Rename {
        /// 原 Profile 名称
        old_name: String,
        /// 新 Profile 名称
        new_name: String,
    },
}

#[derive(clap::Subcommand)]
pub enum SystemCommands {
    /// 诊断并检查系统的整体运行状态
    Status {
        /// 扫描并输出所有存在的 Profile 的状态
        #[arg(short, long)]
        all: bool,
    },
}

#[derive(clap::Subcommand)]
pub enum StoreCommands {
    /// 设置全局存储配置
    Set {
        #[arg(long, env = "COWEN_STORE_TYPE", help = "存储后端: local (本地文件), mysql, postgres, mssql, redis")]
        store: Option<String>,
        #[arg(long, env = "COWEN_DB_URL", help = "数据库连接 URL")]
        db_url: Option<String>,
        #[arg(long, env = "COWEN_CACHE_TYPE", help = "缓存后端: none, redis")]
        cache: Option<String>,
        #[arg(long, env = "COWEN_CACHE_URL", help = "缓存连接 URL (Redis)")]
        cache_url: Option<String>,
    },
    /// 查看存储状态并验证连接性
    Status,
    /// 迁移全量数据到新的存储后端
    Migrate {
        /// 目标存储 URL (如 mysql://user:pass@host/db 或 local)
        #[arg(long, help = "目标存储 URL")]
        to: String,
        /// 迁移模式: clone (复制全量数据并切换), move (复制全量数据并切换，且清理源端数据)
        #[arg(long, value_enum, default_value = "clone", help = "迁移模式")]
        mode: crate::core::migration::MigrationMode,
    },
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
    /// 重置当前配置环境的所有凭据与安全设置 (非破坏性)
    Reset,
    /// 注销当前会话并清理所有凭证 (与 Reset 行为一致)
    Logout,
    /// 触发 AppTicket 推送与令牌刷新
    Login {
        /// 强制清除本地 AccessToken 缓存并重新触发网络刷新
        #[arg(short, long)]
        force: bool,
        /// 内部使用：后台 Finalizer 的会话 UUID，终端用户不可见
        #[arg(long, hide = true)]
        finalize: Option<String>,
    },
    /// 查看令牌
    Token {
        /// 强制执行网络刷新逻辑
        #[arg(short, long)]
        refresh: bool,
    },
}

#[derive(clap::Subcommand)]
pub enum DaemonCommands {
    /// 启动后台服务 (包括 Stream 桥接、反向代理与转发器)
    Start {
        /// 指定本地 OpenAPI 反向代理端口 (也可以通过 config 设置)
        #[arg(long)]
        proxy_port: Option<u16>,

        /// 启用本地 OpenAPI 反向代理服务器
        #[arg(long)]
        enable_proxy: bool,

        /// 禁用本地 OpenAPI 反向代理服务器 (覆盖 config)
        #[arg(long)]
        no_proxy: bool,
        
        /// 在前台运行 (阻塞模式)
        #[arg(long)]
        foreground: bool,

        /// 启动所有未运行的守护进程
        #[arg(short, long)]
        all: bool,
    },
    /// 停止正在运行的守护进程
    Stop {
        /// 停止所有正在运行的守护进程
        #[arg(short, long)]
        all: bool,
    },
    /// 重启守护进程
    Restart {
        /// 指定本地 OpenAPI 反向代理端口
        #[arg(long)]
        proxy_port: Option<u16>,

        /// 启用本地 OpenAPI 反向代理服务器
        #[arg(long)]
        enable_proxy: bool,

        /// 禁用本地 OpenAPI 反向代理服务器
        #[arg(long)]
        no_proxy: bool,

        /// 重启所有正在运行的守护进程
        #[arg(short, long)]
        all: bool,
    },
    /// 管理守护进程的系统服务 (实现开机自启动)
    Service {
        #[command(subcommand)]
        action: ServiceCommands,
    },
}

#[derive(clap::Subcommand)]
pub enum ServiceCommands {
    /// 安装自启动服务
    Install,
    /// 卸载自启动服务
    Uninstall,
    /// 查看服务注册状态
    Status,
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

    // CAPTURE SIGNALS: Ensure graceful shutdown
    let shutdown_handle = tokio::spawn(async move {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("\n\x1b[33mReceived CTRL-C, shutting down gracefully...\x1b[0m");
                tracing::warn!(target: "sys", "Received SIGINT, shutting down");
            }
            _ = async {
                #[cfg(unix)]
                {
                    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
                    sigterm.recv().await;
                    println!("\n\x1b[33mReceived SIGTERM, shutting down gracefully...\x1b[0m");
                    tracing::warn!(target: "sys", "Received SIGTERM, shutting down");
                }
                #[cfg(not(unix))]
                {
                    std::future::pending::<()>().await;
                }
            } => {}
        }
    });

    tokio::select! {
        res = run() => {
            if let Err(e) = res {
                let err_msg = e.to_string();
                if err_msg.contains("SKIPPED:") {
                    // Message already printed via eprintln! in load(), just exit gracefully
                    std::process::exit(1);
                }
                tracing::error!(target: "sys", error = %err_msg, "CLI execution failed");
                eprintln!("❌ Error: {}", err_msg);
                std::process::exit(1);
            }
        }
        _ = shutdown_handle => {
            // Give a tiny grace period for background tasks to cleanup
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }
}


async fn run() -> Result<()> {
    let cli = Cli::parse();
    let bin_name = get_bin_name();

    // 1. Core Paths
    let app_dir = crate::core::config::get_app_dir();
    let log_dir = app_dir.join("logs");

    // 2. Load Config to get Log Settings
    let cfg_mgr = ConfigManager::new()?;
    let mut app_config = cfg_mgr.load_app_config().await?;

    // Initialize Vault and Validator EARLY (after AppConfig is known)
    // This allows the initial profile load to be validated against distributed storage restrictions.
    let fingerprint = security::get_machine_fingerprint()?;
    let vault = crate::core::vault::create_vault(&app_config, &app_dir, &fingerprint).await?;
    cfg_mgr.set_vault(vault.clone());

    let auth_cli = crate::auth::create_auth_client_with_vault(vault.clone());
    cfg_mgr.set_validator(std::sync::Arc::new(crate::auth::AuthProviderValidator::new(auth_cli.clone())));

    let mut active_profile = cli.profile.clone().unwrap_or_else(|| cfg_mgr.get_default_profile());

    // Logic Fix: Ensure 'init' always creates a NEW profile instead of overwriting the current one.
    if matches!(&cli.command, Commands::Init { .. }) {
        if cli.profile.is_none() {
            active_profile = cfg_mgr.get_next_profile_name().await?;
            println!("🪄 No profile name provided. Automatically generating new profile: \x1b[1;32m{}\x1b[0m", active_profile);
        }
    }
    
    // Load config for the target profile (from vault or local)
    let mut config = match cfg_mgr.load(&active_profile).await {
        Ok(cfg) => cfg,
        Err(e) if e.to_string().contains("SKIPPED:") => return Err(e),
        Err(_) => crate::core::config::Config::default_with_profile(&active_profile),
    };

    // --- Cloud-Native Override ---
    if let Ok(key) = std::env::var("COWEN_APP_KEY") { config.app_key = key; }
    if let Ok(secret) = std::env::var("COWEN_APP_SECRET") { config.app_secret = secret; }
    if let Ok(ek) = std::env::var("COWEN_ENCRYPT_KEY") { config.encrypt_key = ek; }
    if let Ok(target) = std::env::var("COWEN_WEBHOOK_TARGET") { config.webhook_target = target; }
    if let Ok(url) = std::env::var("COWEN_OPENAPI_URL") { config.openapi_url = url; }
    if let Ok(url) = std::env::var("COWEN_STREAM_URL") { config.stream_url = url; }
    if let Ok(port) = std::env::var("COWEN_PROXY_PORT") {
        if let Ok(p) = port.parse::<u16>() { config.proxy_port = p; }
    }
    if let Ok(mode) = std::env::var("COWEN_APP_MODE") {
        config.app_mode = match mode.as_str() {
            "self-built" => crate::auth::models::AuthMode::SelfBuilt,
            "store-app" => crate::auth::models::AuthMode::StoreApp,
            _ => crate::auth::models::AuthMode::Oauth2,
        };
    }

    // Override config flags if CLI provides them
    if cli.no_telemetry {
        config.telemetry_enabled = false;
    }
    if cli.no_ai {
        config.ai_enabled = false;
    }


    // 3. Initialize Telemetry (Structured & Rotated Logging)
    let (vault_tx, vault_rx) = tokio::sync::watch::channel(None);
    let _guards = match crate::core::telemetry::init_telemetry(log_dir, &active_profile, &config.log, vault_rx) {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("⚠️ Warning: Telemetry system failed to initialize: {}. Continuing without structured logging.", e);
            None
        }
    };
    tracing::info!(target: "sys", "{} starting (version {})", bin_name, env!("CARGO_PKG_VERSION"));
    tracing::info!(target: "sys", profile = %active_profile, "active profile loaded");

    // 4. Check for Activation (First Run)
    let marker_path = app_dir.join(".telemetry_marker");
    if !marker_path.exists() && cli.format == "text" {
        // --- 隐私声明 / Privacy Notice ---
        println!("\n\x1b[1;36m🛡️  安全与隐私提示 (Security & Privacy Notice)\x1b[0m");
        println!("--------------------------------------------------");
        println!("欢迎使用 cowen CLI！为了提供更好的服务，本工具包含以下特性：");
        println!("- \x1b[1m遥测数据 (Telemetry)\x1b[0m: 我们会收集匿名指纹、OS/Arch 及命令运行情况以优化产品。");
        println!("- \x1b[1mAI 语义搜索 (AI Search)\x1b[0m: 内置极轻量 ONNX 引擎，通过本地向量化实现 API 快速检索。");
        println!("\n您可以随时通过以下方式禁用这些功能：");
        println!("1. 在命令中添加 \x1b[33m--no-telemetry\x1b[0m 或 \x1b[33m--no-ai\x1b[0m");
        println!("2. 修改配置文件 (yaml) 中的 \x1b[33mtelemetry_enabled\x1b[0m 或 \x1b[33mai_enabled\x1b[0m 为 false");
        println!("--------------------------------------------------\n");

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
        Commands::Status { .. } => "status",
        Commands::Config => "config",
        Commands::Reset => "reset",
        Commands::Completion { .. } => "completion",
        Commands::Profile { .. } => "profile",
        Commands::Dlq { .. } => "dlq",
        Commands::Log { .. } => "log",
        Commands::Store { .. } => "store",
        Commands::System { .. } => "system",
    };
    crate::core::telemetry::report_event(&config, "command_run".to_string(), serde_json::json!({ "cmd": cmd_name }));

    // Handle 'store' command early before Vault creation to allow fixing broken storage configs
    // Handle 'store' commands that don't need a vault first (Set/Status)
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
            _ => {} // Migrate needs vault, handle later
        }
    }

    let _ = vault_tx.send(Some(vault.clone()));

    // Now handle 'store migrate' which needs the vault
    if let Commands::Store { action: StoreCommands::Migrate { to, mode } } = &cli.command {
        cmd::store::migrate(&cfg_mgr, to, *mode).await?;
        return Ok(());
    }

    // 4. Automatic Shell Completion Installation (One-time check)
    if crate::cmd::completion::is_auto_install_needed() {
        let _ = crate::cmd::completion::install_completion(None);
    }

    // 5. Ensure daemon is running and up to date with this CLI binary
    if !matches!(&cli.command, Commands::Daemon { .. } | Commands::Reset | Commands::Init { .. }) {
        let _ = crate::cmd::system::ensure_daemon_running(&active_profile, &config, &cfg_mgr, vault.clone(), &auth_cli).await;
    }

    // 6. Execute Command
    match &cli.command {
// ... existing match ...
        Commands::Init { 
            app_key, 
            app_secret, 
            certificate,
            encrypt_key,
            webhook_target,
            openapi_url,
            stream_url,
            app_mode,
            proxy_port,
        } => {
            cmd::init::execute(
                &active_profile, 
                &cfg_mgr, 
                &mut app_config,
                vault.clone(), 
                app_key, 
                app_secret, 
                certificate,
                encrypt_key,
                webhook_target,
                openapi_url,
                stream_url,
                app_mode,
                proxy_port,
                true,
            ).await?;
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
                println!("Usage: {} api [METHOD] [PATH] or use subcommands (list, spec)", bin_name);
            }
        },

        Commands::Auth { action } => match action {
            AuthCommands::Status => {
                cmd::system::status(&active_profile, &cfg_mgr, vault.clone(), &cli.format, false).await?;
            }
            AuthCommands::Reset | AuthCommands::Logout => {
                cmd::auth::logout(&active_profile, &config, &auth_cli).await?;
            }
            AuthCommands::Login { force, finalize } => {
                cmd::auth::login(&active_profile, &config, &auth_cli, *force, finalize.as_deref()).await?;
            }
            AuthCommands::Token { refresh } => {
                cmd::auth::token(&active_profile, &config, &auth_cli, &cli.format, *refresh).await?;
            }
        },
        Commands::Daemon { action } => match action {
            DaemonCommands::Start { proxy_port, enable_proxy, no_proxy, foreground, all } => {
                let mut updated_config = config.clone();
                let mut changed = false;
                
                if let Some(p) = proxy_port {
                    updated_config.proxy_port = *p;
                    changed = true;
                }
                if *enable_proxy {
                    updated_config.proxy_enabled = true;
                    changed = true;
                } else if *no_proxy {
                    updated_config.proxy_enabled = false;
                    changed = true;
                }
                
                if changed && !*all {
                    cfg_mgr.save(&active_profile, &updated_config).await?;
                }

                cmd::daemon::start(&active_profile, &updated_config, updated_config.proxy_port, updated_config.proxy_enabled, *foreground, *all, &cfg_mgr, vault.clone()).await?;
            }
            DaemonCommands::Stop { all } => {
                cmd::daemon::stop(&active_profile, *all, &cfg_mgr).await?;
            }
            DaemonCommands::Restart { proxy_port, enable_proxy, no_proxy, all } => {
                let mut updated_config = config.clone();
                let mut changed = false;
                if let Some(p) = proxy_port {
                    updated_config.proxy_port = *p;
                    changed = true;
                }
                if *enable_proxy {
                    updated_config.proxy_enabled = true;
                    changed = true;
                } else if *no_proxy {
                    updated_config.proxy_enabled = false;
                    changed = true;
                }

                if changed && !*all {
                    cfg_mgr.save(&active_profile, &updated_config).await?;
                }

                cmd::daemon::restart(&active_profile, &updated_config, updated_config.proxy_port, updated_config.proxy_enabled, *all, &cfg_mgr, vault.clone()).await?;
            }
            DaemonCommands::Service { action } => match action {
                ServiceCommands::Install => {
                    cmd::daemon::service::execute(cmd::daemon::service::ServiceAction::Install).await?;
                }
                ServiceCommands::Uninstall => {
                    cmd::daemon::service::execute(cmd::daemon::service::ServiceAction::Uninstall).await?;
                }
                ServiceCommands::Status => {
                    cmd::daemon::service::execute(cmd::daemon::service::ServiceAction::Status).await?;
                }
            }
        },
        Commands::Status { all } => {
            cmd::system::status(&active_profile, &cfg_mgr, vault.clone(), &cli.format, *all).await?;
        }
        Commands::System { action } => {
            match action {
                SystemCommands::Status { all } => {
                    cmd::system::status(&active_profile, &cfg_mgr, vault.clone(), &cli.format, *all).await?;
                }
            }
        }
        Commands::Config => {
            cmd::system::config(&active_profile, &cfg_mgr, &cli.format).await?;
        }
        Commands::Reset => {
            cmd::system::reset(&active_profile, Some(vault.as_ref()), &cfg_mgr).await?;
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
                let mut buf = Vec::new();
                match crate::cmd::completion::generate_completion(*s, &mut buf) {
                    Ok(_) => {
                        let _ = std::io::stdout().write_all(&buf);
                    },
                    Err(e) => eprintln!("❌ Failed to generate completion: {}", e),
                }
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
                let profiles = cfg_mgr.list_profiles().await?;
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
            ProfileCommands::Rename { old_name, new_name } => {
                cmd::system::rename_profile(old_name, new_name, &cfg_mgr, vault.clone()).await?;
            }
        },
        Commands::Dlq { action } => match action {
            DlqCommands::List => {
                cmd::dlq::list(&active_profile, &config, &cli.format, vault.clone()).await?;
            }
            DlqCommands::Retry { id } => {
                cmd::dlq::retry(&active_profile, &config, id, vault.clone()).await?;
            }
            DlqCommands::Purge => {
                cmd::dlq::purge(&active_profile, &config, vault.clone()).await?;
            }
        },
        Commands::Log { action } => match action {
            LogCommands::List => {
                cmd::log::list(&active_profile, vault.clone()).await?;
            }
            LogCommands::View { domain, follow, lines } => {
                cmd::log::view(&active_profile, domain, *follow, *lines, vault.clone()).await?;
            }
        },
        Commands::Store { .. } => unreachable!("Store command should have been handled early"),
    }

    Ok(())
}
