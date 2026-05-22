pub(crate) mod core;
pub(crate) mod cmd;

use clap::Parser;
use cowen_config::ConfigManager;
use cowen_common::security;
use cowen_common::utils::get_bin_name;
use anyhow::Result;
use std::io::Write;
use std::sync::Arc;
use cowen_auth::client::Client;
use cowen_common::daemon::DaemonService;

pub trait Colorize {
    fn red(&self) -> String;
    fn green(&self) -> String;
    fn yellow(&self) -> String;
    fn cyan(&self) -> String;
    fn bold(&self) -> String;
    fn dimmed(&self) -> String;
    fn underline(&self) -> String;
}

impl<T: std::fmt::Display> Colorize for T {
    fn red(&self) -> String { format!("\x1b[31m{}\x1b[0m", self) }
    fn green(&self) -> String { format!("\x1b[32m{}\x1b[0m", self) }
    fn yellow(&self) -> String { format!("\x1b[33m{}\x1b[0m", self) }
    fn cyan(&self) -> String { format!("\x1b[36m{}\x1b[0m", self) }
    fn bold(&self) -> String { format!("\x1b[1m{}\x1b[0m", self) }
    fn dimmed(&self) -> String { format!("\x1b[2m{}\x1b[0m", self) }
    fn underline(&self) -> String { format!("\x1b[4m{}\x1b[0m", self) }
}

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
        /// 展示所有 profile 的配置
        #[arg(short, long)]
        all: bool,

        #[command(subcommand)]
        action: Option<ConfigCommands>,
    },
    /// 重置当前环境的配置状态
    Reset {
        /// Dry run: 列出将要删除的文件，但不实际删除
        #[arg(long)]
        dry_run: bool,
    },
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
    /// 运行环境诊断工具，排查网络、存储与配置问题
    Doctor {
        /// 指定要诊断的 Profile (默认为 active profile)
        #[arg(short, long)]
        profile: Option<String>,
        
        /// 开启详细诊断模式
        #[arg(short, long)]
        verbose: bool,

        /// 尝试自动修复发现的问题 (如存储 Schema 更新)
        #[arg(long)]
        fix: bool,
    },
    /// 管理本地动态搜索与扩展插件 (扫描、启用与禁用)
    Plugins {
        #[command(subcommand)]
        action: PluginsCommands,
    },
    /// 查看过去的系统事件流与故障轨迹 (诊断回溯)
    Events(cmd::events::EventsArgs),
    /// 管理并检查系统整体状态
    System {
        #[command(subcommand)]
        action: SystemCommands,
    },
    /// 获取当前 CLI 构建版本信息
    #[command(name = "version")]
    Version {
        #[arg(short = 'o', long, default_value = "text", help = "输出格式 (text, json)")]
        format: String,
    },
}

#[derive(clap::Subcommand)]
pub enum ConfigCommands {
    /// 设置当前环境配置项的值 (e.g., cowen config set log.level debug)
    Set {
        #[arg(help = "要设置的配置项路径 (例如 log.level, storage.db_url)")]
        key: String,
        #[arg(help = "配置项的新值")]
        value: String,
        #[arg(long, help = "动态修改全局基础设施配置 (app.yaml)")]
        global: bool,
    },
    /// 获取指定配置项的当前数值
    Get {
        #[arg(help = "要获取的配置项路径")]
        key: String,
    },
    /// 删除局部配置项 (支持配置数组坍缩)
    Unset {
        #[arg(help = "要删除的配置项路径 (例如 search.plugins.0)")]
        key: String,
    },
    /// 列出当前生效的所有配置项目
    List {
        #[arg(short, long, default_value = "table", help = "列表的展示输出格式 (table, json)")]
        format: String,
    },
}

