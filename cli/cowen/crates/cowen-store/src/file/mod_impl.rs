use cowen_common::CowenResult;
use async_trait::async_trait;
use cowen_common::models::{Token, Ticket, Item, AuditEntry, DlqMessage};
use crate::file::core::FileStore;

#[async_trait]
impl cowen_common::store::Store for FileStore {
    async fn shutdown(&self) -> CowenResult<()> { Ok(()) }

    async fn get_config(&self, p: &str, k: &str) -> CowenResult<String> { 
        Ok(self.load::<Item>(p, k)?.value)
    }
    async fn get_config_metadata(&self, p: &str, k: &str) -> CowenResult<(u64, i64)> {
        let item = self.load::<Item>(p, k)?;
        Ok((item.version, item.updated_at))
    }
    async fn get_config_full(&self, p: &str, k: &str) -> CowenResult<Item> {
        self.load::<Item>(p, k)
    }
    async fn set_config(&self, p: &str, k: &str, v: &str) -> CowenResult<()> {
        let item = Item {
            profile: p.to_string(),
            key: k.to_string(),
            value: v.to_string(),
            version: 0,
            updated_at: chrono::Utc::now().timestamp(),
        };
        self.save(p, k, &item)
    }
    async fn set_config_conditional(&self, p: &str, k: &str, v: &str, _ev: u64) -> CowenResult<()> {
        self.set_config(p, k, v).await
    }
    async fn delete_config(&self, p: &str, k: &str) -> CowenResult<()> {
        self.delete::<Item>(p, k)
    }
    async fn list_configs(&self, p: &str) -> CowenResult<Vec<String>> {
        self.list::<Item>(p)
    }

    async fn get_secret(&self, p: &str, k: &str) -> CowenResult<String> {
        self.load_raw("sec", p, k)
    }
    async fn set_secret(&self, p: &str, k: &str, v: &str) -> CowenResult<()> {
        self.save_raw("sec", p, k, v)
    }
    async fn delete_secret(&self, p: &str, k: &str) -> CowenResult<()> {
        let path = self.get_path(p, "sec", k, false);
        if path.exists() { std::fs::remove_file(path).map_err(|e| cowen_common::CowenError::Store(e.to_string()))?; }
        Ok(())
    }
    async fn list_secrets(&self, p: &str) -> CowenResult<Vec<String>> {
        let dir = self.root_dir().join(p).join("sec");
        if !dir.exists() { return Ok(vec![]); }
        Ok(std::fs::read_dir(dir).map_err(|e| cowen_common::CowenError::Store(e.to_string()))?.filter_map(|e| e.ok().map(|x| x.file_name().to_string_lossy().into_owned())).collect())
    }

    async fn get_access_token(&self, p: &str) -> CowenResult<Token> { self.load(p, "access") }
    async fn save_access_token(&self, p: &str, t: Token) -> CowenResult<()> { self.save(p, "access", &t) }
    async fn delete_access_token(&self, p: &str) -> CowenResult<()> { self.delete::<Token>(p, "access") }
    async fn get_refresh_token(&self, p: &str) -> CowenResult<Token> { self.load(p, "refresh") }
    async fn save_refresh_token(&self, p: &str, t: Token) -> CowenResult<()> { self.save(p, "refresh", &t) }
    async fn delete_refresh_token(&self, p: &str) -> CowenResult<()> { self.delete::<Token>(p, "refresh") }
    async fn get_app_access_token(&self, k: &str) -> CowenResult<Token> { self.load("global", k) }
    async fn save_app_access_token(&self, k: &str, t: Token) -> CowenResult<()> { self.save("global", k, &t) }
    async fn delete_app_access_token(&self, k: &str) -> CowenResult<()> { self.delete::<Token>("global", k) }

    async fn get_app_ticket(&self, k: &str) -> CowenResult<Ticket> { self.load("global", k) }
    async fn save_app_ticket(&self, k: &str, t: Ticket) -> CowenResult<()> { self.save("global", k, &t) }
    async fn delete_app_ticket(&self, k: &str) -> CowenResult<()> { self.delete::<Ticket>("global", k) }

    async fn get_org_permanent_code(&self, k: &str, org: &str) -> CowenResult<String> { self.load_raw("perm_org", k, org) }
    async fn save_org_permanent_code(&self, k: &str, org: &str, c: &str) -> CowenResult<()> { self.save_raw("perm_org", k, org, c) }
    async fn get_user_permanent_code(&self, k: &str, org: &str, user: &str) -> CowenResult<String> { self.load_raw("perm_user", k, &format!("{}_{}", org, user)) }
    async fn save_user_permanent_code(&self, k: &str, org: &str, user: &str, c: &str) -> CowenResult<()> { self.save_raw("perm_user", k, &format!("{}_{}", org, user), c) }

    async fn get_token(&self, p: &str, k: &str) -> CowenResult<String> { self.load_raw("tokens_legacy", p, k) }
    async fn set_token(&self, p: &str, k: &str, v: &str, _exp: u64) -> CowenResult<()> { self.save_raw("tokens_legacy", p, k, v) }
    async fn delete_token(&self, p: &str, k: &str) -> CowenResult<()> { 
        let path = self.get_path(p, "tokens_legacy", k, false);
        if path.exists() { std::fs::remove_file(path).map_err(|e| cowen_common::CowenError::Store(e.to_string()))?; }
        Ok(())
    }
    async fn list_tokens(&self, p: &str) -> CowenResult<Vec<String>> {
        let dir = self.root_dir().join(p).join("tokens_legacy");
        if !dir.exists() { return Ok(vec![]); }
        Ok(std::fs::read_dir(dir).map_err(|e| cowen_common::CowenError::Store(e.to_string()))?.filter_map(|e| e.ok().map(|x| x.file_name().to_string_lossy().into_owned())).collect())
    }

