use assert_cmd::Command;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("Usage:"));
}

#[test]
fn test_cli_version() {
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("cowen"));
}

#[test]
fn test_cli_invalid_command() {
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.arg("nonexistent_command");
    cmd.assert()
        .failure()
        .stderr(predicates::str::contains("unrecognized subcommand"));
}

#[test]
fn test_cli_config_set_global() {
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "1");
    // 动态修改全局配置的局部值应该支持 --global 选项，目前代码中未实现，所以这在 TDD 流程中将是“红灯”（失败测试）
    cmd.args(&["config", "set", "log.level", "debug", "--global"]);
    cmd.assert().success();
}

#[test]
fn test_cli_dlq_list_page_size_short_n() {
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "1");
    // 统一后的 DLQ 列表分页应该支持 -n 缩写（原来仅支持 -s，这在 TDD 中为“红灯”）
    cmd.args(&["dlq", "list", "-n", "5"]);
    cmd.assert().success();
}