#[derive(clap::Subcommand)]
pub enum ProfileCommands {
    /// 切换并激活到指定的 Profile 环境
    Use {
        /// 要激活的 Profile 环境名称
        name: String,
    },
    /// 显示当前默认激活的 Profile 环境名称
    Current,
    /// 列出所有已配置的多 Profile 环境列表
    List,
    /// 重命名已有的环境及其关联的安全存储与数据库记录
    Rename {
        /// 当前需要重命名的 Profile 名称
        old_name: String,
        /// 新的环境 Profile 名称
        new_name: String,
    },
}

#[derive(clap::Subcommand)]
pub enum PluginsCommands {
    /// 扫描并列出 ~/.cowen/plugins/ 目录下的可用扩展插件
    List,
    /// 启用指定的插件并将其注册到全局配置中
    Enable {
        /// 要启用的插件名称 (支持去扩展名的文件名，如 libcowen_search_embedding)
        name: String,
    },
    /// 禁用指定的插件
    Disable {
        /// 要禁用的插件名称
        name: String,
    },
}

#[derive(clap::Subcommand)]
pub enum SystemCommands {
    /// 检查系统的整体运行状态指标 (包括 Daemon、Store、Auth、AI 等)
    Status {
        /// 扫描并输出所有 Profile 环境的详细运行状态
        #[arg(short, long)]
        all: bool,
    },
}

#[derive(clap::Subcommand)]
pub enum StoreCommands {
    /// 配置全局存储后端与缓存的连接参数与引擎类型
    Set {
        /// 主存储引擎类型 (可选 sqlite / innerdb / mysql / postgres / redis / local)
        #[arg(
            long, 
            env = "COWEN_STORE_TYPE",
            long_help = "主存储数据库引擎类型。\n\n支持的取值:\n  - sqlite / innerdb: 本地高性能 SQLite 数据库文件\n  - mysql / postgres: 远程分布式关系数据库\n  - redis: 高并发 Key-Value 内存存储\n  - local: 遗留的扁平化文件本地配置存储"
        )]
        store: Option<String>,
        /// 数据库连接 URL 地址
        #[arg(
            long, 
            env = "COWEN_DB_URL",
            long_help = "选定主存储引擎所需的物理连接 URL 地址。\n\n格式示例:\n  - sqlite:data/cowen.db\n  - postgres://user:pass@localhost:5432/cowen\n  - mysql://user:pass@localhost:3306/cowen\n  - redis://localhost:6379"
        )]
        db_url: Option<String>,
        /// 全局缓存引擎类型 (如 redis, memory)
        #[arg(long, env = "COWEN_CACHE_TYPE", help = "全局缓存引擎类型")]
        cache: Option<String>,
        /// 缓存连接 URL 地址 (如 redis://localhost:6379)
        #[arg(long, env = "COWEN_CACHE_URL", help = "缓存连接的物理 URL 地址")]
        cache_url: Option<String>,
    },
    /// 检查当前配置的主存储后端与缓存连接性及健康状态
    Status,
    /// 在不同的底层存储后端之间安全地迁移已保存的配置与凭据状态
    Migrate {
        /// 迁移的目标数据库连接 URL 地址
        #[arg(
            long, 
            value_name = "URL",
            long_help = "需要将当前数据迁移到的目标数据库连接 URL 地址。\n\n格式与 Set 命令一致，例如 sqlite:data/new.db"
        )]
        to: String,
        /// 数据迁移的交互工作模式
        #[arg(long, value_enum, default_value = "clone", help = "数据迁移模式 (clone: 复制数据; move: 物理迁移)")]
        mode: cowen_store::migration::MigrationMode,
    },
}

