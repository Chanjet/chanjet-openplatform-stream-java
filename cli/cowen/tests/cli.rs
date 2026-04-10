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
