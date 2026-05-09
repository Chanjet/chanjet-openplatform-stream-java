use anyhow::{Result, Context};
use async_trait::async_trait;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::collections::HashMap;
use std::io::{Read, Write};
use fs2::FileExt;
use super::{Store, AuditEntry, DlqMessage, Item};
use crate::core::security;

pub struct FileStore {
    root_dir: PathBuf,
}

impl FileStore {
    pub fn new<P: AsRef<Path>>(root_dir: P, _fingerprint: &str) -> Result<Self> {
        let root_dir = root_dir.as_ref().to_path_buf();
        if !root_dir.exists() { fs::create_dir_all(&root_dir)?; }
        Ok(Self { root_dir })
    }

    fn get_path(&self, profile: &str, domain: &str, key: &str) -> PathBuf {
        let dir = self.root_dir.join(profile).join(domain);
        if !dir.exists() { let _ = fs::create_dir_all(&dir); }
        dir.join(key.replace(":", "_"))
    }
}

#[async_trait]
impl Store for FileStore {
    async fn get_config(&self, p: &str, k: &str) -> Result<String> { Ok(fs::read_to_string(self.get_path(p, "cfg", k))?) }
    async fn get_config_full(&self, p: &str, k: &str) -> Result<Item> {
        let val = self.get_config(p, k).await?;
        Ok(Item {
            profile: p.to_string(),
            key: k.to_string(),
            value: val,
            version: 0,
            updated_at: chrono::Utc::now().timestamp(),
        })
    }
    async fn set_config(&self, p: &str, k: &str, v: &str) -> Result<()> { Ok(fs::write(self.get_path(p, "cfg", k), v)?) }
    async fn set_config_conditional(&self, p: &str, k: &str, v: &str, _ev: u64) -> Result<()> {
        self.set_config(p, k, v).await
    }
    async fn delete_config(&self, p: &str, k: &str) -> Result<()> {
        let path = self.get_path(p, "cfg", k);
        if path.exists() { fs::remove_file(path)?; }
        Ok(())
    }
    async fn list_configs(&self, p: &str) -> Result<Vec<String>> {
        let dir = self.root_dir.join(p).join("cfg");
        if !dir.exists() { return Ok(vec![]); }
        Ok(fs::read_dir(dir)?.filter_map(|e| e.ok().map(|x| x.file_name().to_string_lossy().replace("_", ":"))).collect())
    }

    async fn get_secret(&self, p: &str, k: &str) -> Result<String> { Ok(fs::read_to_string(self.get_path(p, "sec", k))?) }
    async fn set_secret(&self, p: &str, k: &str, v: &str) -> Result<()> { Ok(fs::write(self.get_path(p, "sec", k), v)?) }
    async fn delete_secret(&self, p: &str, k: &str) -> Result<()> {
        let path = self.get_path(p, "sec", k);
        if path.exists() { fs::remove_file(path)?; }
        Ok(())
    }
    async fn list_secrets(&self, p: &str) -> Result<Vec<String>> {
        let dir = self.root_dir.join(p).join("sec");
        if !dir.exists() { return Ok(vec![]); }
        Ok(fs::read_dir(dir)?.filter_map(|e| e.ok().map(|x| x.file_name().to_string_lossy().replace("_", ":"))).collect())
    }

    async fn get_access_token(&self, p: &str) -> Result<crate::auth::models::Token> {
        let json = fs::read_to_string(self.get_path(p, "tok_v2", "access"))?;
        Ok(serde_json::from_str(&json)?)
    }
    async fn save_access_token(&self, p: &str, t: crate::auth::models::Token) -> Result<()> {
        let json = serde_json::to_string(&t)?;
        Ok(fs::write(self.get_path(p, "tok_v2", "access"), json)?)
    }
    async fn delete_access_token(&self, p: &str) -> Result<()> {
        let path = self.get_path(p, "tok_v2", "access");
        if path.exists() { fs::remove_file(path)?; }
        Ok(())
    }
    async fn get_app_access_token(&self, app_key: &str) -> Result<crate::auth::models::Token> {
        let json = fs::read_to_string(self.get_path(&format!("app:{}", app_key), "tok_v2", "app_access"))?;
        Ok(serde_json::from_str(&json)?)
    }
    async fn save_app_access_token(&self, app_key: &str, t: crate::auth::models::Token) -> Result<()> {
        let json = serde_json::to_string(&t)?;
        Ok(fs::write(self.get_path(&format!("app:{}", app_key), "tok_v2", "app_access"), json)?)
    }

