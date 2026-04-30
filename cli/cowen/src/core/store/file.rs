use anyhow::{Result, Context};
use async_trait::async_trait;
use std::fs;
use std::path::{Path, PathBuf};
use super::{Store, AuditEntry, DlqMessage, Item};

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

    async fn get_token(&self, p: &str, k: &str) -> Result<String> { Ok(fs::read_to_string(self.get_path(p, "tok", k))?) }
    async fn set_token(&self, p: &str, k: &str, v: &str, _exp: u64) -> Result<()> { Ok(fs::write(self.get_path(p, "tok", k), v)?) }
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

    async fn get_cache(&self, p: &str, k: &str) -> Result<String> { Ok(fs::read_to_string(self.get_path(p, "cch", k))?) }
    async fn set_cache(&self, p: &str, k: &str, v: &str, _ttl: u64) -> Result<()> { Ok(fs::write(self.get_path(p, "cch", k), v)?) }

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
        if !self.root_dir.exists() { return Ok(vec![]); }
        let mut profiles = Vec::new();
        for e in fs::read_dir(&self.root_dir)?.filter_map(|e| e.ok()) {
            if e.path().is_dir() {
                profiles.push(e.file_name().to_string_lossy().to_string());
            }
        }
        Ok(profiles)
    }

    async fn notify_config_changed(&self, _profile: &str, _key: &str) -> Result<()> { Ok(()) }
    async fn watch_config(&self, _profile: &str) -> Result<std::pin::Pin<Box<dyn tokio_stream::Stream<Item = String> + Send>>> {
        Err(anyhow::anyhow!("Notifications not supported for FileStore"))
    }
}

pub struct LocalStoreBuilder;

#[async_trait]
impl super::StoreBuilder for LocalStoreBuilder {
    fn scheme(&self) -> &str {
        "local"
    }

    async fn build(&self, _url: &str, app_dir: &Path, fingerprint: &str) -> Result<Arc<dyn Store>> {
        let seal_path = app_dir.join(".seal");
        Ok(Arc::new(FileStore::new(seal_path, fingerprint)?))
    }
}

inventory::submit! { super::StoreBuilderRegistration { builder: &LocalStoreBuilder } }
