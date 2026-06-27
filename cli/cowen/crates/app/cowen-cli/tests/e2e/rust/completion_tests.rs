use assert_cmd::Command;

#[tokio::test(flavor = "multi_thread")]
async fn test_completion_generation() {
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.args(["completion", "zsh"]);
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(!stdout.is_empty(), "Completion script should not be empty");
    assert!(
        stdout.contains("compdef") || stdout.contains("cowen"),
        "Output should look like a zsh completion script"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_completion_install_uninstall() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let home_str = home.to_str().unwrap().to_string();

    let zshrc = home.join(".zshrc");
    std::fs::write(&zshrc, "").unwrap();

    // Install
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("HOME", &home_str);
    cmd.args(["completion", "--install", "zsh"]);
    cmd.assert().success();

    let content_after_install = std::fs::read_to_string(&zshrc).unwrap();
    assert!(
        content_after_install.contains("cowen completion")
            || content_after_install.contains("source"),
        "Completion script should be sourced in .zshrc"
    );

    // Uninstall
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("HOME", &home_str);
    cmd.args(["completion", "--uninstall", "zsh"]);
    cmd.assert().success();

    let content_after_uninstall = std::fs::read_to_string(&zshrc).unwrap();
    assert!(
        !content_after_uninstall.contains("cowen completion"),
        "Completion script should be removed from .zshrc"
    );
}