    async fn get_app_ticket(&self, app_key: &str) -> Result<crate::auth::models::Ticket> {
        let json = fs::read_to_string(self.get_path(&format!("app:{}", app_key), "tic", "v1"))?;
        Ok(serde_json::from_str(&json)?)
    }
    async fn save_app_ticket(&self, app_key: &str, t: crate::auth::models::Ticket) -> Result<()> {
        let json = serde_json::to_string(&t)?;
        Ok(fs::write(self.get_path(&format!("app:{}", app_key), "tic", "v1"), json)?)
    }

    async fn delete_app_ticket(&self, app_key: &str) -> Result<()> {
        let path = self.get_path(&format!("app:{}", app_key), "tic", "v1");
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    // --- Permanent Code Domain ---
    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> Result<String> {
        Ok(fs::read_to_string(self.get_path(&format!("app:{}", app_key), "perm", &format!("{}:org", org_id)))?)
    }
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> Result<()> {
        Ok(fs::write(self.get_path(&format!("app:{}", app_key), "perm", &format!("{}:org", org_id)), code)?)
    }
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> Result<String> {
        Ok(fs::read_to_string(self.get_path(&format!("app:{}", app_key), "perm", &format!("{}:{}:user", org_id, user_id)))?)
    }
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> Result<()> {
        Ok(fs::write(self.get_path(&format!("app:{}", app_key), "perm", &format!("{}:{}:user", org_id, user_id)), code)?)
    }

    async fn get_token(&self, p: &str, k: &str) -> Result<String> { Ok(fs::read_to_string(self.get_path(p, "tok", k))?) }
    async fn set_token(&self, p: &str, k: &str, v: &str, _exp: u64) -> Result<()> { Ok(fs::write(self.get_path(p, "tok", k), v)?) }
    async fn delete_token(&self, p: &str, k: &str) -> Result<()> {
        let path = self.get_path(p, "tok", k);
        if path.exists() { fs::remove_file(path)?; }
        Ok(())
    }
    async fn list_tokens(&self, p: &str) -> Result<Vec<String>> {
        let dir = self.root_dir.join(p).join("tok");
        if !dir.exists() { return Ok(vec![]); }
        Ok(fs::read_dir(dir)?.filter_map(|e| e.ok().map(|x| x.file_name().to_string_lossy().replace("_", ":"))).collect())
    }

    async fn save_audit(&self, e: &AuditEntry) -> Result<()> {
        let key = format!("{}_{}", e.timestamp.timestamp_millis(), e.id);
        Ok(fs::write(self.get_path(&e.profile, "aud", &key), serde_json::to_string(e)?)?)
    }
    async fn list_audit(&self, p: &str, limit: usize) -> Result<Vec<AuditEntry>> {
        let dir = self.root_dir.join(p).join("aud");
        if !dir.exists() { return Ok(vec![]); }
        let mut paths: Vec<_> = fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();
        paths.sort_by_key(|e| e.file_name());
        paths.reverse();
        let mut entries = Vec::new();
        for e in paths.into_iter().take(limit) {
            if let Ok(json) = fs::read_to_string(e.path()) {
                if let Ok(ent) = serde_json::from_str(&json) { entries.push(ent); }
            }
        }
        Ok(entries)
    }

    async fn push_dlq(&self, m: &DlqMessage) -> Result<()> {
        let dir = self.root_dir.join(&m.profile).join("dlq").join(&m.topic);
        if !dir.exists() { fs::create_dir_all(&dir)?; }
        let key = format!("{}_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0), uuid::Uuid::new_v4());
        Ok(fs::write(dir.join(key), serde_json::to_string(m)?)?)
    }
    async fn pop_dlq(&self, p: &str, t: &str) -> Result<Option<DlqMessage>> {
        let dir = self.root_dir.join(p).join("dlq").join(t);
        if !dir.exists() { return Ok(None); }
        let mut paths: Vec<_> = fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();
        paths.sort_by_key(|e| e.file_name());
        if let Some(e) = paths.first() {
            let json = fs::read_to_string(e.path())?;
            fs::remove_file(e.path())?;
            Ok(Some(serde_json::from_str(&json)?))
        } else { Ok(None) }
    }
    async fn list_dlq(&self, p: &str, limit: usize) -> Result<Vec<DlqMessage>> {
        let dir = self.root_dir.join(p).join("dlq");
        if !dir.exists() { return Ok(vec![]); }
        let mut msgs = Vec::new();
        for t_dir in fs::read_dir(dir)? {
            if let Ok(t_dir) = t_dir {
                if t_dir.path().is_dir() {
                    for e in fs::read_dir(t_dir.path())? {
                        if let Ok(e) = e {
                            if let Ok(json) = fs::read_to_string(e.path()) {
                                if let Ok(m) = serde_json::from_str(&json) { msgs.push(m); }
                            }
                        }
                        if msgs.len() >= limit { break; }
                    }
                }
            }
            if msgs.len() >= limit { break; }
        }
        Ok(msgs)
    }

    async fn list_all_dlq(&self, p: &str) -> Result<Vec<DlqMessage>> {
        let dir = self.root_dir.join(p).join("dlq");
        if !dir.exists() { return Ok(vec![]); }
        let mut msgs = Vec::new();
        for t_dir in fs::read_dir(dir)? {
            if let Ok(t_dir) = t_dir {
                if t_dir.path().is_dir() {
                    for e in fs::read_dir(t_dir.path())? {
                        if let Ok(e) = e {
                            if let Ok(json) = fs::read_to_string(e.path()) {
                                if let Ok(m) = serde_json::from_str(&json) { msgs.push(m); }
                            }
                        }
                    }
                }
            }
        }
        Ok(msgs)
    }


    async fn clear_profile(&self, p: &str) -> Result<()> {
        let dir = self.root_dir.join(p);
        if dir.exists() { fs::remove_dir_all(dir)?; }
        Ok(())
    }
    async fn rename_profile(&self, old: &str, new: &str) -> Result<()> {
        let od = self.root_dir.join(old);
        let nd = self.root_dir.join(new);
        if od.exists() { fs::rename(od, nd)?; }
        Ok(())
    }
    async fn list_all_profiles(&self) -> Result<Vec<String>> {
        if !self.root_dir.exists() || self.root_dir.is_file() { return Ok(vec![]); }
        let mut profiles = Vec::new();
        for e in fs::read_dir(&self.root_dir)?.filter_map(|e| e.ok()) {
            if e.path().is_dir() {
                profiles.push(e.file_name().to_string_lossy().to_string());
            }
        }
        Ok(profiles)
    }

}

pub struct MonolithicSealStore {
    path: PathBuf,
    key: [u8; 32],
    lock_path: PathBuf,
}

impl MonolithicSealStore {
    pub fn new(path: PathBuf, fingerprint: &str) -> Self {
        let key = security::derive_key(fingerprint);
        let lock_path = path.with_extension("lock");
        Self { path, key, lock_path }
    }

    fn load_all(&self) -> Result<HashMap<String, HashMap<String, String>>> {
        if !self.path.exists() { return Ok(HashMap::new()); }
        let mut file = File::open(&self.path)?;
        let mut encrypted = Vec::new();
        file.read_to_end(&mut encrypted)?;
        if encrypted.is_empty() { return Ok(HashMap::new()); }
        let decrypted = match security::decrypt(&encrypted, &self.key) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(target: "sys", error = %e, "Vault decryption failed. Data might be from an incompatible version or different machine.");
                return Ok(HashMap::new());
            }
        };
        match serde_json::from_slice(&decrypted) {
            Ok(data) => Ok(data),
            Err(e) => {
                tracing::warn!(target: "sys", error = %e, "Vault parsing failed. Starting fresh.");
                Ok(HashMap::new())
            }
        }
    }

