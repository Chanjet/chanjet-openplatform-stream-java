pub(crate) mod cmd;

use clap::Parser;

use cowen_common::utils::get_bin_name;
use anyhow::Result;
use std::io::Write;
use std::sync::Arc;
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
    long_about = "畅捷通 (Chanjet) 开放平台官方全流程治理工具。\n\n本工具是连接企业本地业务系统与 畅捷通好业财、T+Cloud、好微、好会计 等云端核心产品的数字支点。它不仅是一个命令行界面，更是为 AI Agent 与自动化管道设计的 零信任安全网关 与 智能接口发现系统。\n\n核心能力 (Core Capabilities):\n- 🧠 意向发现 (api list --search): 内置极轻量 ONNX 神经网络推理引擎，支持通过自然语言实现 API 的语义搜索与精准锁定。\n- 🛡️ 安全编排 (init/auth): 自动化执行 AppTicket/AccessToken 握手解析，托管加密的安全凭据存储 (Vault)，自动注入签名安全头。\n- 🔄 实时流桥 (daemon): 基于 WebSocket 实现的高性能 Streaming Gateway 桥接器，支持在防火墙内安全接收云端消息推送并本地转发。\n- 📊 健壮运维 (dlq/log): 完整的死信队列 (DLQ) 处理机制与多域结构化审计日志，确保每一笔交易与推送均可回溯与自动补试。\n\n🔒 隐私说明: 默认开启的遥测仅用于收集匿名崩溃报告与性能指标，不包含任何业务数据。可通过 --no-telemetry 或全局配置关闭。"
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
        #[arg(long, env = "COWEN_APP_MODE", help = "应用模式: self_built (自建应用), oauth2 (OAuth2应用), store_app (商店应用)")]
        app_mode: Option<String>,
        #[arg(long, env = "COWEN_PROXY_PORT", help = "本地代理监听端口")]
        proxy_port: Option<u16>,
    },
    /// 调用开放平台 API 或管理接口规范
    #[command(long_about = "调用开放平台 API 或管理接口规范。\n\n有两种使用方式:\n1. 直接发起 API 调用: cowen api GET /v1/user\n2. 使用子命令: cowen api list 或 cowen api spec\n\n注意: METHOD 和 PATH 必须成对出现，如果遇到名为 list 的 API 路径，请写为 cowen api GET /list，以免与子命令 list 冲突。")]
    Api {
        #[arg(help = "HTTP Method (e.g. GET, POST)")]
        method: Option<String>,
        #[arg(help = "API Path (e.g. /v1/user)")]
        path: Option<String>,
        #[arg(short = 'd', long = "data", help = "HTTP 请求体数据 (JSON格式)")]
        data: Option<String>,
        #[arg(short = 'f', long = "file", help = "从文件读取请求体数据 (JSON格式)")]
        data_file: Option<String>,
        #[arg(long, help = "绕过 OpenAPI 规约严格校验")]
        force: bool,

        #[command(subcommand)]
        action: Option<ApiCommands>,
    },
    /// 管理身份认证与凭据 (Token/Ticket)
    Auth {
        #[command(subcommand)]
        action: AuthCommands,
    },
    /// 守护进程管理 (Streaming Gateway / 本地代理)
    Daemon {
        #[command(subcommand)]
        action: DaemonCommands,
    },
    /// 检查 CLI 的整体运行状态 (cowen system status 的简写形式)
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

        /// 重置所有环境配置状态与遥测数据
        #[arg(short, long)]
        all: bool,
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
    /// 查看与跟踪系统审计日志
    Audit {
        #[command(subcommand)]
        action: AuditCommands,
    },
    /// 查看并追踪 CLI 运行日志
    Log {
        #[command(subcommand)]
        action: LogCommands,
    },
    /// 管理全局存储后端，部分操作会校验当前 Profile 的兼容性
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
    /// 删除局部配置项 (自动重排索引)
    #[command(long_about = "删除局部配置项。\n\n提示: 若删除 JSON 数组中的特定索引元素，工具会自动重排索引 (Index Reordering)。\n例如: cowen config unset search.plugins.1\n删除后，原本的 plugins[2] 会自动前移到 plugins[1]，保持数组连续性。")]
    Unset {
        #[arg(help = "要删除的配置项路径 (例如 search.plugins.0)")]
        key: String,
    },
    /// 列出当前生效的所有配置项目
    List,
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
    /// 安装插件，将其拷贝到 ~/.cowen/plugins 目录并赋予权限
    Install {
        /// 要安装的插件文件路径 (如 ./libcowen_search_embedding.so)
        path: String,
    },
    /// 刷新插件签名校准状态 (支持本地临时开发者签名或清理 OS 隔离标记)
    RefreshSignature {
        /// 要校准签名的插件名称
        name: String,
    },
}