    async fn save_audit(&self, e: &AuditEntry) -> CowenResult<()> { self.save(&e.profile, &e.id, e) }
    async fn list_audit(&self, p: &str, limit: usize) -> CowenResult<Vec<AuditEntry>> { self.list_all_paged(p, 0, limit) }

    async fn push_dlq(&self, m: &DlqMessage) -> CowenResult<()> { 
        let id = format!("{}_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0), uuid::Uuid::new_v4());
        self.save(&m.profile, &id, m) 
    }
    async fn pop_dlq(&self, p: &str, topic: &str) -> CowenResult<Option<DlqMessage>> {
        let dir = self.root_dir().join(p).join("dlq");
        if !dir.exists() { return Ok(None); }
        
        let mut paths: Vec<_> = std::fs::read_dir(dir).map_err(|e| cowen_common::CowenError::Store(e.to_string()))?.flatten().collect();
        paths.sort_by_key(|e| e.file_name());

        for entry in paths {
            if let Ok(m) = self.load::<DlqMessage>(p, entry.file_name().to_str().unwrap()) {
                if m.topic == topic {
                    let _ = std::fs::remove_file(entry.path());
                    return Ok(Some(m));
                }
            }
        }
        Ok(None)
    }
    async fn list_dlq(&self, p: &str, limit: usize) -> CowenResult<Vec<DlqMessage>> { self.list_all_paged(p, 0, limit) }
    async fn list_all_dlq(&self, p: &str) -> CowenResult<Vec<DlqMessage>> { self.list_all_paged(p, 0, 10000) }
    async fn get_dlq_by_id(&self, id: i64) -> CowenResult<Option<DlqMessage>> {
        let profiles = self.list_all_profiles().await?;
        for p in profiles {
            let msgs = self.list_all_dlq(&p).await?;
            if let Some(m) = msgs.into_iter().find(|m| m.id == Some(id)) {
                return Ok(Some(m));
            }
        }
        Ok(None)
    }
    async fn list_dlq_paged(&self, p: &str, offset: usize, limit: usize) -> CowenResult<Vec<DlqMessage>> { self.list_all_paged(p, offset, limit) }
    async fn delete_dlq_by_id(&self, id: i64) -> CowenResult<()> { 
        let profiles = self.list_all_profiles().await?;
        for p in profiles {
            let ids = self.list::<DlqMessage>(&p)?;
            for file_id in ids {
                if let Ok(m) = self.load::<DlqMessage>(&p, &file_id) {
                    if m.id == Some(id) {
                        return self.delete::<DlqMessage>(&p, &file_id);
                    }
                }
            }
        }
        Ok(())
    }

    async fn migrate(&self) -> CowenResult<()> { Ok(()) }

    async fn clear_profile(&self, p: &str) -> CowenResult<()> {
        let dir = self.root_dir().join(p);
        if dir.exists() { std::fs::remove_dir_all(dir).map_err(|e| cowen_common::CowenError::Store(e.to_string()))?; }
        Ok(())
    }
    async fn rename_profile(&self, old: &str, new: &str) -> CowenResult<()> {
        let old_dir = self.root_dir().join(old);
        let new_dir = self.root_dir().join(new);
        if old_dir.exists() { std::fs::rename(old_dir, new_dir).map_err(|e| cowen_common::CowenError::Store(e.to_string()))?; }
        Ok(())
    }
    async fn list_all_profiles(&self) -> CowenResult<Vec<String>> {
        Ok(std::fs::read_dir(self.root_dir()).map_err(|e| cowen_common::CowenError::Store(e.to_string()))?.filter_map(|e| e.ok().filter(|x| x.path().is_dir()).map(|x| x.file_name().to_string_lossy().into_owned())).collect())
    }
    async fn raw_del(&self, _key: &str) -> CowenResult<()> { Ok(()) }

    fn name(&self) -> &str { "file" }
    fn description(&self) -> String { format!("Local File Store at {:?}", self.root_dir()) }
}

impl FileStore {
    fn load_raw(&self, prefix: &str, profile: &str, id: &str) -> CowenResult<String> {
        let path = self.get_path(profile, prefix, id, false);
        if !path.exists() { return Err(cowen_common::CowenError::Store("Not found".into())); }
        let content = std::fs::read(path).map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
        
        let json_bytes = if let Some(fp) = self.fingerprint() {
            let key = cowen_common::security::derive_key(fp);
            cowen_common::security::decrypt(&content, &key).map_err(|e| cowen_common::CowenError::Store(e.to_string()))?
        } else {
            content
        };

        String::from_utf8(json_bytes).map_err(|e| cowen_common::CowenError::Store(e.to_string()))
    }

    fn save_raw(&self, prefix: &str, profile: &str, id: &str, data: &str) -> CowenResult<()> {
        let path = self.get_path(profile, prefix, id, true);
        let final_data = if let Some(fp) = self.fingerprint() {
            let key = cowen_common::security::derive_key(fp);
            cowen_common::security::encrypt(data.as_bytes(), &key).map_err(|e| cowen_common::CowenError::Store(e.to_string()))?
        } else {
            data.as_bytes().to_vec()
        };
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        
        options.open(&path)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(&final_data)
            })
            .map_err(|e| cowen_common::CowenError::Store(e.to_string()))
    }
}