    fn save_all(&self, data: &HashMap<String, HashMap<String, String>>) -> Result<()> {
        let json = serde_json::to_vec(data)?;
        let encrypted = security::encrypt(&json, &self.key)?;
        let mut file = OpenOptions::new().write(true).create(true).truncate(true).open(&self.path)?;
        file.write_all(&encrypted)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = file.metadata()?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&self.path, perms)?;
        }
        Ok(())
    }

    fn with_lock<F, R>(&self, f: F) -> Result<R> where F: FnOnce() -> Result<R> {
        let lock_file = OpenOptions::new().read(true).write(true).create(true).open(&self.lock_path)?;
        lock_file.lock_exclusive()?;
        let res = f();
        let _ = lock_file.unlock();
        res
    }
}

#[async_trait]
impl Store for MonolithicSealStore {
    async fn get_config(&self, p: &str, k: &str) -> Result<String> {
        self.with_lock(|| self.load_all()?.get(p).and_then(|m| m.get(k)).cloned().context("not found"))
    }
    async fn get_config_full(&self, p: &str, k: &str) -> Result<Item> {
        let val = self.get_config(p, k).await?;
        Ok(Item { profile: p.to_string(), key: k.to_string(), value: val, version: 0, updated_at: 0 })
    }
    async fn set_config(&self, p: &str, k: &str, v: &str) -> Result<()> {
        self.with_lock(|| {
            let mut data = self.load_all()?;
            data.entry(p.to_string()).or_insert_with(HashMap::new).insert(k.to_string(), v.to_string());
            self.save_all(&data)
        })
    }
    async fn set_config_conditional(&self, p: &str, k: &str, v: &str, _ev: u64) -> Result<()> { self.set_config(p, k, v).await }
    async fn delete_config(&self, p: &str, k: &str) -> Result<()> {
        self.with_lock(|| {
            let mut data = self.load_all()?;
            if let Some(m) = data.get_mut(p) {
                m.remove(k);
                self.save_all(&data)?;
            }
            Ok(())
        })
    }
    async fn list_configs(&self, p: &str) -> Result<Vec<String>> {
        let data = self.load_all()?;
        Ok(data.get(p).map(|m| m.keys().cloned().collect()).unwrap_or_default())
    }

