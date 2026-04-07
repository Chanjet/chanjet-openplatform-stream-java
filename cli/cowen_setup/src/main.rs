use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("🚀 Starting cowen installation...");
    
    // Embed the binary
    let bytes = include_bytes!(r#"..\..\..\bin\windows-x64\cowen.exe"#);
    
    let home = std::env::var("USERPROFILE").expect("Failed to find USERPROFILE");
    let install_dir = PathBuf::from(&home).join(".cowen").join("bin");
    
    if !install_dir.exists() {
        fs::create_dir_all(&install_dir).expect("Failed to create install directory");
    }
    
    let dest = install_dir.join("cowen.exe");
    fs::write(&dest, bytes).expect("Failed to write cowen.exe");
    
    println!("✅ Copied cowen.exe to {}", install_dir.display());
    
    // Add to User PATH
    let current_path = get_user_path();
    let install_dir_str = install_dir.to_string_lossy().to_string();
    
    if !current_path.contains(&install_dir_str) {
        println!("Adding {} to User PATH...", install_dir_str);
        let new_path = if current_path.is_empty() {
            install_dir_str
        } else {
            format!("{};{}", current_path, install_dir_str)
        };
        
        let status = Command::new("powershell")
            .args(&["-NoProfile", "-Command", &format!("[Environment]::SetEnvironmentVariable('Path', '{}', 'User')", new_path)])
            .status();
            
        if status.is_ok() && status.unwrap().success() {
            println!("✅ PATH updated successfully.");
        } else {
            println!("⚠️ Failed to update PATH. You may need to add it manually.");
        }
    } else {
        println!("ℹ️ {} is already in PATH.", install_dir_str);
    }
    
    // Add powershell completion
    let doc_dir = std::env::var("USERPROFILE").expect("USERPROFILE").to_string() + r#"\Documents\WindowsPowerShell"#;
    let profile_path = PathBuf::from(&doc_dir).join("Microsoft.PowerShell_profile.ps1");
    
    if !PathBuf::from(&doc_dir).exists() {
        fs::create_dir_all(&doc_dir).ok();
    }
    
    let completion_cmd = "\n# cowen completion\nif (Get-Command cowen -ErrorAction SilentlyContinue) { cowen completion powershell | Out-String | Invoke-Expression }\n";
    
    let profile_content = fs::read_to_string(&profile_path).unwrap_or_default();
    if !profile_content.contains("# cowen completion") {
        println!("⚙️ Setting up PowerShell auto-completion...");
        if let Ok(_) = fs::write(&profile_path, profile_content + completion_cmd) {
            println!("✅ Auto-completion added to your PowerShell profile.");
        }
    }

    println!("\n🎉 Installation complete! Please RESTART your terminal.");
    Command::new("cmd").args(&["/c", "pause"]).status().unwrap();
}

fn get_user_path() -> String {
    let output = Command::new("powershell")
        .args(&["-NoProfile", "-Command", "[Environment]::GetEnvironmentVariable('Path', 'User')"])
        .output();
        
    if let Ok(out) = output {
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    } else {
        "".to_string()
    }
}
