use assert_cmd::Command;
use serde_json::Value;

#[tokio::test(flavor = "multi_thread")]
async fn test_version_json() {
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.args(["version", "-o", "json"]);
    let out = String::from_utf8_lossy(&cmd.output().unwrap().stdout).to_string();

    let json: Value = serde_json::from_str(&out).expect("Output must be valid JSON");
    assert!(json.get("build_id").is_some(), "Missing build_id");
    assert!(json.get("build_time").is_some(), "Missing build_time");
    assert!(json.get("version").is_some(), "Missing version");
}
