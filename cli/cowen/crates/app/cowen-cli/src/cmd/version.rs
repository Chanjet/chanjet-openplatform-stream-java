use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
struct VersionInfo {
    build_id: String,
    build_time: String,
    version: String,
}

pub async fn run(format: &str) -> Result<()> {
    let info = VersionInfo {
        build_id: env!("COWEN_BUILD_ID").to_string(),
        build_time: env!("COWEN_BUILD_TIME").to_string(),
        version: env!("COWEN_VERSION").to_string(),
    };

    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("Cowen CLI Version Info");
        println!("----------------------");
        println!("Version:    {}", info.version);
        println!("Build ID:   {}", info.build_id);
        println!("Build Time: {}", info.build_time);
    }

    Ok(())
}
