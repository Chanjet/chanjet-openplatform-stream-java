use anyhow::Result;

use cowen_common::config::get_app_dir;
use cowen_common::grpc::client::DaemonClient;
use cowen_common::grpc::proto::TunnelPluginRequest;
use std::fs;
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_stream::wrappers::ReceiverStream;

fn load_enabled_plugins() -> (std::path::PathBuf, serde_yaml::Value, Vec<String>) {
    let app_yaml_path = get_app_dir().join("app.yaml");
    let content = std::fs::read_to_string(&app_yaml_path).unwrap_or_else(|_| "{}".to_string());
    let val = serde_yaml::from_str::<serde_yaml::Value>(&content)
        .unwrap_or_else(|_| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));

    let enabled_plugins = val
        .get("plugins")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    (app_yaml_path, val, enabled_plugins)
}

fn save_enabled_plugins(
    app_yaml_path: &std::path::Path,
    mut val: serde_yaml::Value,
    enabled_plugins: Vec<String>,
) -> Result<()> {
    if let serde_yaml::Value::Mapping(ref mut map) = val {
        let seq = enabled_plugins
            .into_iter()
            .map(serde_yaml::Value::String)
            .collect();
        map.insert(
            serde_yaml::Value::String("plugins".to_string()),
            serde_yaml::Value::Sequence(seq),
        );
    }
    std::fs::write(app_yaml_path, serde_yaml::to_string(&val)?)?;
    Ok(())
}

fn get_enabled_plugins() -> Vec<String> {
    let (_, _, plugins) = load_enabled_plugins();
    plugins
}

fn parse_capabilities_and_transport(m: &serde_json::Value, display_trait: &mut String) {
    if let Some(req_caps) = m.get("required_capabilities").and_then(|c| c.as_object()) {
        let caps: Vec<String> = req_caps.keys().map(|k| k.to_string()).collect();
        if !caps.is_empty() {
            *display_trait = format!("Req: {}", caps.join(", "));
        }
    }
    if let Some(transport) = m.get("transport").and_then(|t| t.as_str()) {
        if display_trait.starts_with("unknown") {
            *display_trait = format!("Transport: {}", transport);
        }
    }
}

fn parse_contributes(m: &serde_json::Value, display_trait: &mut String) {
    if let Some(contrib) = m.get("contributes").and_then(|c| c.as_object()) {
        let mut parts = vec![];
        if let Some(cmds) = contrib.get("cli_commands").and_then(|a| a.as_array()) {
            if !cmds.is_empty() {
                parts.push(format!("{} cmds", cmds.len()));
            }
        }
        if let Some(provs) = contrib.get("providers").and_then(|a| a.as_array()) {
            for p in provs {
                if let Some(t) = p.get("type").and_then(|v| v.as_str()) {
                    parts.push(format!("Provider:{}", t));
                }
            }
        }
        if !parts.is_empty() {
            *display_trait = parts.join(", ");
        }
    }
}

fn get_required_privileges(m: &serde_json::Value) -> Vec<String> {
    let mut scopes = vec![];
    if let Some(perms) = m.get("requested_permissions").and_then(|p| p.as_object()) {
        for (k, v) in perms {
            if v.as_bool().unwrap_or(false) {
                scopes.push(k.clone());
            }
        }
    } else if let Some(privs) = m.get("required_privileges").and_then(|p| p.as_array()) {
        for p in privs {
            if let Some(s) = p.as_str() {
                scopes.push(s.to_string());
            }
        }
    }
    scopes
}

fn parse_bundle_metadata(bundle_path: &std::path::Path) -> (String, String) {
    let mut display_trait = "unknown (Thin CLI)".to_string();
    let mut display_desc = "Inspected by daemon".to_string();

    if bundle_path.exists() {
        if let Ok(bundle_str) = std::fs::read_to_string(bundle_path) {
            if let Ok(bundle) = serde_json::from_str::<serde_json::Value>(&bundle_str) {
                if let Some(m) = bundle.get("manifest") {
                    parse_capabilities_and_transport(m, &mut display_trait);
                    parse_contributes(m, &mut display_trait);

                    if let Some(version) = m.get("version").and_then(|v| v.as_str()) {
                        display_desc = format!("v{} (Signed)", version);
                    } else {
                        display_desc = "Signed bundle".to_string();
                    }

                    let scopes = get_required_privileges(m);
                    if !scopes.is_empty() {
                        display_desc = format!("{} | Priv: {}", display_desc, scopes.join(","));
                    }
                }
            }
        }
    }

    (display_trait, display_desc)
}

