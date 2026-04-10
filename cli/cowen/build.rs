use chrono::{DateTime, Local, Utc};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-env-changed=DEF_OPENAPI_URL");
    println!("cargo:rerun-if-env-changed=DEF_STREAM_URL");

    let now = SystemTime::now();
    let since_the_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

    let build_id = since_the_epoch.as_millis();

    // Get formatted build time (UTC and Local for clarity)
    let datetime: DateTime<Utc> = now.into();
    let build_time = datetime
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    // Get Git Commit ID
    let git_hash = std::process::Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=BUILD_ID={}", build_id);
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);

    // Provide env vars for config if not present (Option A: Forced env injection)
    let openapi_url = std::env::var("DEF_OPENAPI_URL").unwrap_or_else(|_| "https://openapi.chanjet.com".to_string());
    let stream_url = std::env::var("DEF_STREAM_URL").unwrap_or_else(|_| "https://stream-open.chanapp.chanjet.com".to_string());
    println!("cargo:rustc-env=DEF_OPENAPI_URL={}", openapi_url);
    println!("cargo:rustc-env=DEF_STREAM_URL={}", stream_url);
}
