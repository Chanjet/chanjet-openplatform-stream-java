use chrono::{DateTime, Local, Utc};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=build.rs");
    // Ensure we rebuild when git HEAD changes to keep BUILD_ID fresh
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");

    println!("cargo:rerun-if-env-changed=DEF_OPENAPI_URL");
    println!("cargo:rerun-if-env-changed=DEF_STREAM_URL");
    println!("cargo:rerun-if-env-changed=DEF_MARKET_URL");
    println!("cargo:rerun-if-env-changed=BUILTIN_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=COWEN_BUILD_CLIENT_ID");

    let now = SystemTime::now();
    let since_the_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

    // Get formatted build time (UTC and Local for clarity)
    let datetime: DateTime<Utc> = now.into();
    let build_time = datetime
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    // Get Git Commit ID
    let git_hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let build_id = if git_hash != "unknown" {
        git_hash.clone()
    } else {
        since_the_epoch.as_millis().to_string()
    };

    let cowen_build_id = std::env::var("COWEN_BUILD_ID").unwrap_or(build_id);
    let cowen_build_time = std::env::var("COWEN_BUILD_TIME").unwrap_or(build_time);
    let cowen_version =
        std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "unknown".to_string());

    println!("cargo:rustc-env=COWEN_BUILD_ID={}", cowen_build_id);
    println!("cargo:rustc-env=COWEN_BUILD_TIME={}", cowen_build_time);
    println!("cargo:rustc-env=COWEN_VERSION={}", cowen_version);
    println!("cargo:rustc-env=BUILD_ID={}", cowen_build_id); // keep for backward compatibility
    println!("cargo:rustc-env=BUILD_TIME={}", cowen_build_time); // keep for backward compatibility
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);

    // Provide env vars for config if not present (Option A: Forced env injection)
    let openapi_url = std::env::var("DEF_OPENAPI_URL")
        .unwrap_or_else(|_| "https://openapi.chanjet.com".to_string());
    let stream_url = std::env::var("DEF_STREAM_URL")
        .unwrap_or_else(|_| "https://stream-open.chanapp.chanjet.com".to_string());
    let market_url = std::env::var("DEF_MARKET_URL")
        .unwrap_or_else(|_| "https://market.chanjet.com".to_string());

    let builtin_client_id = std::env::var("COWEN_BUILD_CLIENT_ID")
        .or_else(|_| std::env::var("BUILTIN_CLIENT_ID"))
        .unwrap_or_else(|_| {
            let profile = std::env::var("PROFILE").unwrap_or_default();
            if profile == "release" {
                // Enforce the requirement from LLD for release builds
                panic!("FATAL: Missing mandatory build-time variable COWEN_BUILD_CLIENT_ID");
            } else {
                "dummy-client-id".to_string()
            }
        });

    println!("cargo:rustc-env=DEF_OPENAPI_URL={}", openapi_url);
    println!("cargo:rustc-env=DEF_STREAM_URL={}", stream_url);
    println!("cargo:rustc-env=DEF_MARKET_URL={}", market_url);
    println!("cargo:rustc-env=BUILTIN_CLIENT_ID={}", builtin_client_id);
    println!(
        "cargo:rustc-env=COWEN_BUILD_CLIENT_ID={}",
        builtin_client_id
    );
}