pub async fn list() -> Result<()> {
    let plugins_dir = get_app_dir().join("plugins");

    let enabled_plugins = get_enabled_plugins();

    println!("🔍 Scanning plugins directory: {:?}", plugins_dir);
    println!(
        "{:<30} | {:<20} | {:<10} | DESCRIPTION",
        "NAME", "CONTRIBUTES", "ENABLED"
    );
    println!("{:-<30}-+-{:-<20}-+-{:-<10}-+-{:-<40}", "", "", "", "");

    if !plugins_dir.exists() {
        println!("(No plugins directory found)");
        return Ok(());
    }

    let mut found_any = false;
    let supported_exts = if cfg!(target_os = "windows") {
        vec!["exe"]
    } else {
        vec![""]
    };

    for entry in fs::read_dir(plugins_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if supported_exts.contains(&ext) {
                found_any = true;
                let file_name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");

                let bundle_path = path.with_extension("bundle");
                let (display_trait, display_desc) = parse_bundle_metadata(&bundle_path);

                let name = file_name;
                let is_enabled = enabled_plugins.contains(&name.to_string());
                let enabled_str = if is_enabled {
                    "\x1b[32mYes\x1b[0m"
                } else {
                    "\x1b[31mNo\x1b[0m"
                };

                println!(
                    "{:<30} | {:<20} | {:<23} | {}",
                    name, display_trait, enabled_str, display_desc
                );
            }
        }
    }

    if found_any {
        println!("\n💡 CONTRIBUTES indicates what the plugin extends (e.g., Provider:SearchEmbedding, 2 cmds).");
    } else {
        println!("(No executable plugins found)");
    }

    Ok(())
}

pub async fn enable(name: &String) -> Result<()> {
    let plugins_dir = get_app_dir().join("plugins");

    let expected_path = if cfg!(target_os = "windows") {
        plugins_dir.join(format!("{}.exe", name))
    } else {
        plugins_dir.join(name)
    };

    if expected_path.exists() {
        let port_path = crate::get_ipc_port_path();
        let _ipc = cowen_common::grpc::client::DaemonClient::new(port_path);

        let (app_yaml_path, val, mut enabled_plugins) = load_enabled_plugins();
        if !enabled_plugins.contains(name) {
            enabled_plugins.push(name.to_string());
            save_enabled_plugins(&app_yaml_path, val, enabled_plugins)?;
            println!("✅ Enabled plugin '{}'.", name);
            println!(
                "🚀 Plugin configuration updated. Restart daemon to take effect if necessary."
            );
        } else {
            println!("ℹ️ Plugin '{}' is already enabled.", name);
        }
    } else {
        println!("❌ Plugin file for '{}' not found in {:?} (Ensure the exact filename without extension is provided)", name, plugins_dir);
    }

    Ok(())
}

pub async fn disable(name: &String) -> Result<()> {
    let (app_yaml_path, val, mut enabled_plugins) = load_enabled_plugins();
    if enabled_plugins.contains(name) {
        enabled_plugins.retain(|n| n != name);
        save_enabled_plugins(&app_yaml_path, val, enabled_plugins)?;
        println!("✅ Disabled plugin '{}'.", name);
        println!("🚀 Plugin configuration updated. Restart daemon to take effect if necessary.");
    } else {
        println!("ℹ️ Plugin '{}' is not currently enabled.", name);
    }

    Ok(())
}

fn prompt_for_permissions(required_privs: &[String]) -> Result<()> {
    if required_privs.is_empty() {
        return Ok(());
    }

    println!("⚠️  WARNING: This plugin requests the following sensitive permissions:");
    for p in required_privs {
        println!("  - \x1b[31m{}\x1b[0m", p);
    }
    use std::io::IsTerminal;
    if io::stdin().is_terminal() {
        print!("Do you want to grant these permissions and install? (y/N): ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() != "y" {
            return Err(anyhow::anyhow!("Installation aborted by user."));
        }
    } else {
        println!("(Non-interactive mode: auto-accepting permissions)");
    }
    Ok(())
}

fn copy_plugin_and_bundle(
    source_path: &std::path::Path,
    target_path: &std::path::Path,
    bundle_source_path: &std::path::Path,
    plugins_dir: &std::path::Path,
) -> Result<()> {
    std::fs::copy(source_path, target_path)?;

    #[cfg(unix)]
    {
        let mut perms = fs::metadata(target_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(target_path, perms)?;
    }

    if bundle_source_path.exists() && bundle_source_path.is_file() {
        let bundle_file_name = bundle_source_path.file_name().unwrap();
        let bundle_target_path = plugins_dir.join(bundle_file_name);
        std::fs::copy(bundle_source_path, &bundle_target_path)?;
        println!(
            "✅ Automatically copied signature bundle: {}",
            bundle_file_name.to_string_lossy()
        );
    } else {
        println!("⚠️  Warning: No signature bundle (.bundle) found alongside the plugin. It may fail to load due to security policy.");
    }
    Ok(())
}

pub async fn install(path: &String) -> Result<()> {
    let source_path = std::path::Path::new(path);
    if !source_path.exists() || !source_path.is_file() {
        return Err(anyhow::anyhow!(
            "❌ Source plugin file not found or is not a file: {}",
            path
        ));
    }

    let file_name = source_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?;
    let plugins_dir = get_app_dir().join("plugins");

    if !plugins_dir.exists() {
        std::fs::create_dir_all(&plugins_dir)?;
    }

    let bundle_source_path = source_path.with_extension("bundle");

    let mut required_privs = vec![];
    if bundle_source_path.exists() {
        if let Ok(bundle_str) = std::fs::read_to_string(&bundle_source_path) {
            if let Ok(bundle) = serde_json::from_str::<serde_json::Value>(&bundle_str) {
                if let Some(m) = bundle.get("manifest") {
                    required_privs = get_required_privileges(m);
                }
            }
        }
    }

    prompt_for_permissions(&required_privs)?;

    let target_path = plugins_dir.join(file_name);
    copy_plugin_and_bundle(source_path, &target_path, &bundle_source_path, &plugins_dir)?;

    println!(
        "✅ Successfully installed plugin '{}' to {:?}",
        file_name.to_string_lossy(),
        plugins_dir
    );
    println!(
        "💡 Use 'cowen plugins list' to view it, and 'cowen plugins enable <name>' to activate it."
    );

    Ok(())
}

pub async fn refresh_signature(_name: &String) -> Result<()> {
    println!("⚠️ Signature verification and refresh is delegated to cowen-daemon in the thin CLI architecture.");
    println!("Please refer to daemon logs for validation status during startup.");
    Ok(())
}

fn get_plugin_transport(path: &std::path::Path) -> Option<String> {
    if let Ok(content) = std::fs::read_to_string(path.with_extension("bundle")) {
        if let Ok(bundle) = serde_json::from_str::<serde_json::Value>(&content) {
            return bundle
                .get("manifest")
                .and_then(|m| m.get("transport"))
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());
        }
    }
    None
}

