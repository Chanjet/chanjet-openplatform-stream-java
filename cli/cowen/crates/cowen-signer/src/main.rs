use clap::{Parser, Subcommand};
use ring::signature::{Ed25519KeyPair, KeyPair};
use std::path::PathBuf;
use anyhow::{Context, Result};
use cowen_infra::pki::{DeveloperCert, PluginManifest, SignatureBundle};
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
    /// Sign a plugin dynamic library
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
        #[arg(long, help = "Permissions/Capabilities", default_value = "SearchProvider", value_delimiter = ',')]
        permissions: Vec<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::GenerateRoot { out_root_key, out_root_pub } => {
            let rng = ring::rand::SystemRandom::new();
            let root_pk8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
            let root_pair = Ed25519KeyPair::from_pkcs8(root_pk8.as_ref()).unwrap();
            
            std::fs::write(&out_root_key, root_pk8.as_ref())?;
            std::fs::write(&out_root_pub, root_pair.public_key().as_ref())?;
            
            println!("✅ Root private key generated: {:?}", out_root_key);
            println!("✅ Root public key generated: {:?}", out_root_pub);
            
            print!("🔑 Please update OFFICIAL_ROOT_PUB_KEY in cowen-infra/src/pki.rs with:\n&[");
            for (i, b) in root_pair.public_key().as_ref().iter().enumerate() {
                if i > 0 { print!(", "); }
                print!("0x{:02X}", b);
            }
            println!("];\n");
        }
        Commands::IssueCert { root_key, dev_id, out_dev_key, out_cert, days, org, country } => {
            let root_bytes = std::fs::read(&root_key).context("Failed to read root key")?;
            let root_pair = Ed25519KeyPair::from_pkcs8(&root_bytes).map_err(|_| anyhow::anyhow!("Invalid root pk8 key"))?;

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
                format!("{}:{}:{}:{}:{}:{}", dev_id, org, country, dev_pub_hex, now, expires_at)
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
        }
        Commands::SignPlugin { dylib, name, version, dev_key, dev_cert, out_bundle, permissions } => {
            let dev_bytes = std::fs::read(&dev_key).context("Failed to read dev key")?;
            let dev_pair = Ed25519KeyPair::from_pkcs8(&dev_bytes).map_err(|_| anyhow::anyhow!("Invalid dev pk8 key"))?;

            let cert_str = std::fs::read_to_string(&dev_cert)?;
            let cert: DeveloperCert = serde_json::from_str(&cert_str)?;

            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            if now > cert.expires_at {
                anyhow::bail!("❌ Developer Certificate has expired! (Expired at: timestamp {})", cert.expires_at);
            }

            let dylib_bytes = std::fs::read(&dylib).context("Failed to read plugin dylib")?;
            use ring::digest::{Context, SHA256};
            let mut ctx = Context::new(&SHA256);
            ctx.update(&dylib_bytes);
            let hash = hex::encode(ctx.finish().as_ref());

            let manifest = PluginManifest {
                name,
                version,
                binary_hash: hash,
                permissions,
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
        }
    }
    Ok(())
}
