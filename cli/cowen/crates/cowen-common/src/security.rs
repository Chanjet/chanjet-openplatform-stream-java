use crate::{CowenResult, CowenError};
use sha2::{Sha256, Digest};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce
};
use rand::{RngCore, thread_rng};
use std::path::Path;

pub fn get_machine_fingerprint() -> CowenResult<String> {
    match cowen_infra::sys::get_sys_fingerprint().get_machine_id() {
        Ok(uuid) => Ok(uuid),
        Err(e) => Err(CowenError::Security(format!("Failed to retrieve machine fingerprint: {}", e))),
    }
}

pub fn derive_key(fingerprint: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(fingerprint);
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

pub fn encrypt(data: &[u8], key: &[u8; 32]) -> CowenResult<Vec<u8>> {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce_bytes = [0u8; 12];
    thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let ciphertext = cipher.encrypt(nonce, data)
        .map_err(|e| CowenError::Security(format!("Encryption failed: {}", e)))?;
        
    let mut combined = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    Ok(combined)
}

pub fn decrypt(combined: &[u8], key: &[u8; 32]) -> CowenResult<Vec<u8>> {
    if combined.len() < 12 {
        return Err(CowenError::Security("Invalid encrypted data".to_string()));
    }
    
    let cipher = Aes256Gcm::new(key.into());
    let nonce = Nonce::from_slice(&combined[..12]);
    let ciphertext = &combined[12..];
    
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|e| CowenError::Security(format!("Decryption failed: {}", e)))?;
        
    Ok(plaintext)
}

pub fn seal_config<P: AsRef<Path>>(path: P, data: &[u8], fingerprint: &str) -> CowenResult<()> {
    let key = derive_key(fingerprint);
    let encrypted = encrypt(data, &key)?;
    crate::utils::secure_write(path, encrypted)?;
    Ok(())
}

pub fn unseal_config<P: AsRef<Path>>(path: P, fingerprint: &str) -> CowenResult<Vec<u8>> {
    let key = derive_key(fingerprint);
    let encrypted = std::fs::read(path)?;
    decrypt(&encrypted, &key)
}

pub mod ssrf {
    use crate::{CowenError, CowenResult};
    use crate::config::SecurityLevel;
    use std::net::IpAddr;
    use ipnet::IpNet;
    
    pub fn validate_ssrf(url_str: &str, level: &SecurityLevel, whitelist: &[String]) -> CowenResult<()> {
        if matches!(level, SecurityLevel::Disabled) {
            return Ok(());
        }

        let url = url::Url::parse(url_str).map_err(|e| CowenError::api(format!("Invalid webhook URL: {}", e)))?;
        let host = url.host_str().unwrap_or("");
        
        let ip: IpAddr = match host.parse() {
            Ok(addr) => addr,
            Err(_) => {
                if host == "localhost" {
                    std::net::Ipv4Addr::new(127, 0, 0, 1).into()
                } else {
                    // Simple DNS resolution (blocking but fine for validation)
                    use std::net::ToSocketAddrs;
                    if let Ok(mut addrs) = format!("{}:80", host).to_socket_addrs() {
                        if let Some(addr) = addrs.next() {
                            addr.ip()
                        } else {
                            return Err(CowenError::api(format!("SSRF Violation: Unable to resolve host {}", host)));
                        }
                    } else {
                        return Err(CowenError::api(format!("SSRF Violation: Unable to resolve host {}", host)));
                    }
                }
            }
        };

        if matches!(level, SecurityLevel::Strict) {
            if !ip.is_loopback() {
                return Err(CowenError::api("SSRF Violation: Only loopback addresses allowed in Strict mode."));
            }
            return Ok(());
        }

        if matches!(level, SecurityLevel::Flexible) {
            if ip.is_loopback() {
                return Ok(());
            }
            
            for cidr_str in whitelist {
                if let Ok(net) = cidr_str.parse::<IpNet>() {
                    if net.contains(&ip) {
                        return Ok(());
                    }
                } else if let Ok(whitelist_ip) = cidr_str.parse::<IpAddr>() {
                    if whitelist_ip == ip {
                        return Ok(());
                    }
                }
            }
            return Err(CowenError::api(format!("SSRF Violation: IP {} is not in the whitelist.", ip)));
        }

        Ok(())
    }
}
