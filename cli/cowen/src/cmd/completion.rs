use anyhow::{Result, Context};
use clap::CommandFactory;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use crate::Cli;

pub fn install_completion(requested_shell: Option<clap_complete::Shell>) -> Result<()> {
    let bin_name = crate::core::utils::get_bin_name();
    let home = directories::UserDirs::new()
        .context("Could not find home directory")?
        .home_dir()
        .to_path_buf();

    let shell = match requested_shell {
        Some(s) => s,
        None => {
            let shell_path = env::var("SHELL").unwrap_or_default();
            if shell_path.ends_with("zsh") {
                clap_complete::Shell::Zsh
            } else if shell_path.ends_with("bash") {
                clap_complete::Shell::Bash
            } else if shell_path.ends_with("fish") {
                clap_complete::Shell::Fish
            } else {
                return Err(anyhow::anyhow!("Unsupported or unknown shell ({}). Please specify shell with '{} completion <SHELL> --install'", shell_path, bin_name));
            }
        }
    };

    let app_dir = crate::core::config::get_app_dir();
    let comp_dir = app_dir.join("completions");
    fs::create_dir_all(&comp_dir)?;

    let mut cmd = Cli::command();
    let mut script_buf = Vec::new();
    clap_complete::generate(shell, &mut cmd, &bin_name, &mut script_buf);

    let script_name = match shell {
        clap_complete::Shell::Zsh => format!("{}.zsh", bin_name),
        clap_complete::Shell::Bash => format!("{}.bash", bin_name),
        clap_complete::Shell::Fish => format!("{}.fish", bin_name),
        _ => format!("{}.sh", bin_name),
    };

    let script_path = comp_dir.join(script_name);
    fs::write(&script_path, script_buf).context("Failed to write completion script")?;

    // Append source command to RC file
    match shell {
        clap_complete::Shell::Zsh => append_to_rc(home.join(".zshrc"), &script_path, shell),
        clap_complete::Shell::Bash => {
            if home.join(".bashrc").exists() {
                append_to_rc(home.join(".bashrc"), &script_path, shell)
            } else {
                append_to_rc(home.join(".bash_profile"), &script_path, shell)
            }
        },
        clap_complete::Shell::Fish => {
            let fish_comp_dir = home.join(".config").join("fish").join("completions");
            fs::create_dir_all(&fish_comp_dir).unwrap_or_default();
            let dest = fish_comp_dir.join(format!("{}.fish", bin_name));
            let _ = fs::copy(&script_path, &dest);
            println!("✅ Auto-completion installed to {:?}", dest);
        },
        _ => {}
    }

    // Mark as installed
    let _ = fs::write(app_dir.join(".completion_installed"), "");

    Ok(())
}

pub fn uninstall_completion() -> Result<()> {
    let bin_name = crate::core::utils::get_bin_name();
    let app_dir = crate::core::config::get_app_dir();
    let comp_dir = app_dir.join("completions");
    
    // 1. Remove completion scripts
    if comp_dir.exists() {
        let _ = fs::remove_dir_all(&comp_dir);
    }

    // 2. Remove marker file
    let _ = fs::remove_file(app_dir.join(".completion_installed"));

    // 3. Clean RC files
    let home = directories::UserDirs::new()
        .context("Could not find home directory")?
        .home_dir()
        .to_path_buf();

    let rc_files = vec![
        home.join(".zshrc"),
        home.join(".bashrc"),
        home.join(".bash_profile"),
    ];

    let marker = format!("# {} autocomplete", bin_name);

    for rc_path in rc_files {
        if rc_path.exists() {
            let content = fs::read_to_string(&rc_path)?;
            if content.contains(&marker) {
                let lines: Vec<String> = content.lines().map(String::from).collect();
                let mut new_lines = Vec::new();
                let mut skipping = false;
                
                for line in lines {
                    if line.contains(&marker) {
                        skipping = true;
                        continue;
                    }
                    if skipping {
                        // The injection block ends with 'fi' for zsh/bash
                        if line.trim() == "fi" {
                            skipping = false;
                            continue;
                        }
                        continue;
                    }
                    new_lines.push(line.to_string());
                }
                
                fs::write(&rc_path, new_lines.join("\n"))?;
                println!("✅ Auto-completion removed from {:?}", rc_path);
            }
        }
    }

    // 4. Special case for Fish
    let fish_comp = home.join(".config").join("fish").join("completions").join(format!("{}.fish", bin_name));
    if fish_comp.exists() {
        let _ = fs::remove_file(&fish_comp);
        println!("✅ Auto-completion removed from {:?}", fish_comp);
    }

    println!("✅ Uninstallation complete.");
    Ok(())
}

pub fn is_auto_install_needed() -> bool {
    let app_dir = crate::core::config::get_app_dir();
    !app_dir.join(".completion_installed").exists()
}

fn append_to_rc(rc_path: PathBuf, script_path: &PathBuf, shell: clap_complete::Shell) {
    let bin_name = crate::core::utils::get_bin_name();
    let script_path_str = script_path.display().to_string();
    let marker = format!("# {} autocomplete", bin_name);

    let source_cmd = if shell == clap_complete::Shell::Zsh {
        format!(
            "\n{}\nif [ -f \"{}\" ]; then\n    type compdef >/dev/null 2>&1 || {{ autoload -Uz compinit; compinit; }}\n    source \"{}\"\n    compdef _{} {} 2>/dev/null\n    compdef _{} ./{} 2>/dev/null\nfi\n",
            marker,
            script_path_str,
            script_path_str,
            bin_name, bin_name,
            bin_name, bin_name
        )
    } else {
        format!("\n{}\n[ -f \"{}\" ] && source \"{}\"\n", marker, script_path_str, script_path_str)
    };
    
    if rc_path.exists() {
        if let Ok(content) = fs::read_to_string(&rc_path) {
            if content.contains(&script_path_str) {
                println!("✅ Auto-completion already configured in {:?}", rc_path);
                println!("💡 Run \x1b[32msource {:?}\x1b[0m to refresh your current session.", rc_path);
                return;
            }
        }
    }

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&rc_path) {
        if file.write_all(source_cmd.as_bytes()).is_ok() {
            println!("✅ Auto-completion configuration injected into {:?}", rc_path);
            
            println!("\n\x1b[1;33m⚠️  ACTION REQUIRED: Activate completion for your current session\x1b[0m");
            println!("Due to shell security restrictions, a child process cannot source files for its parent.");
            println!("To enable completion \x1b[1mNOW\x1b[0m, please run:");
            println!("   \x1b[32msource {:?}\x1b[0m", rc_path);
            
            let shell_name = match shell {
                clap_complete::Shell::Zsh => "zsh",
                clap_complete::Shell::Bash => "bash",
                clap_complete::Shell::Fish => "fish",
                _ => "zsh",
            };
            println!("\nAlternatively, for instant activation without restarting, run:");
            println!("   \x1b[32meval \"$({} completion {})\"\x1b[0m", bin_name, shell_name);
        }
    } else {
        println!("⚠️ Could not write to {:?}. Please manually add: {}", rc_path, source_cmd.trim());
    }
}