fn list_stdio_plugins(plugins_dir: &std::path::Path) -> Result<()> {
    println!("The following installed plugins implement 'stdio' transport (MCP servers):\n");
    println!("{:<30} | TRANSPORT", "NAME");
    println!("{:-<30}-+-{:-<30}", "", "");

    for entry in (std::fs::read_dir(plugins_dir)?).flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_none() {
            let transport = get_plugin_transport(&path);
            if transport.as_deref() == Some("stdio") {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                println!("{:<30} | stdio", name);
            }
        }
    }
    Ok(())
}

async fn run_stdio_plugin(
    name: &str,
    expected_path: &std::path::Path,
    profile: &str,
    args: &[String],
) -> Result<()> {
    let transport = get_plugin_transport(expected_path);
    if transport.as_deref() != Some("stdio") {
        return Err(anyhow::anyhow!("❌ Permission Denied: Plugin '{}' does not declare 'stdio' transport in its metadata. Only MCP plugins can be directly run.", name));
    }

    let port_path = crate::get_ipc_port_path();
    let daemon_client = DaemonClient::new(&port_path);
    let (channel, interceptor) = daemon_client.ensure_daemon().await?;
    let mut client = cowen_common::grpc::proto::native_system_service_client::NativeSystemServiceClient::with_interceptor(channel, interceptor);

    let (tx, rx) = tokio::sync::mpsc::channel(100);

    let mut envs = std::collections::HashMap::new();
    envs.insert("COWEN_PROFILE".to_string(), profile.to_string());

    tx.send(TunnelPluginRequest {
        plugin_name: Some(name.to_string()),
        stdin_payload: None,
        args: args.to_vec(),
        envs,
    })
    .await
    .unwrap();

    let tx_in = tx.clone();
    tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        let mut buf = vec![0u8; 8192];
        loop {
            match stdin.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if tx_in
                        .send(TunnelPluginRequest {
                            plugin_name: None,
                            stdin_payload: Some(buf[..n].to_vec()),
                            args: vec![],
                            envs: std::collections::HashMap::new(),
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    drop(tx);

    let mut stream = client
        .tunnel_plugin(tonic::Request::new(ReceiverStream::new(rx)))
        .await?
        .into_inner();

    let mut stdout = tokio::io::stdout();
    let mut stderr = tokio::io::stderr();

    while let Ok(Some(resp)) = stream.message().await {
        if let Some(err) = resp.error_message {
            eprintln!("Daemon Error: {}", err);
            break;
        }
        if let Some(out) = resp.stdout_payload {
            let _ = stdout.write_all(&out).await;
            let _ = stdout.flush().await;
        }
        if let Some(err) = resp.stderr_payload {
            let _ = stderr.write_all(&err).await;
            let _ = stderr.flush().await;
        }
    }

    Ok(())
}

pub async fn run(profile: &str, name_opt: &Option<String>, args: &[String]) -> Result<()> {
    let plugins_dir = get_app_dir().join("plugins");

    if !plugins_dir.exists() {
        return Err(anyhow::anyhow!(
            "❌ Plugins directory not found at {:?}",
            plugins_dir
        ));
    }

    if let Some(name) = name_opt {
        let expected_path = if cfg!(target_os = "windows") {
            plugins_dir.join(format!("{}.exe", name))
        } else {
            plugins_dir.join(name)
        };

        if !expected_path.exists() {
            return Err(anyhow::anyhow!(
                "❌ Plugin executable '{}' not found at {:?}",
                name,
                expected_path
            ));
        }

        run_stdio_plugin(name, &expected_path, profile, args).await?;
    } else {
        list_stdio_plugins(&plugins_dir)?;
    }

    Ok(())
}