#[derive(clap::Subcommand)]
pub enum ApiCommands {
    /// 搜索并列出当前环境规格中所有授权可用的平台 API 列表
    List {
        /// 提供搜索词以模糊过滤或语义检索特定的 API 接口
        #[arg(short, long, help = "API 模糊过滤或自然语言语义检索词")]
        search: Option<String>,
        /// 分页查看的页码
        #[arg(long, default_value_t = 1, help = "要查看的页码")]
        page: usize,
        /// 每页显示的 API 记录数量
        #[arg(short = 'n', long, default_value_t = 20, help = "每页显示的记录限制条数")]
        page_size: usize,
        /// 强制从云端开放平台重新拉取同步最新的规约定义
        #[arg(short, long, help = "强制从云平台刷新同步最新规约")]
        refresh: bool,
    },
    /// 查看指定 API 接口在本地规格中的具体数据与详情规约定义
    Spec {
        /// 接口对应的 HTTP 方法名 (如 GET, POST)
        method: String,
        /// 接口地址路径 (如 /v1/user)
        path: String,
        /// 以原始 JSON 结构输出接口规约详情
        #[arg(long, help = "直接输出原始 JSON 格式规约定义")]
        raw: bool,
    },
}

#[derive(clap::Subcommand)]
pub enum AuthCommands {
    /// 检查当前环境的安全凭据健康状况与剩余寿命
    Status,
    /// 强制清空并重置本地的所有身份认证与换票状态
    Reset,
    /// 安全清除本地内存与 Vault 中的 Token 凭据并退出会话
    Logout,
    /// 触发与开放平台的换票及交互式 OAuth2 登录流
    Login {
        /// 强制废弃本地 Token 并立即触发重新网络登录
        #[arg(short, long, help = "强制废弃缓存凭据并立即网络重登录")]
        force: bool,
        /// 仅供内部流转，用于异步完成登录握手回传
        #[arg(long, hide = true)]
        finalize: Option<String>,
    },
    /// 获取或 proactive 刷新当前 Profile 的 AccessToken
    Token {
        /// 主动触发向开放平台发起 Token 的寿命续期刷新
        #[arg(short, long, help = "强制立即向开放平台执行 Token 刷新")]
        refresh: bool,
    },
    /// 从共享存储中强制同步最新凭据数据到当前工作上下文
    Reload,
}

#[derive(clap::Subcommand)]
pub enum DaemonCommands {
    /// 启动长连接桥接器、反向代理与 Token 自适应自动续签后台服务
    Start {
        /// 覆盖代理端口
        #[arg(long, help = "覆盖本地反向代理服务监听端口")]
        proxy_port: Option<u16>,
        /// 覆盖监控管理服务的通信监听端口
        #[arg(long, help = "覆盖守护进程管理端口")]
        monitor_port: Option<u16>,
        /// 强制开启 HTTP API 签名反向代理服务
        #[arg(long, help = "强制启用 API 请求代理网关能力")]
        enable_proxy: bool,
        /// 强制禁用 HTTP API 反向代理
        #[arg(long, help = "强制禁用代理网关，仅做流消息桥接")]
        no_proxy: bool,
        /// 在控制台前台挂起运行以方便直接观察日志流
        #[arg(long, help = "在前台阻塞式启动并打印交互日志")]
        foreground: bool,
        /// 同时为所有已配置激活的 Profile 环境一并启动后台守护服务
        #[arg(short, long, help = "批量为所有已配置的 Profile 环境并行启动服务")]
        all: bool,
    },
    /// 优雅停止运行中的后台守护进程服务
    Stop {
        /// 批量停止当前机器上运行的所有 Profile 后台守护进程
        #[arg(short, long, help = "批量停止所有正在运行环境的守护服务")]
        all: bool,
    },
    /// 重启守护服务并应用最新加载的配置文件参数
    Restart {
        /// 重启并覆盖代理网关监听端口
        #[arg(long, help = "重新指定反向代理服务监听端口")]
        proxy_port: Option<u16>,
        /// 重启强制开启请求签名反向代理
        #[arg(long, help = "强制启用 API 请求代理网关")]
        enable_proxy: bool,
        /// 重启强制关闭请求代理
        #[arg(long, help = "强制禁用代理网关，仅做消息桥接")]
        no_proxy: bool,
        /// 同时为所有已配置的 Profile 重启其关联的守护进程服务
        #[arg(short, long, help = "批量为所有 Profile 服务进行重启")]
        all: bool,
    },
    /// 在不停止主控服务 Master 守护的前提下，平滑热重启特定的工作 Worker 子进程
    Reload {
        /// 批量热重启所有的工作 Worker 子进程
        #[arg(short, long, help = "热重启所有激活环境的工作子进程")]
        all: bool,
    },
    /// 管理操作系统的底层自启动服务单元 (支持 systemd / launchd)
    Service { 
        #[command(subcommand)] 
        action: ServiceCommands 
    },
}

