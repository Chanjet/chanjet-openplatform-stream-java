use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about = "Cowen MCP Plugin")]
pub struct Cli {
    #[arg(short, long, env = "COWEN_PROFILE", default_value = "default")]
    pub profile: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 启动 MCP Server (标准 stdio 交互模式)
    Server,

    /// 获取连接此 MCP 插件的 stdio 配置 JSON，用于配置 Cursor 等 IDE
    Config,
}
