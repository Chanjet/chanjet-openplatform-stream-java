use std::time::{SystemTime, UNIX_EPOCH};
use chrono::{DateTime, Utc, Local};

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-env-changed=DEF_OPENAPI_URL");
    println!("cargo:rerun-if-env-changed=DEF_STREAM_URL");

    let now = SystemTime::now();
    let since_the_epoch = now
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    
    let build_id = since_the_epoch.as_millis();
    
    // Get formatted build time (UTC and Local for clarity)
    let datetime: DateTime<Utc> = now.into();
    let build_time = datetime.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string();

    println!("cargo:rustc-env=BUILD_ID={}", build_id);
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);
}
