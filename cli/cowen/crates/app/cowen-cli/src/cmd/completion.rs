use anyhow::Context;
use clap::CommandFactory;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::Cli;

pub fn generate_completion(shell: clap_complete::Shell, buf: &mut Vec<u8>) -> anyhow::Result<()> {
    let bin_name = cowen_common::utils::get_bin_name();
    let mut cmd = Cli::command();

    let mut tmp_buf = Vec::new();
    clap_complete::generate(shell, &mut cmd, &bin_name, &mut tmp_buf);

    if shell == clap_complete::Shell::PowerShell {
        // Strip non-ASCII to prevent PowerShell encoding/syntax issues with Chinese descriptions
        let sanitized: Vec<u8> = tmp_buf.into_iter().filter(|&b| b < 128).collect();
        buf.extend_from_slice(&sanitized);
    } else {
        buf.extend_from_slice(&tmp_buf);
    }

    Ok(())
}

fn resolve_shell(
    requested_shell: Option<clap_complete::Shell>,
    bin_name: &str,
) -> anyhow::Result<clap_complete::Shell> {
    if let Some(s) = requested_shell {
        return Ok(s);
    }
    if cfg!(windows) {
        Ok(clap_complete::Shell::PowerShell)
    } else {
        let shell_path = env::var("SHELL").unwrap_or_default();
        if shell_path.ends_with("zsh") {
            Ok(clap_complete::Shell::Zsh)
        } else if shell_path.ends_with("bash") {
            Ok(clap_complete::Shell::Bash)
        } else if shell_path.ends_with("fish") {
            Ok(clap_complete::Shell::Fish)
        } else {
            Err(anyhow::anyhow!("Unsupported or unknown shell ({}). Please specify shell with '{} completion <SHELL> --install'", shell_path, bin_name))
        }
    }
}

fn inject_completion_to_profile(
    shell: clap_complete::Shell,
    script_path: &Path,
) -> anyhow::Result<()> {
    let bin_name = cowen_common::utils::get_bin_name();
    match shell {
        clap_complete::Shell::Zsh => {
            let home = get_home()?;
            append_to_rc(home.join(".zshrc"), script_path, shell);
        }
        clap_complete::Shell::Bash => {
            let home = get_home()?;
            if home.join(".bashrc").exists() {
                append_to_rc(home.join(".bashrc"), script_path, shell);
            } else {
                append_to_rc(home.join(".bash_profile"), script_path, shell);
            }
        }
        clap_complete::Shell::Fish => {
            let home = get_home()?;
            let fish_comp_dir = home.join(".config").join("fish").join("completions");
            fs::create_dir_all(&fish_comp_dir).unwrap_or_default();
            let dest = fish_comp_dir.join(format!("{}.fish", bin_name));
            let _ = fs::copy(script_path, &dest);
            eprintln!("✅ Auto-completion installed to {:?}", dest);
        }
        clap_complete::Shell::PowerShell => {
            install_powershell_completion(script_path)?;
        }
        _ => {}
    }
    Ok(())
}

pub fn install_completion(requested_shell: Option<clap_complete::Shell>) -> anyhow::Result<()> {
    let bin_name = cowen_common::utils::get_bin_name();
    let shell = resolve_shell(requested_shell, &bin_name)?;

    let app_dir = cowen_common::config::get_app_dir();
    let comp_dir = app_dir.join("completions");
    fs::create_dir_all(&comp_dir)?;

    let mut script_buf = Vec::new();
    generate_completion(shell, &mut script_buf)?;

    let script_name = match shell {
        clap_complete::Shell::Zsh => format!("{}.zsh", bin_name),
        clap_complete::Shell::Bash => format!("{}.bash", bin_name),
        clap_complete::Shell::Fish => format!("{}.fish", bin_name),
        clap_complete::Shell::PowerShell => format!("{}.ps1", bin_name),
        _ => format!("{}.sh", bin_name),
    };

    let script_path = comp_dir.join(script_name);
    fs::write(&script_path, script_buf).context("Failed to write completion script")?;

    inject_completion_to_profile(shell, &script_path)?;

    // Mark as installed
    let _ = fs::write(app_dir.join(".completion_installed"), "");

    Ok(())
}

fn get_home() -> anyhow::Result<PathBuf> {
    directories::UserDirs::new()
        .context("Could not find home directory")
        .map(|u| u.home_dir().to_path_buf())
}

fn install_powershell_completion(script_path: &Path) -> anyhow::Result<()> {
    let bin_name = cowen_common::utils::get_bin_name();
    let script_path_str = script_path.display().to_string();

    // We try to find the PowerShell profile
    // PS 5.1: $HOME\Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1
    // PS 7: $HOME\Documents\PowerShell\Microsoft.PowerShell_profile.ps1
    // A safer way is to use the user's documents folder if available.

    let docs = directories::UserDirs::new()
        .and_then(|u| u.document_dir().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| {
            // Fallback to $HOME\Documents
            get_home().unwrap_or_default().join("Documents")
        });

    let profiles = vec![
        docs.join("WindowsPowerShell")
            .join("Microsoft.PowerShell_profile.ps1"),
        docs.join("PowerShell")
            .join("Microsoft.PowerShell_profile.ps1"),
    ];

    let marker = format!("# {} autocomplete", bin_name);
    let source_cmd = format!(
        "\n{}\nif (Test-Path \"{}\") {{ . \"{}\" }}\n",
        marker, script_path_str, script_path_str
    );

    let mut success = false;
    for profile_path in profiles {
        // Create directory if not exists
        if let Some(parent) = profile_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&profile_path)
        {
            if file.write_all(source_cmd.as_bytes()).is_ok() {
                eprintln!(
                    "✅ Auto-completion configuration injected into {:?}",
                    profile_path
                );
                success = true;
            }
        }
    }

    if success {
        eprintln!(
            "\n\x1b[1;33m⚠️  ACTION REQUIRED: Activate completion for your current session\x1b[0m"
        );
        eprintln!("Please restart your PowerShell or run:");
        eprintln!("   \x1b[32m. \"{}\"\x1b[0m", script_path_str);
    } else {
        eprintln!("⚠️ Could not automatically find or write to PowerShell profile.");
        eprintln!("💡 Please manually add the following line to your $PROFILE:");
        eprintln!("   \x1b[32m. \"{}\"\x1b[0m", script_path_str);
    }

    Ok(())
}

