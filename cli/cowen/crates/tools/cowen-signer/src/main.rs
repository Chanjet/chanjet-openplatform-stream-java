use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use cowen_infra::pki::{DeveloperCert, PluginManifest, SignatureBundle};
use ring::signature::{Ed25519KeyPair, KeyPair};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new Root Key Pair (Cold Storage)
    GenerateRoot {
        #[arg(long, help = "Output path for the generated pk8 root private key")]
        out_root_key: PathBuf,
        #[arg(long, help = "Output path for the generated root public key (binary)")]
        out_root_pub: PathBuf,
    },
    /// Issue a new developer certificate
    IssueCert {
        #[arg(long, help = "Path to official root PKCS8 private key")]
        root_key: PathBuf,
        #[arg(long, help = "Developer ID (e.g. 'official-ci')")]
        dev_id: String,
        #[arg(long, help = "Output path for the generated pk8 dev key")]
        out_dev_key: PathBuf,
        #[arg(long, help = "Output path for the generated cert JSON")]
        out_cert: PathBuf,
        #[arg(long, help = "Validity in days", default_value = "365")]
        days: u64,
        #[arg(long, help = "Organization Name", default_value = "")]
        org: String,
        #[arg(long, help = "Country", default_value = "")]
        country: String,
    },
    SignPlugin {
        #[arg(long, help = "Path to the .dylib/.so binary")]
        dylib: PathBuf,
        #[arg(long, help = "Plugin Name")]
        name: String,
        #[arg(long, help = "Plugin Version")]
        version: String,
        #[arg(long, help = "Path to the developer private key")]
        dev_key: PathBuf,
        #[arg(long, help = "Path to the developer certificate JSON")]
        dev_cert: PathBuf,
        #[arg(long, help = "Output path for the signature bundle JSON")]
        out_bundle: PathBuf,
        #[arg(
            long,
            help = "Path to the plugin.json manifest. If provided, capabilities, privileges, transport and contributes are read from it."
        )]
        manifest_file: Option<PathBuf>,
        #[arg(
            long,
            help = "Capabilities/Interfaces provided by the plugin",
            value_delimiter = ','
        )]
        capabilities: Option<Vec<String>>,
        #[arg(
            long,
            help = "Sensitive privileges consumed by the plugin",
            value_delimiter = ','
        )]
        required_privileges: Option<Vec<String>>,
    },
}

fn cmd_generate_root(out_root_key: PathBuf, out_root_pub: PathBuf) -> Result<()> {
    let rng = ring::rand::SystemRandom::new();
    let root_pk8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
    let root_pair = Ed25519KeyPair::from_pkcs8(root_pk8.as_ref()).unwrap();

    std::fs::write(&out_root_key, root_pk8.as_ref())?;
    std::fs::write(&out_root_pub, root_pair.public_key().as_ref())?;

    println!("✅ Root private key generated: {:?}", out_root_key);
    println!("✅ Root public key generated: {:?}", out_root_pub);

    print!("🔑 Please update OFFICIAL_ROOT_PUB_KEY in cowen-infra/src/pki.rs with:\n&[");
    for (i, b) in root_pair.public_key().as_ref().iter().enumerate() {
        if i > 0 {
            print!(", ");
        }
        print!("0x{:02X}", b);
    }
    println!("];\n");
    Ok(())
}

fn cmd_issue_cert(
    root_key: PathBuf,
    dev_id: String,
    out_dev_key: PathBuf,
    out_cert: PathBuf,
    days: u64,
    org: String,
    country: String,
) -> Result<()> {
    let root_bytes = std::fs::read(&root_key).context("Failed to read root key")?;
    let root_pair = Ed25519KeyPair::from_pkcs8(&root_bytes)
        .map_err(|_| anyhow::anyhow!("Invalid root pk8 key"))?;

    // Generate dev key
    let rng = ring::rand::SystemRandom::new();
    let dev_pk8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
    let dev_pair = Ed25519KeyPair::from_pkcs8(dev_pk8.as_ref()).unwrap();

    std::fs::write(&out_dev_key, dev_pk8.as_ref())?;

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let expires_at = now + (days * 24 * 3600);
    let dev_pub_hex = hex::encode(dev_pair.public_key().as_ref());

    let msg = if org.is_empty() && country.is_empty() {
        format!("{}:{}:{}:{}", dev_id, dev_pub_hex, now, expires_at)
    } else {
        format!(
            "{}:{}:{}:{}:{}:{}",
            dev_id, org, country, dev_pub_hex, now, expires_at
        )
    };
    let sig = root_pair.sign(msg.as_bytes());

    let cert = DeveloperCert {
        developer_id: dev_id,
        organization: org,
        country,
        public_key_hex: dev_pub_hex,
        issued_at: now,
        expires_at,
        signature_hex: hex::encode(sig.as_ref()),
    };

    std::fs::write(&out_cert, serde_json::to_string_pretty(&cert)?)?;
    println!("✅ Developer certificate issued: {:?}", out_cert);
    println!("✅ Developer private key generated: {:?}", out_dev_key);
    Ok(())
}