    async fn get_secret(&self, p: &str, k: &str) -> Result<String> { self.get_config(p, k).await }
    async fn set_secret(&self, p: &str, k: &str, v: &str) -> Result<()> { self.set_config(p, k, v).await }
    async fn delete_secret(&self, p: &str, k: &str) -> Result<()> { self.delete_config(p, k).await }
    async fn list_secrets(&self, p: &str) -> Result<Vec<String>> { self.list_configs(p).await }

    async fn get_access_token(&self, p: &str) -> Result<crate::auth::models::Token> {
        let json = self.get_config(p, "access_token_v2").await?;
        Ok(serde_json::from_str(&json)?)
    }
    async fn save_access_token(&self, p: &str, t: crate::auth::models::Token) -> Result<()> {
        let json = serde_json::to_string(&t)?;
        self.set_config(p, "access_token_v2", &json).await
    }
    async fn delete_access_token(&self, p: &str) -> Result<()> {
        self.delete_config(p, "access_token_v2").await
    }
    async fn get_app_access_token(&self, app_key: &str) -> Result<crate::auth::models::Token> {
        let json = self.get_config(&format!("app:{}", app_key), "app_access_token_v2").await?;
        Ok(serde_json::from_str(&json)?)
    }
    async fn save_app_access_token(&self, app_key: &str, t: crate::auth::models::Token) -> Result<()> {
        let json = serde_json::to_string(&t)?;
        self.set_config(&format!("app:{}", app_key), "app_access_token_v2", &json).await
    }