fn get_rc_files(home: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut rc_files = vec![
        home.join(".zshrc"),
        home.join(".bashrc"),
        home.join(".bash_profile"),
    ];

    let docs = directories::UserDirs::new()
        .and_then(|u| u.document_dir().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| home.join("Documents"));

    rc_files.push(
        docs.join("WindowsPowerShell")
            .join("Microsoft.PowerShell_profile.ps1"),
    );
    rc_files.push(
        docs.join("PowerShell")
            .join("Microsoft.PowerShell_profile.ps1"),
    );

    Ok(rc_files)
}

fn clean_rc_file(rc_path: &Path, marker: &str) -> anyhow::Result<()> {
    if !rc_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(rc_path)?;
    if !content.contains(marker) {
        return Ok(());
    }

    let lines: Vec<String> = content.lines().map(String::from).collect();
    let mut new_lines = Vec::new();
    let mut skipping = false;

    for line in lines {
        if line.contains(marker) {
            skipping = true;
            continue;
        }
        if skipping {
            if line.trim().is_empty() || line.trim() == "fi" || line.contains("}") {
                skipping = false;
                continue;
            }
            continue;
        }
        new_lines.push(line);
    }

    fs::write(rc_path, new_lines.join("\n"))?;
    eprintln!("✅ Auto-completion removed from {:?}", rc_path);

    Ok(())
}

pub fn uninstall_completion() -> anyhow::Result<()> {
    let bin_name = cowen_common::utils::get_bin_name();
    let app_dir = cowen_common::config::get_app_dir();
    let comp_dir = app_dir.join("completions");

    // 1. Remove completion scripts
    if comp_dir.exists() {
        let _ = fs::remove_dir_all(&comp_dir);
    }

    // 2. Remove marker file
    let _ = fs::remove_file(app_dir.join(".completion_installed"));

    // 3. Clean RC files
    let home = get_home()?;
    let rc_files = get_rc_files(&home)?;
    let marker = format!("# {} autocomplete", bin_name);

    for rc_path in rc_files {
        let _ = clean_rc_file(&rc_path, &marker);
    }

    // 4. Special case for Fish
    let fish_comp = home
        .join(".config")
        .join("fish")
        .join("completions")
        .join(format!("{}.fish", bin_name));
    if fish_comp.exists() {
        let _ = fs::remove_file(&fish_comp);
        eprintln!("✅ Auto-completion removed from {:?}", fish_comp);
    }

    eprintln!("✅ Uninstallation complete.");
    Ok(())
}

pub fn is_auto_install_needed() -> bool {
    let app_dir = cowen_common::config::get_app_dir();
    !app_dir.join(".completion_installed").exists()
}

fn append_to_rc(rc_path: PathBuf, script_path: &Path, shell: clap_complete::Shell) {
    let bin_name = cowen_common::utils::get_bin_name();
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
        format!(
            "\n{}\n[ -f \"{}\" ] && source \"{}\"\n",
            marker, script_path_str, script_path_str
        )
    };

    if rc_path.exists() {
        if let Ok(content) = fs::read_to_string(&rc_path) {
            if content.contains(&script_path_str) {
                eprintln!("✅ Auto-completion already configured in {:?}", rc_path);
                eprintln!(
                    "💡 Run \x1b[32msource {:?}\x1b[0m to refresh your current session.",
                    rc_path
                );
                return;
            }
        }
    }

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&rc_path) {
        if file.write_all(source_cmd.as_bytes()).is_ok() {
            eprintln!(
                "✅ Auto-completion configuration injected into {:?}",
                rc_path
            );

            eprintln!("\n\x1b[1;33m⚠️  ACTION REQUIRED: Activate completion for your current session\x1b[0m");
            eprintln!("Due to shell security restrictions, a child process cannot source files for its parent.");
            eprintln!("To enable completion \x1b[1mNOW\x1b[0m, please run:");
            eprintln!("   \x1b[32msource {:?}\x1b[0m", rc_path);

            let shell_name = match shell {
                clap_complete::Shell::Zsh => "zsh",
                clap_complete::Shell::Bash => "bash",
                clap_complete::Shell::Fish => "fish",
                _ => "zsh",
            };
            eprintln!("\nAlternatively, for instant activation without restarting, run:");
            eprintln!(
                "   \x1b[32meval \"$({} completion {})\"\x1b[0m",
                bin_name, shell_name
            );
        }
    } else {
        eprintln!(
            "⚠️ Could not write to {:?}. Please manually add: {}",
            rc_path,
            source_cmd.trim()
        );
    }
}