fn apply_manifest_to_plugin_config(
    manifest_file: Option<PathBuf>,
    final_caps: &mut Vec<String>,
    final_privs: &mut Vec<String>,
    final_transport: &mut Option<String>,
    final_contributes: &mut Option<cowen_infra::pki::PluginContributes>,
) -> Result<()> {
    if let Some(mf) = manifest_file {
        let content = std::fs::read_to_string(&mf).context("Failed to read plugin.json")?;
        let parsed: serde_json::Value =
            serde_json::from_str(&content).context("Failed to parse plugin.json")?;

        if let Some(req_caps) = parsed
            .get("required_capabilities")
            .and_then(|v| v.as_object())
        {
            for k in req_caps.keys() {
                if !final_caps.contains(k) {
                    final_caps.push(k.clone());
                }
            }
        }

        if let Some(req_perms) = parsed
            .get("requested_permissions")
            .and_then(|v| v.as_object())
        {
            for k in req_perms.keys() {
                if !final_privs.contains(k) {
                    final_privs.push(k.clone());
                }
            }
        }

        if let Some(t) = parsed.get("transport").and_then(|v| v.as_str()) {
            *final_transport = Some(t.to_string());
        }

        if let Some(c) = parsed.get("contributes") {
            *final_contributes = serde_json::from_value(c.clone()).ok();
        }
    }
    Ok(())
}

fn cmd_sign_plugin(
    dylib: PathBuf,
    name: String,
    version: String,
    dev_key: PathBuf,
    dev_cert: PathBuf,
    out_bundle: PathBuf,
    manifest_file: Option<PathBuf>,
    capabilities: Option<Vec<String>>,
    required_privileges: Option<Vec<String>>,
) -> Result<()> {
    let dev_bytes = std::fs::read(&dev_key).context("Failed to read dev key")?;
    let dev_pair = Ed25519KeyPair::from_pkcs8(&dev_bytes)
        .map_err(|_| anyhow::anyhow!("Invalid dev pk8 key"))?;

    let cert_str = std::fs::read_to_string(&dev_cert)?;
    let cert: DeveloperCert = serde_json::from_str(&cert_str)?;

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    if now > cert.expires_at {
        anyhow::bail!(
            "❌ Developer Certificate has expired! (Expired at: timestamp {})",
            cert.expires_at
        );
    }

    let dylib_bytes = std::fs::read(&dylib).context("Failed to read plugin dylib")?;
    use ring::digest::{Context, SHA256};
    let mut ctx = Context::new(&SHA256);
    ctx.update(&dylib_bytes);
    let hash = hex::encode(ctx.finish().as_ref());

    let mut final_caps = capabilities.unwrap_or_default();
    let mut final_privs = required_privileges.unwrap_or_default();
    let mut final_transport = None;
    let mut final_contributes = None;

    apply_manifest_to_plugin_config(
        manifest_file,
        &mut final_caps,
        &mut final_privs,
        &mut final_transport,
        &mut final_contributes,
    )?;

    let manifest = PluginManifest {
        name,
        version,
        binary_hash: hash,
        transport: final_transport,
        capabilities: final_caps,
        required_privileges: final_privs,
        contributes: final_contributes,
    };

    let manifest_str = serde_json::to_string(&manifest)?;
    let sig = dev_pair.sign(manifest_str.as_bytes());

    let bundle = SignatureBundle {
        cert,
        manifest,
        manifest_signature_hex: hex::encode(sig.as_ref()),
    };

    std::fs::write(&out_bundle, serde_json::to_string_pretty(&bundle)?)?;
    println!("✅ Plugin signed and bundle generated: {:?}", out_bundle);
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::GenerateRoot {
            out_root_key,
            out_root_pub,
        } => {
            cmd_generate_root(out_root_key, out_root_pub)?;
        }
        Commands::IssueCert {
            root_key,
            dev_id,
            out_dev_key,
            out_cert,
            days,
            org,
            country,
        } => {
            cmd_issue_cert(root_key, dev_id, out_dev_key, out_cert, days, org, country)?;
        }
        Commands::SignPlugin {
            dylib,
            name,
            version,
            dev_key,
            dev_cert,
            out_bundle,
            manifest_file,
            capabilities,
            required_privileges,
        } => {
            cmd_sign_plugin(
                dylib,
                name,
                version,
                dev_key,
                dev_cert,
                out_bundle,
                manifest_file,
                capabilities,
                required_privileges,
            )?;
        }
    }
    Ok(())
}
