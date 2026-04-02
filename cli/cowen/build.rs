use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-env-changed=DEF_OPENAPI_URL");
    println!("cargo:rerun-if-env-changed=DEF_STREAM_URL");

    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    println!("cargo:rustc-env=BUILD_ID={}", since_the_epoch.as_millis());
}
