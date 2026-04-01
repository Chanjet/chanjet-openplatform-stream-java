use anyhow::{Result, Context};
use clap::CommandFactory;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use crate::Cli;

pub fn install_completion(requested_shell: Option<clap_complete::Shell>) -> Result<()> {
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
                return Err(anyhow::anyhow!("Unsupported or unknown shell ({}). Please specify shell with 'owenc completion <SHELL> --install'", shell_path));
            }
        }
    };

    let app_dir = crate::core::config::get_app_dir();
    let comp_dir = app_dir.join("completions");
    fs::create_dir_all(&comp_dir)?;

    let mut cmd = Cli::command();
    let mut script_buf = Vec::new();
    clap_complete::generate(shell, &mut cmd, "owenc", &mut script_buf);

    let script_name = match shell {
        clap_complete::Shell::Zsh => "owenc.zsh",
        clap_complete::Shell::Bash => "owenc.bash",
        clap_complete::Shell::Fish => "owenc.fish",
        _ => "owenc.sh",
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
            let dest = fish_comp_dir.join("owenc.fish");
            let _ = fs::copy(&script_path, &dest);
            println!("✅ Auto-completion installed to {:?}", dest);
        },
        _ => {}
    }

    Ok(())
}

fn append_to_rc(rc_path: PathBuf, script_path: &PathBuf, shell: clap_complete::Shell) {
    let script_path_str = script_path.display().to_string();
    let source_cmd = if shell == clap_complete::Shell::Zsh {
        format!(
            "\n# owenc autocomplete\nif [ -f \"{}\" ]; then\n    type compdef >/dev/null 2>&1 || {{ autoload -Uz compinit; compinit; }}\n    source \"{}\"\n    compdef _owenc owenc 2>/dev/null\n    compdef _owenc ./owenc 2>/dev/null\nfi\n",
            script_path_str,
            script_path_str
        )
    } else {
        format!("\n# owenc autocomplete\n[ -f \"{}\" ] && source \"{}\"\n", script_path_str, script_path_str)
    };
    
    if rc_path.exists() {
        if let Ok(content) = fs::read_to_string(&rc_path) {
            if content.contains(&script_path.display().to_string()) {
                println!("✅ Auto-completion already configured in {:?}", rc_path);
                return;
            }
        }
    }

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&rc_path) {
        if file.write_all(source_cmd.as_bytes()).is_ok() {
            println!("✅ Auto-completion successfully injected into {:?}", rc_path);
            println!("💡 Please restart your terminal or run: source {:?}", rc_path);
        }
    } else {
        println!("⚠️ Could not write to {:?}. Please manually add: {}", rc_path, source_cmd.trim());
    }
}
