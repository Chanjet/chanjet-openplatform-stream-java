use anyhow::Result;

#[derive(clap::Args, Debug)]
pub struct EventsArgs {
    /// 按 Profile 过滤事件
    #[arg(short, long)]
    pub profile: Option<String>,

    /// 显示的事件条数限制
    #[arg(short = 'n', long, default_value = "20")]
    pub limit: i64,
}

pub async fn execute(_args: &EventsArgs) -> Result<()> {
    Err(anyhow::anyhow!("Events DB viewer is moved to Daemon via IPC (or use 'cowen log view') in the new thin CLI architecture."))
}
