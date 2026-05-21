use anyhow::Result;
use clap::Parser;
use cowen_monitor::telemetry_db::TelemetryDb;
use prettytable::{Table, row, format};
use chrono::{DateTime, Local};

#[derive(Parser, Debug)]
pub struct EventsArgs {
    /// Filter events by profile
    #[arg(short, long)]
    pub profile: Option<String>,
    
    /// Number of events to show
    #[arg(short = 'n', long, default_value = "20")]
    pub limit: i64,
}

pub async fn execute(args: &EventsArgs) -> Result<()> {
    let app_dir = cowen_infra::get_app_dir();
    let db_path = app_dir.join("telemetry.db");
    
    if !db_path.exists() {
        println!("No telemetry data found. Run the daemon to start collecting events.");
        return Ok(());
    }

    let db = TelemetryDb::new(&db_path).await?;
    let events = db.list_events(args.profile.as_deref(), args.limit).await?;

    if events.is_empty() {
        println!("No events found.");
        return Ok(());
    }

    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
    table.set_titles(row!["TIMESTAMP", "PROFILE", "EVENT", "TRANSITION", "DETAILS"]);

    for ev in events {
        let local_time: DateTime<Local> = ev.created_at.into();
        let time_str = local_time.format("%Y-%m-%d %H:%M:%S").to_string();
        
        let transition = match (ev.old_status, ev.new_status) {
            (Some(o), Some(n)) => format!("{} -> {}", o, n),
            (None, Some(n)) => format!("-> {}", n),
            (Some(o), None) => format!("{} ->", o),
            _ => "".to_string(),
        };

        table.add_row(row![
            time_str,
            ev.profile,
            ev.event_type,
            transition,
            ev.details.unwrap_or_default()
        ]);
    }

    table.printstd();
    Ok(())
}
