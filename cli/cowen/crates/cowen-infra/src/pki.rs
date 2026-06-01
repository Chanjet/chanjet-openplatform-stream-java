use ring::signature::{self, UnparsedPublicKey};
use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::path::Path;

pub const OFFICIAL_ROOT_PUB_KEY: &[u8] = include_bytes!("../../../dist_assets/keys/root_public.bin");

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeveloperCert {
    pub developer_id: String,
    #[serde(default)]
    pub organization: String,
    #[serde(default)]
    pub country: String,
    #[serde(rename = "pub")]
    pub public_key_hex: String,
    pub issued_at: u64,
    pub expires_at: u64,
    #[serde(rename = "sig")]
    pub signature_hex: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub binary_hash: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_privileges: Vec<String>,
}

impl PluginManifest {
    pub fn normalize(&mut self) {
        if self.capabilities.is_empty() && self.required_privileges.is_empty() && !self.permissions.is_empty() {
            // Backward compatibility mapping
            for p in &self.permissions {
                if p == "SearchProvider" || p == "AuthProvider" || p == "StorageProvider" {
                    self.capabilities.push(p.clone());
                } else {
                    self.required_privileges.push(p.clone());
                }
            }
            // Auto-enrich old plugins
            if self.capabilities.iter().any(|c| c == "SearchProvider") {
                if !self.required_privileges.iter().any(|r| r == "LocalCacheAccess") {
                    self.required_privileges.push("LocalCacheAccess".to_string());
                }
                if !self.required_privileges.iter().any(|r| r == "ModelAssetFetch") {
                    self.required_privileges.push("ModelAssetFetch".to_string());
                }
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignatureBundle {
    pub cert: DeveloperCert,
    pub manifest: PluginManifest,
    #[serde(rename = "m_sig")]
    pub manifest_signature_hex: String,
}

pub fn verify_signature(pub_key: &[u8], msg: &[u8], sig_hex: &str) -> Result<()> {
    let sig_bytes = hex::decode(sig_hex).map_err(|e| anyhow::anyhow!("Invalid hex signature: {}", e))?;
    let public_key = UnparsedPublicKey::new(&signature::ED25519, pub_key);
    public_key.verify(msg, &sig_bytes).map_err(|_| anyhow::anyhow!("Signature verification failed"))?;
    Ok(())
}

pub fn verify_plugin_bundle(dylib_path: &Path) -> Result<()> {
    verify_plugin_bundle_with_root(dylib_path, OFFICIAL_ROOT_PUB_KEY)
}

pub fn verify_plugin_bundle_with_root(dylib_path: &Path, root_pub_key: &[u8]) -> Result<()> {
    if std::env::var("COWEN_DEV_MODE").unwrap_or_default() == "1" {
        tracing::warn!("⚠️ COWEN_DEV_MODE IS ENABLED. BYPASSING PKI VERIFICATION FOR: {:?}", dylib_path);
        return Ok(());
    }

    let bundle_path = dylib_path.with_extension("bundle");
    if !bundle_path.exists() {
        return Err(anyhow::anyhow!("Missing signature bundle: {:?}", bundle_path));
    }

    let bundle_str = std::fs::read_to_string(&bundle_path)?;
    let mut bundle: SignatureBundle = serde_json::from_str(&bundle_str)?;

    // 1. Verify DeveloperCert using Root Key
    let cert_msg = if bundle.cert.organization.is_empty() && bundle.cert.country.is_empty() {
        format!("{}:{}:{}:{}", bundle.cert.developer_id, bundle.cert.public_key_hex, bundle.cert.issued_at, bundle.cert.expires_at)
    } else {
        format!("{}:{}:{}:{}:{}:{}", bundle.cert.developer_id, bundle.cert.organization, bundle.cert.country, bundle.cert.public_key_hex, bundle.cert.issued_at, bundle.cert.expires_at)
    };
    verify_signature(root_pub_key, cert_msg.as_bytes(), &bundle.cert.signature_hex)
        .map_err(|e| anyhow::anyhow!("Invalid Developer Certificate (Root validation failed): {}", e))?;

    // 1.5 Verify Certificate is not in Revoked List (CRL)
    const REVOKED_DEV_KEYS: &[&str] = &[
        // e.g., "aabbccddeeff..." // Add compromised developer public_key_hex here
    ];
    if REVOKED_DEV_KEYS.contains(&bundle.cert.public_key_hex.as_str()) {
        return Err(anyhow::anyhow!("❌ FATAL: This developer certificate has been revoked due to security reasons."));
    }

    // 2. Verify Manifest using Developer Key
    let dev_pub_key = hex::decode(&bundle.cert.public_key_hex)?;
    let manifest_str = serde_json::to_string(&bundle.manifest)?;
    verify_signature(&dev_pub_key, manifest_str.as_bytes(), &bundle.manifest_signature_hex)
        .map_err(|e| anyhow::anyhow!("Invalid Plugin Manifest signature: {}", e))?;

    // Normalize capabilities and required_privileges after verifying the signature
    bundle.manifest.normalize();

    // 3. Verify Dylib Hash
    use ring::digest::{Context, SHA256};
    let dylib_bytes = std::fs::read(dylib_path)?;
    let mut ctx = Context::new(&SHA256);
    ctx.update(&dylib_bytes);
    let hash = ctx.finish();
    let hash_hex = hex::encode(hash.as_ref());

    if hash_hex != bundle.manifest.binary_hash {
        return Err(anyhow::anyhow!("Binary hash mismatch! Expected {}, got {}", bundle.manifest.binary_hash, hash_hex));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{Ed25519KeyPair, KeyPair};
    use std::fs;
    use tempfile::tempdir;

    fn generate_keys() -> (Ed25519KeyPair, Ed25519KeyPair) {
        let rng = ring::rand::SystemRandom::new();
        let root_pk8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let root_pair = Ed25519KeyPair::from_pkcs8(root_pk8.as_ref()).unwrap();
        
        let dev_pk8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let dev_pair = Ed25519KeyPair::from_pkcs8(dev_pk8.as_ref()).unwrap();
        
        (root_pair, dev_pair)
    }

    #[test]
    fn test_valid_plugin_signature() {
        let (root_pair, dev_pair) = generate_keys();
        let root_pub = root_pair.public_key().as_ref();
        
        // 1. Create a fake dylib
        let dir = tempdir().unwrap();
        let dylib_path = dir.path().join("fake_plugin.dylib");
        let fake_bin = b"dummy_binary_data";
        fs::write(&dylib_path, fake_bin).unwrap();
        
        // 2. Issue dev cert
        let issued_at = 1000;
        let expires_at = 2000;
        let dev_pub_hex = hex::encode(dev_pair.public_key().as_ref());
        let cert_msg = format!("dev1:{}:{}:{}", dev_pub_hex, issued_at, expires_at);
        let cert_sig = root_pair.sign(cert_msg.as_bytes());
        
        let cert = DeveloperCert {
            developer_id: "dev1".to_string(),
            organization: "".to_string(),
            country: "".to_string(),
            public_key_hex: dev_pub_hex,
            issued_at,
            expires_at,
            signature_hex: hex::encode(cert_sig.as_ref()),
        };
        
        // 3. Create manifest and sign it
        use ring::digest::{Context, SHA256};
        let mut ctx = Context::new(&SHA256);
        ctx.update(fake_bin);
        let hash_hex = hex::encode(ctx.finish().as_ref());
        
        let manifest = PluginManifest {
            name: "fake".to_string(),
            version: "1.0".to_string(),
            binary_hash: hash_hex,
            permissions: vec!["SearchProvider".to_string()],
            capabilities: vec![],
            required_privileges: vec![],
        };
        let manifest_str = serde_json::to_string(&manifest).unwrap();
        let manifest_sig = dev_pair.sign(manifest_str.as_bytes());
        
        let bundle = SignatureBundle {
            cert,
            manifest,
            manifest_signature_hex: hex::encode(manifest_sig.as_ref()),
        };
        
        let bundle_path = dir.path().join("fake_plugin.bundle");
        fs::write(&bundle_path, serde_json::to_string(&bundle).unwrap()).unwrap();
        
        // 4. Verify
        std::env::remove_var("COWEN_DEV_MODE");
        assert!(verify_plugin_bundle_with_root(&dylib_path, root_pub).is_ok());
    }

    #[test]
    fn test_tampered_binary_hash() {
        let (root_pair, dev_pair) = generate_keys();
        let root_pub = root_pair.public_key().as_ref();
        
        let dir = tempdir().unwrap();
        let dylib_path = dir.path().join("fake_plugin.dylib");
        let fake_bin = b"dummy_binary_data";
        
        // Issue dev cert
        let dev_pub_hex = hex::encode(dev_pair.public_key().as_ref());
        let cert_msg = format!("dev1:{}:1000:2000", dev_pub_hex);
        let cert_sig = root_pair.sign(cert_msg.as_bytes());
        let cert = DeveloperCert {
            developer_id: "dev1".to_string(),
            organization: "".to_string(),
            country: "".to_string(),
            public_key_hex: dev_pub_hex,
            issued_at: 1000,
            expires_at: 2000,
            signature_hex: hex::encode(cert_sig.as_ref()),
        };
        
        // Use a WRONG hash
        let manifest = PluginManifest {
            name: "fake".to_string(),
            version: "1.0".to_string(),
            binary_hash: "wrong_hash".to_string(),
            permissions: vec!["SearchProvider".to_string()],
            capabilities: vec![],
            required_privileges: vec![],
        };
        let manifest_str = serde_json::to_string(&manifest).unwrap();
        let manifest_sig = dev_pair.sign(manifest_str.as_bytes());
        
        let bundle = SignatureBundle {
            cert,
            manifest,
            manifest_signature_hex: hex::encode(manifest_sig.as_ref()),
        };
        
        fs::write(&dir.path().join("fake_plugin.bundle"), serde_json::to_string(&bundle).unwrap()).unwrap();
        fs::write(&dylib_path, fake_bin).unwrap();
        
        // It should fail!
        std::env::remove_var("COWEN_DEV_MODE");
        let result = verify_plugin_bundle_with_root(&dylib_path, root_pub);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Binary hash mismatch"));
    }

    #[test]
    fn test_dev_mode_bypass() {
        let dir = tempdir().unwrap();
        let dylib_path = dir.path().join("unsigned_plugin.dylib");
        fs::write(&dylib_path, b"data").unwrap();
        
        // Enable dev mode
        std::env::set_var("COWEN_DEV_MODE", "1");
        
        // Will pass even without bundle file!
        let result = verify_plugin_bundle_with_root(&dylib_path, OFFICIAL_ROOT_PUB_KEY);
        assert!(result.is_ok());
        
        // Clean up env for other tests
        std::env::remove_var("COWEN_DEV_MODE");
    }

    #[test]
    fn test_manifest_normalization_backward_compatibility() {
        let mut manifest = PluginManifest {
            name: "cowen_search_embedding".to_string(),
            version: "0.4.0".to_string(),
            binary_hash: "hash".to_string(),
            permissions: vec!["SearchProvider".to_string(), "LocalCacheAccess".to_string()],
            capabilities: vec![],
            required_privileges: vec![],
        };

        manifest.normalize();

        assert_eq!(manifest.capabilities, vec!["SearchProvider".to_string()]);
        assert!(manifest.required_privileges.contains(&"LocalCacheAccess".to_string()));
        assert!(manifest.required_privileges.contains(&"ModelAssetFetch".to_string()));
    }
}