#[derive(clap::Subcommand)]
pub enum SystemCommands {
    /// 检查系统的整体运行状态指标 (包括 Daemon、Store、Auth、AI 等)。此命令是 cowen status 的全名形式。
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
    #[command(long_about = "在不同的底层存储后端之间安全地迁移已保存的配置与凭据状态。\n\n⚠️  警告: 数据迁移可能会覆盖目标数据库中的同名记录。请在执行前确保目标数据库为空或已备份。\n\n示例: \n  cowen store migrate --to sqlite:data/new.db --mode clone\n  cowen store migrate --to redis://localhost:6379/1")]
    Migrate {
        /// 迁移的目标数据库连接 URL 地址
        #[arg(long, value_name = "URL", long_help = "需要将当前数据迁移到的目标数据库连接 URL 地址。\n\n格式与 Set 命令一致，例如 sqlite:data/new.db")]
        to: String,
        /// 数据迁移的交互工作模式
        #[arg(long, help = "Migration mode (copy, move)")]
        mode: String,
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
    /// 触发与开放平台的换票及交互式 OAuth2 登录流 (支持 Headless / SSH 远程复制 URL 授权)
    #[command(long_about = "触发与开放平台的换票及交互式 OAuth2 登录流。\n\n默认情况下，CLI 会尝试打开系统本地浏览器进行授权。如果处于无桌面/无头服务器环境 (Headless)，可以直接复制控制台打印的鉴权链接，并在您的本地浏览器中完成授权。\n\n【SSH 环境收尾说明】授权完成后浏览器会重定向到 localhost 并可能提示无法访问。您只需将地址栏中被重定向的 URL 完整复制，然后在当前无头服务器中执行 `curl \"<复制的URL>\"` 即可完成最终的换票收尾流程。也可以使用 --manual 参数禁用自动打开浏览器。")]
    Login {
        /// 强制废弃本地 Token 并立即触发重新网络登录
        #[arg(short, long, help = "强制废弃缓存凭据并立即网络重登录")]
        force: bool,
        /// 手动复制登录链接并在本地浏览器授权 (适用于远程 SSH 或 Headless 环境)
        #[arg(short, long, help = "手动复制链接授权 (Headless 无头环境适用)")]
        manual: bool,
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
    /// 从共享存储中强制同步最新凭据数据到当前工作上下文 (常用于多机共享存储时同步远端节点更新的 Token)
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
    /// 查看特定死信事件的详细请求体 (Payload) 和堆栈信息
    #[command(alias = "show")]
    View {
        /// 需要查看的死信事件 ID
        #[arg(help = "死信记录的唯一事件 ID")]
        id: String,
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

#[derive(clap::Subcommand)]
pub enum AuditCommands {
    /// 实时追踪特定环境的关键业务操作与审计日志
    Tail {
        /// 默认在起始位置展示日志审计文件的尾部行数
        #[arg(short = 'n', long, default_value_t = 10, help = "要在尾部默认读取的日志行数")]
        lines: usize,
    },
}

async fn get_all_profiles(active_profile: &str) -> Vec<String> {
    let app_dir = cowen_common::config::get_app_dir();
    let mut profiles = std::collections::HashSet::new();
    profiles.insert("default".to_string());
    if let Ok(entries) = std::fs::read_dir(&app_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map(|s| s == "yaml").unwrap_or(false) {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    if !name.contains("_openapi") && name != "app" {
                        profiles.insert(name.to_string());
                    }
                }
            }
        }
    }

    let port_path = cowen_common::ipc::get_ipc_port_path();
    let ipc = cowen_common::ipc::client::IpcDaemonService::new(port_path);
    if let Ok(cowen_common::ipc::DaemonResponse::SystemStatusData { json }) = ipc.system_status(active_profile, true).await {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json) {
            if let Some(arr) = val.as_array() {
                for item in arr {
                    if let Some(p) = item.get("profile").and_then(|v| v.as_str()) {
                        profiles.insert(p.to_string());
                    }
                }
            }
        }
    }
    
    let mut profiles: Vec<String> = profiles.into_iter().collect();
    profiles.sort();
    profiles
}

pub async fn run(cli: Cli) -> Result<()> {
    let _bin_name = get_bin_name();


        let active_profile = cli.profile.clone().unwrap_or_else(|| {
        let home = std::env::var("COWEN_HOME").unwrap_or_else(|_| {
            if let Some(user_dirs) = directories::UserDirs::new() {
                user_dirs.home_dir().join(".cowen").to_string_lossy().to_string()
            } else {
                ".cowen".to_string()
            }
        });
        std::fs::read_to_string(std::path::Path::new(&home).join("current_profile"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "default".to_string())
    });
    let port_path = cowen_common::ipc::get_ipc_port_path();
    let daemon_svc: Arc<dyn DaemonService> = Arc::new(cowen_common::ipc::client::IpcDaemonService::new(port_path.clone()));

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
        Commands::Audit { .. } => "audit",
        Commands::Log { .. } => "log",
        Commands::Store { .. } => "store",
        Commands::Doctor { .. } => "doctor",
        Commands::Plugins { .. } => "plugins",
        Commands::Events(..) => "events",
        Commands::System { .. } => "system",
        Commands::Version { .. } => "version",
    };

    if let Commands::Version { format } = &cli.command {
        cmd::version::run(format).await?;
        return Ok(());
    }

    if let Commands::Store { action } = &cli.command {
        match action {
            StoreCommands::Set { store, db_url, cache, cache_url } => {
                cmd::store::set(store, db_url, cache, cache_url).await?;
                return Ok(());
            }
            StoreCommands::Status => {
                cmd::store::status().await?;
                return Ok(());
            }
            _ => {} 
        }
    }

    if let Commands::Store { action: StoreCommands::Migrate { to, mode } } = &cli.command {
        cmd::store::migrate(to, mode.clone()).await?;
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
        cmd::system::enforce_daemon_version_sync(&active_profile).await?;
    }

    // 2. Auto-recovery: "确保必要的后台进程正在运行"
    // We skip auto-recovery for lifecycle/management commands to avoid starting a daemon 
    // that the user is explicitly trying to stop, reset or initialize.
    // However, diagnostic commands like 'status' SHOULD trigger recovery to maintain the "always-on" promise.
    let skip_recovery = match cmd_name {
        "daemon" | "reset" | "init" | "config" | "profile" | "dlq" | "log" | "audit" => true,
        "doctor" => !matches!(&cli.command, Commands::Doctor { fix: true, .. }),
        "auth" => matches!(&cli.command, Commands::Auth { action: AuthCommands::Reset }),
        _ => false,
    } || std::env::var("COWEN_SKIP_DAEMON_RECOVERY").is_ok();
    
    if !skip_recovery {
        cmd::system::ensure_daemon_running(&active_profile).await?;
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

            };
            cmd::init::execute(&active_profile, ctx).await?;
        }
        Commands::Api { method, path, data, data_file, force, action } => {
            if let Some(act) = action {
                match act {
                    ApiCommands::List { search, page, page_size, refresh } => {
                        cmd::api::list(&active_profile, search, *page, *page_size, &cli.format, *refresh).await?;
                    }
                    ApiCommands::Spec { method, path, raw } => {
                        cmd::api::spec(&active_profile, method, path, *raw).await?;
                    }
                }
            } else if let (Some(m), Some(p)) = (method, path) {
                let port_path = cowen_common::ipc::get_ipc_port_path();
                let _stream = cowen_common::ipc::client::ensure_daemon(&port_path).await?;
                let daemon_client = cowen_common::ipc::client::IpcDaemonService::new(port_path);

                let body_data = if let Some(file_path) = data_file {
                    Some(std::fs::read_to_string(file_path).map_err(|e| anyhow::anyhow!("Failed to read data file: {}", e))?)
                } else {
                    data.clone()
                };

                let res = daemon_client.call_api(&active_profile, m, p, body_data, *force).await?;
                match res {
                    cowen_common::ipc::DaemonResponse::ApiResponse(dto) => {
                        if cli.format == "json" || cli.format == "yaml" {
                            let mut json_val: serde_json::Value = serde_json::from_str(&dto.body).unwrap_or(serde_json::Value::String(dto.body));
                            if let Some(trace_id) = dto.headers.get("x-b3-traceid")
                                .or_else(|| dto.headers.get("x-msg-id"))
                                .or_else(|| dto.headers.get("msgId"))
                                .or_else(|| dto.headers.get("x-trace-id")) {
                                if let serde_json::Value::Object(ref mut map) = json_val {
                                    map.insert("_trace_id".to_string(), serde_json::Value::String(trace_id.to_string()));
                                }
                            }
                            cowen_common::utils::render(&json_val, &cli.format).map_err(|e| anyhow::anyhow!(e))?;
                        } else {
                            println!("\n🚀 API Response (Status: {})", dto.status);
                            if let Some(trace_id) = dto.headers.get("x-b3-traceid")
                                .or_else(|| dto.headers.get("x-msg-id"))
                                .or_else(|| dto.headers.get("msgId"))
                                .or_else(|| dto.headers.get("x-trace-id")) {
                                println!("\x1b[1;30mTrace ID: {}\x1b[0m", trace_id);
                            }
                            println!("--------------------------------------------------");
                            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&dto.body) {
                                println!("{}", serde_json::to_string_pretty(&json_val).unwrap());
                            } else {
                                println!("{}", dto.body);
                            }
                            println!();
                        }
                    }
                    cowen_common::ipc::DaemonResponse::Error { message, .. } => {
                        return Err(anyhow::anyhow!("API Call failed: {}", message));
                    }
                    _ => return Err(anyhow::anyhow!("Unexpected IPC response")),
                }
            } else {
                println!("Usage: cowen api [METHOD] [PATH] or use subcommands (list, spec)");
            }
        }
        Commands::Auth { action } => match action {
            AuthCommands::Status => cmd::system::status(&active_profile, &cli.format, false).await?,
            AuthCommands::Reset | AuthCommands::Logout => cmd::auth::logout(&active_profile).await?,
            AuthCommands::Login { force, manual, finalize: _ } => {
                if *manual {
                    std::env::set_var("COWEN_SKIP_BROWSER", "true");
                }
                
                cmd::auth::login(&active_profile, *force).await?;
            },
            AuthCommands::Token { refresh } => cmd::auth::token(&active_profile, &cli.format, *refresh).await?,
            AuthCommands::Reload => {
                cmd::auth::token(&active_profile, "text", false).await?;
                
                // Signal running daemon to reload if active
                if let Some(_info) = cowen_common::status::get_active_daemon_info(&active_profile) {

                    let _ = daemon_svc.reload_daemon(&active_profile).await;
                } else {
                    let _ = daemon_svc.reload_daemon(&active_profile).await;
                }
                println!("✅ Token reloaded and synchronized from shared storage.");
            }
        }
        Commands::Daemon { action } => match action {
            DaemonCommands::Start { proxy_port, monitor_port, enable_proxy, no_proxy, foreground, all } => {
                let mut e_opt = None;
                if *enable_proxy { e_opt = Some(true); }
                else if *no_proxy { e_opt = Some(false); }
                
                if let Some(m) = monitor_port {
                    let daemon_client = cowen_common::ipc::client::IpcDaemonService::new(port_path.clone());
                    let _ = daemon_client.set_global_config("monitor_port", &m.to_string()).await;
                }

                cmd::daemon::start(&active_profile, *proxy_port, e_opt, *foreground, *all).await?;
            }
            DaemonCommands::Stop { all } => cmd::daemon::stop(&active_profile, *all).await?,
            DaemonCommands::Restart { proxy_port, enable_proxy, no_proxy, all } => {
                let mut e_opt = None;
                if *enable_proxy { e_opt = Some(true); }
                else if *no_proxy { e_opt = Some(false); }
                
                cmd::daemon::restart(&active_profile, *proxy_port, e_opt, *all).await?;
            }
            DaemonCommands::Reload { all } => {
                if *all {
                    eprintln!("⚠️ Reload all is not supported in Dumb Client mode. Reloading current profile only.");
                }
                
                let _ = daemon_svc.reload_daemon(&active_profile).await;
                println!("✅ Daemon workers reloaded successfully.");
            }
        }
        Commands::Doctor { profile, verbose, fix } => {
            let target_prof = profile.as_deref().unwrap_or(&active_profile);
            cmd::doctor::execute(target_prof, *verbose, *fix).await?;
        }
        Commands::Events(args) => cmd::events::execute(args).await?,
        Commands::Status { all } => cmd::system::status(&active_profile, &cli.format, *all).await?,
        Commands::System { action } => match action { SystemCommands::Status { all } => cmd::system::status(&active_profile, &cli.format, *all).await? },
        Commands::Plugins { action } => match action {
            PluginsCommands::List => cmd::plugins::list().await?,
            PluginsCommands::Enable { name } => cmd::plugins::enable(name).await?,
            PluginsCommands::Disable { name } => cmd::plugins::disable(name).await?,
            PluginsCommands::Install { path } => cmd::plugins::install(path).await?,
            PluginsCommands::RefreshSignature { name } => cmd::plugins::refresh_signature(name).await?,
        },
        Commands::Config { action, all } => match action {
            Some(ConfigCommands::Set { key, value, global: _ }) => {
                let daemon_client = cowen_common::ipc::client::IpcDaemonService::new(port_path.clone());
                match daemon_client.set_config(&active_profile, &key, &value).await? {
                    cowen_common::ipc::DaemonResponse::Success { .. } => {
                        println!("✅ Successfully sent config update to Daemon: '{}' -> '{}'", key, value);
                    }
                    cowen_common::ipc::DaemonResponse::Error { message, .. } => {
                        eprintln!("⚠️ Failed to set config: {}", message);
                        std::process::exit(1);
                    }
                    _ => {
                        eprintln!("⚠️ Unexpected response from daemon");
                        std::process::exit(1);
                    }
                }
            }
            Some(ConfigCommands::Get { key }) => {
                let daemon_client = cowen_common::ipc::client::IpcDaemonService::new(port_path.clone());
                match daemon_client.get_config(&active_profile, &key).await? {
                    cowen_common::ipc::DaemonResponse::ConfigData { config_json } => {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&config_json) {
                            match val {
                                serde_json::Value::Null => {}
                                serde_json::Value::String(s) => {
                                    println!("{}", s);
                                }
                                _ => {
                                    println!("{}", val);
                                }
                            }
                        } else {
                            println!("{}", config_json);
                        }
                    }
                    cowen_common::ipc::DaemonResponse::Error { message, .. } => {
                        eprintln!("⚠️ Failed to get config: {}", message);
                    }
                    _ => eprintln!("⚠️ Unexpected response from daemon"),
                }
            }
            Some(ConfigCommands::Unset { key }) => {
                let daemon_client = cowen_common::ipc::client::IpcDaemonService::new(port_path.clone());
                daemon_client.set_config(&active_profile, key, "").await?;
                println!("✅ Successfully unset '{}'", key);
            }
            Some(ConfigCommands::List) => {
                let list_format = if cli.format == "text" { "yaml" } else { &cli.format };
                cmd::system::config(&active_profile, list_format, *all).await?;
            }
            None => cmd::system::config(&active_profile, &cli.format, *all).await?,
        },
        Commands::Reset { dry_run, all } => {
            let target_profile = if !*all {
                Some(active_profile.clone())
            } else {
                None
            };
            cmd::system::reset(target_profile.as_deref(), *dry_run).await?
        }
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
            ProfileCommands::Use { name } => {
                let profiles = get_all_profiles(&active_profile).await;
                if !profiles.contains(name) {
                    return Err(anyhow::anyhow!("Profile '{}' does not exist", name));
                }
                let default_path = cowen_common::config::get_app_dir().join("current_profile");
                std::fs::write(&default_path, name)?;
                println!("✅ Set default profile to '{}'", name);
            }
            ProfileCommands::Current => {
                let default_path = cowen_common::config::get_app_dir().join("current_profile");
                let p = std::fs::read_to_string(&default_path).unwrap_or_else(|_| "default".to_string());
                println!("{}", p);
            }
            ProfileCommands::List => {
                let profiles = get_all_profiles(&active_profile).await;
                let default_path = cowen_common::config::get_app_dir().join("current_profile");
                let current = std::fs::read_to_string(&default_path).unwrap_or_else(|_| "default".to_string());
                if cli.format == "json" || cli.format == "yaml" { cowen_common::utils::render(&profiles, &cli.format).map_err(|e| anyhow::anyhow!(e))?; }
                else {
                    println!("\n📂 Available Profiles:");
                    for p in profiles { if p == current { println!("  * \x1b[32m{:<20}\x1b[0m (current)", p); } else { println!("    {:<20}", p); } }
                }
            }
            ProfileCommands::Rename { old_name, new_name } => cmd::system::rename_profile(old_name, new_name).await?,
        }
        Commands::Dlq { action } => match action {
            DlqCommands::View { id } => cmd::dlq::view(&active_profile, id.clone()).await?,
            DlqCommands::List { page, page_size } => cmd::dlq::list(&active_profile, &cli.format, *page, *page_size).await?,
            DlqCommands::Retry { id } => cmd::dlq::retry(&active_profile, id.clone()).await?,
            DlqCommands::Purge => cmd::dlq::purge(&active_profile).await?,
        }
        Commands::Log { action } => match action {
            LogCommands::List => cmd::log::list(&active_profile).await?,
            LogCommands::View { domain, follow, lines } => cmd::log::view(&active_profile, domain, *follow, *lines).await?,
        }
        Commands::Audit { action } => match action {
            AuditCommands::Tail { lines } => cmd::audit::tail(&active_profile, *lines).await?,
        }
        Commands::Store { .. } => unreachable!(),
        Commands::Version { .. } => unreachable!(),
    }
    Ok(())
}