#[derive(clap::Subcommand)]
pub enum ServiceCommands {
    /// 安装后台守护守护服务到操作系统开机自启动单元中
    Install,
    /// 从操作系统服务管理器中安全卸载守护进程开机单元
    Uninstall,
    /// 诊断操作系统服务管理器中的当前单元运行生命周期状态
    Status,
}

#[derive(clap::Subcommand)]
pub enum DlqCommands {
    /// 列出当前死信队列 (DLQ) 中因为网络或本地重试耗尽而堆积的异常事件
    List {
        /// 要查看的页码
        #[arg(long, default_value = "1", help = "死信记录查看的分页页码")]
        page: usize,
        /// 每页显示的死信记录数限制
        #[arg(short = 'n', long, default_value = "20", help = "每页显示的死信记录条数")]
        page_size: usize,
    },
    /// 手动重新投递重试特定 ID 的死信队列异常事件
    Retry {
        /// 需要手动触发重发动作的死信事件 ID
        #[arg(help = "死信记录的唯一事件 ID (UUID)")]
        id: String,
    },
    /// 物理抹除死信队列中堆积的所有历史事件
    Purge,
}

#[derive(clap::Subcommand)]
pub enum LogCommands {
    /// 列出当前环境支持的所有日志域及其对应的物理审计文件信息
    List,
    /// 查看审计文件或实时跟踪多域日志流的控制台输出
    View {
        /// 需要查看的日志域 (可选 sys / audit / stream / dlq，缺省为 sys)
        #[arg(default_value = "sys", help = "要查询的日志域。可用 'cowen log list' 命令获取")]
        domain: String,
        /// 挂起终端实时追踪并跟随时时产生的新日志流水
        #[arg(short, long, help = "挂起终端以 follow 模式实时追随日志流")]
        follow: bool,
        /// 默认在起始位置展示日志审计文件的尾部行数
        #[arg(short = 'n', long, default_value_t = 10, help = "要在尾部默认读取的日志行数")]
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
    let _ = cfg_mgr.auto_migrate().await;
    let mut app_config = cfg_mgr.load_app_config().await.map_err(|e| anyhow::anyhow!(e))?;

    let fingerprint = security::get_machine_fingerprint().map_err(|e| anyhow::anyhow!(e))?;
    let vault = cowen_store::create_vault(&app_config, &app_dir, &fingerprint).await.map_err(|e| anyhow::anyhow!(e))?;
    let _ = cfg_mgr.set_vault(vault.clone());

    let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
    let _ = cfg_mgr.set_validator(std::sync::Arc::new(cowen_auth::AuthProviderValidator::new(auth_cli.clone())));

    let mut active_profile = cli.profile.clone().unwrap_or_else(|| cfg_mgr.get_default_profile());
    #[cfg(unix)]
    let daemon_svc: Arc<dyn DaemonService> = {
        let is_foreground_start = matches!(&cli.command, Commands::Daemon { action: DaemonCommands::Start { foreground: true, .. } });

        if is_foreground_start {
            Arc::new(cowen_server::ServerDaemonService::new(cfg_mgr.clone(), None))
        } else {
            let uds_path = cowen_common::ipc::get_uds_path();
            Arc::new(cowen_common::ipc::client::IpcDaemonService::new(uds_path))
        }
    };
    #[cfg(not(unix))]
    let daemon_svc: Arc<dyn DaemonService> = Arc::new(cowen_server::ServerDaemonService::new(cfg_mgr.clone(), None));

    if matches!(&cli.command, Commands::Init { .. })
        && cli.profile.is_none() {
            active_profile = cfg_mgr.get_next_profile_name().await.map_err(|e| anyhow::anyhow!(e))?;
            println!("🪄 No profile name provided. Automatically generating new profile: \x1b[1;32m{}\x1b[0m", active_profile);
        }
    
    let mut config = match cfg_mgr.load(&active_profile).await {
        Ok(cfg) => cfg,
        Err(e) if e.to_string().contains("SKIPPED:") => return Err(anyhow::anyhow!(e)),
        Err(e) => {
            let is_lifecycle_cmd = matches!(&cli.command, Commands::Reset { .. } | Commands::Init { .. } | Commands::Profile { .. });
            if cfg_mgr.exists(&active_profile).await && !is_lifecycle_cmd {
                return Err(anyhow::anyhow!("Failed to load existing profile '{}': {}. Try 'cowen reset -p {}' if the config is corrupted.", active_profile, e, active_profile));
            }
            cowen_common::Config::default_with_profile(&active_profile)
        }
    };

    config.apply_env_overrides();

    let mut app_cfg = cfg_mgr.load_app_config().await.unwrap_or_default();
    
    if cli.no_telemetry { app_cfg.telemetry_enabled = false; }

    let (vault_tx, vault_rx) = tokio::sync::watch::channel(None);
    let telemetry_control = match core::telemetry::init_telemetry(log_dir, &active_profile, &app_cfg.log, vault_rx) {
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
        Commands::Reset { .. } => "reset",
        Commands::Completion { .. } => "completion",
        Commands::Profile { .. } => "profile",
        Commands::Dlq { .. } => "dlq",
        Commands::Log { .. } => "log",
        Commands::Store { .. } => "store",
        Commands::Doctor { .. } => "doctor",
        Commands::Plugins { .. } => "plugins",
        Commands::Events(..) => "events",
        Commands::System { .. } => "system",
        Commands::Version { .. } => "version",
    };
    core::telemetry::report_event(&config, &app_config, "command_run".to_string(), serde_json::json!({ "cmd": cmd_name }));

    if let Commands::Version { format } = &cli.command {
        cmd::version::run(format).await?;
        return Ok(());
    }

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
        cmd::store::migrate(&cfg_mgr, to, *mode, daemon_svc.clone()).await?;
        return Ok(());
    }

