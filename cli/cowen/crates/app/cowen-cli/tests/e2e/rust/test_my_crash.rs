#[test]
fn test_my_crash() {
    let mut cmd = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd.arg("daemon").arg("start").arg("--foreground");
    let output = cmd.output().unwrap();
    println!("STDOUT: {}", String::from_utf8_lossy(&output.stdout));
    println!("STDERR: {}", String::from_utf8_lossy(&output.stderr));
}