    async fn get_app_ticket(&self, app_key: &str) -> Result<crate::auth::models::Ticket> {
        let json = self.get_config(&format!("app:{}", app_key), "app_ticket_v2").await?;
        Ok(serde_json::from_str(&json)?)
    }
    async fn save_app_ticket(&self, app_key: &str, t: crate::auth::models::Ticket) -> Result<()> {
        let json = serde_json::to_string(&t)?;
        self.set_config(&format!("app:{}", app_key), "app_ticket_v2", &json).await
    }
    async fn delete_app_ticket(&self, app_key: &str) -> Result<()> {
        self.delete_config(&format!("app:{}", app_key), "app_ticket_v2").await
    }

    // --- Permanent Code ---
    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> Result<String> {
        self.get_config(&format!("app:{}", app_key), &format!("perm:{}:org", org_id)).await
    }
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> Result<()> {
        self.set_config(&format!("app:{}", app_key), &format!("perm:{}:org", org_id), code).await
    }
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> Result<String> {
        self.get_config(&format!("app:{}", app_key), &format!("perm:{}:{}:user", org_id, user_id)).await
    }
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> Result<()> {
        self.set_config(&format!("app:{}", app_key), &format!("perm:{}:{}:user", org_id, user_id), code).await
    }

    async fn get_token(&self, p: &str, k: &str) -> Result<String> { self.get_config(p, k).await }
    async fn set_token(&self, p: &str, k: &str, v: &str, _exp: u64) -> Result<()> { self.set_config(p, k, v).await }
    async fn delete_token(&self, p: &str, k: &str) -> Result<()> { self.delete_config(p, k).await }
    async fn list_tokens(&self, p: &str) -> Result<Vec<String>> { self.list_configs(p).await }

    async fn save_audit(&self, _e: &AuditEntry) -> Result<()> { Ok(()) }
    async fn list_audit(&self, _p: &str, _l: usize) -> Result<Vec<AuditEntry>> { Ok(vec![]) }
    async fn push_dlq(&self, _m: &DlqMessage) -> Result<()> { Ok(()) }
    async fn pop_dlq(&self, _p: &str, _t: &str) -> Result<Option<DlqMessage>> { Ok(None) }
    async fn list_dlq(&self, _p: &str, _l: usize) -> Result<Vec<DlqMessage>> { Ok(vec![]) }
    async fn list_all_dlq(&self, _p: &str) -> Result<Vec<DlqMessage>> { Ok(vec![]) }

    async fn clear_profile(&self, p: &str) -> Result<()> {
        self.with_lock(|| {
            let mut data = self.load_all()?;
            data.remove(p);
            self.save_all(&data)
        })
    }
    async fn rename_profile(&self, old: &str, new: &str) -> Result<()> {
        self.with_lock(|| {
            let mut data = self.load_all()?;
            if let Some(m) = data.remove(old) {
                data.insert(new.to_string(), m);
                self.save_all(&data)?;
            }
            Ok(())
        })
    }
    async fn list_all_profiles(&self) -> Result<Vec<String>> {
        let data = self.load_all()?;
        Ok(data.keys().cloned().collect())
    }

}

pub struct LocalStoreBuilder;

#[async_trait]
impl super::StoreBuilder for LocalStoreBuilder {
    fn scheme(&self) -> &str { "local" }

    async fn build(&self, _url: &str, app_dir: &Path, fingerprint: &str) -> Result<Arc<dyn Store>> {
        let seal_path = app_dir.join(".seal");
        if seal_path.is_file() {
            Ok(Arc::new(MonolithicSealStore::new(seal_path, fingerprint)))
        } else {
            Ok(Arc::new(FileStore::new(seal_path, fingerprint)?))
        }
    }
}

inventory::submit! { super::StoreBuilderRegistration { builder: &LocalStoreBuilder } }