    if std::env::var("COWEN_SKIP_COMPLETION_INSTALL").is_err() && cmd::completion::is_auto_install_needed() {
        let _ = cmd::completion::install_completion(None);
    }

    // --- Daemon Lifecycle Enforcement ---
    // 1. Version Sync: Ensure all CURRENTLY RUNNING daemons match the CLI version.
    // We only skip this during explicit stop, reset, or init operations.
    let skip_version_sync = match cmd_name {
        "reset" | "init" => true,
        "daemon" => matches!(&cli.command, Commands::Daemon { action: DaemonCommands::Stop { .. } }),
        _ => false,
    } || std::env::var("COWEN_SKIP_DAEMON_RECOVERY").is_ok();
    
    if !skip_version_sync {
        let _ = cmd::system::enforce_daemon_version_sync(&active_profile, &cfg_mgr, vault.clone(), daemon_svc.clone()).await;
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
                proxy_port: *proxy_port,
                auto_start: false,
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
            AuthCommands::Reload => {
                let _ = auth_cli.get_token(&active_profile, &config, &reqwest::header::HeaderMap::new()).await.map_err(|e: cowen_common::CowenError| anyhow::anyhow!(e))?;
                
                // Signal running daemon to reload if active
                if let Some(_info) = cowen_monitor::status::get_active_daemon_info(&active_profile) {
                    let app_cfg = cfg_mgr.load_app_config().await?;
                    let client = cowen_monitor::client::MonitorClient::new(app_cfg.monitor_port);
                    let _ = client.reload_worker(&active_profile).await;
                } else {
                    let _ = daemon_svc.reload_daemon(&active_profile).await;
                }
                println!("✅ Token reloaded and synchronized from shared storage.");
            }
        }
        Commands::Daemon { action } => match action {
            DaemonCommands::Start { proxy_port, monitor_port, enable_proxy, no_proxy, foreground, all } => {
                let mut updated_config = config.clone();
                let mut changed = false;
                if let Some(p) = proxy_port { if updated_config.proxy_port != *p { updated_config.proxy_port = *p; changed = true; } }
                if *enable_proxy { if !updated_config.proxy_enabled { updated_config.proxy_enabled = true; changed = true; } }
                else if *no_proxy && updated_config.proxy_enabled { updated_config.proxy_enabled = false; changed = true; }
                
                if let Some(m) = monitor_port {
                    let mut app_cfg = cfg_mgr.load_app_config().await?;
                    if app_cfg.monitor_port != *m {
                        app_cfg.monitor_port = *m;
                        cfg_mgr.save_app_config(&app_cfg).await?;
                    }
                }

                if changed && !*all { cfg_mgr.save(&active_profile, &mut updated_config).await.map_err(|e| anyhow::anyhow!(e))?; }
                
                if *foreground {
                    cmd::daemon::start(&active_profile, &updated_config, updated_config.proxy_port, updated_config.proxy_enabled, true, *all, &cfg_mgr, vault.clone(), telemetry_control.clone(), daemon_svc.clone()).await?;
                } else {
                    cmd::daemon::restart(&active_profile, &updated_config, updated_config.proxy_port, updated_config.proxy_enabled, *all, &cfg_mgr, vault.clone(), telemetry_control.clone(), daemon_svc.clone()).await?;
                }
            }
            DaemonCommands::Stop { all } => cmd::daemon::stop(&active_profile, *all, &cfg_mgr).await?,
            DaemonCommands::Restart { proxy_port, enable_proxy, no_proxy, all } => {
                let mut updated_config = config.clone();
                let mut changed = false;
                if let Some(p) = proxy_port { if updated_config.proxy_port != *p { updated_config.proxy_port = *p; changed = true; } }
                if *enable_proxy { if !updated_config.proxy_enabled { updated_config.proxy_enabled = true; changed = true; } }
                else if *no_proxy && updated_config.proxy_enabled { updated_config.proxy_enabled = false; changed = true; }
                if changed && !*all { cfg_mgr.save(&active_profile, &mut updated_config).await.map_err(|e| anyhow::anyhow!(e))?; }
                cmd::daemon::restart(&active_profile, &updated_config, updated_config.proxy_port, updated_config.proxy_enabled, *all, &cfg_mgr, vault.clone(), telemetry_control.clone(), daemon_svc.clone()).await?;
            }
            DaemonCommands::Reload { all } => {
                let profiles = if *all { cfg_mgr.list_profiles().await? } else { vec![active_profile.clone()] };
                
                if let Some(_info) = cowen_monitor::status::get_active_daemon_info(&active_profile) {
                    let app_cfg = cfg_mgr.load_app_config().await?;
                    let client = cowen_monitor::client::MonitorClient::new(app_cfg.monitor_port);
                    for p in profiles {
                        let _ = client.reload_worker(&p).await;
                    }
                } else {
                    for p in profiles {
                        let _ = daemon_svc.reload_daemon(&p).await;
                    }
                }
                println!("✅ Daemon workers reloaded successfully.");
            }
            DaemonCommands::Service { action } => match action {
                ServiceCommands::Install => cmd::daemon::service::execute(cmd::daemon::service::ServiceAction::Install).await?,
                ServiceCommands::Uninstall => cmd::daemon::service::execute(cmd::daemon::service::ServiceAction::Uninstall).await?,
                ServiceCommands::Status => cmd::daemon::service::execute(cmd::daemon::service::ServiceAction::Status).await?,
            }
        }
        Commands::Doctor { profile: doctor_profile, verbose, fix } => {
            let target_profile = doctor_profile.as_ref().unwrap_or(&active_profile);
            let target_config = if doctor_profile.is_some() {
                cfg_mgr.load(target_profile).await.unwrap_or_else(|_| cowen_common::config::Config::default_with_profile(target_profile))
            } else {
                config.clone()
            };
            cmd::doctor::execute(target_profile, &target_config, *verbose, *fix, vault.clone(), &cfg_mgr).await?;
        }
        Commands::Events(args) => cmd::events::execute(args).await?,
        Commands::Status { all } => cmd::system::status(&active_profile, &cfg_mgr, vault.clone(), &cli.format, *all).await?,
        Commands::System { action } => match action { SystemCommands::Status { all } => cmd::system::status(&active_profile, &cfg_mgr, vault.clone(), &cli.format, *all).await? },
        Commands::Plugins { action } => match action {
            PluginsCommands::List => cmd::plugins::list(&cfg_mgr).await?,
            PluginsCommands::Enable { name } => cmd::plugins::enable(&cfg_mgr, name).await?,
            PluginsCommands::Disable { name } => cmd::plugins::disable(&cfg_mgr, name).await?,
        },
        Commands::Config { action, all } => match action {
            Some(ConfigCommands::Set { key, value, global }) => {
                if *global {
                    let global_strategy = cowen_config::strategy::GlobalAppConfigStrategy;
                    use cowen_config::strategy::ConfigStrategy;
                    if !global_strategy.matches(key) {
                        return Err(anyhow::anyhow!("❌ Error: Key '{}' is not a global infrastructure config. Only global configs (e.g. log.level, storage.*, security.*, search.*, monitor_port, openapi_url, stream_url, telemetry_enabled) can be modified globally.", key));
                    }
                }
                cfg_mgr.set_value(&active_profile, key, value).await.map_err(|e| anyhow::anyhow!(e))?;
                println!("✅ Successfully updated '{}' to '{}'", key, value);
            }
            Some(ConfigCommands::Get { key }) => {
                let val = cfg_mgr.get_value(&active_profile, key).await.map_err(|e| anyhow::anyhow!(e))?;
                println!("{}", val);
            }
            Some(ConfigCommands::Unset { key }) => {
                cfg_mgr.unset_value(&active_profile, key).await.map_err(|e| anyhow::anyhow!(e))?;
                println!("✅ Successfully unset '{}'", key);
            }
            Some(ConfigCommands::List { format }) => {
                let val = cfg_mgr.list_values(&active_profile).await.map_err(|e| anyhow::anyhow!(e))?;
                if format == "json" {
                    println!("{}", serde_json::to_string_pretty(&val).unwrap());
                } else {
                    println!("{}", serde_yaml::to_string(&val).unwrap());
                }
            }
            None => cmd::system::config(&active_profile, &cfg_mgr, &cli.format, *all).await?,
        },
        Commands::Reset { dry_run } => cmd::system::reset(&active_profile, Some(vault.as_ref()), &cfg_mgr, Some(cowen_common::events::event_bus()), *dry_run).await?,
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
            DlqCommands::List { page, page_size } => cmd::dlq::list(&active_profile, &config, &cli.format, *page, *page_size, vault.clone()).await?,
            DlqCommands::Retry { id } => cmd::dlq::retry(&active_profile, &config, id.clone(), vault.clone()).await?,
            DlqCommands::Purge => cmd::dlq::purge(&active_profile, &config, vault.clone()).await?,
        }
        Commands::Log { action } => match action {
            LogCommands::List => cmd::log::list(&active_profile, vault.clone()).await?,
            LogCommands::View { domain, follow, lines } => cmd::log::view(&active_profile, domain, *follow, *lines, vault.clone()).await?,
        }
        Commands::Store { .. } => unreachable!(),
        Commands::Version { .. } => unreachable!(),
    }
    Ok(())
}
