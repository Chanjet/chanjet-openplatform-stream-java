use cowen_common::{CowenResult, CowenError, security};
use std::path::Path;
use std::fs;
use crate::file::core::FileStore;

pub async fn migrate_v2_to_v3(root_dir: &Path, profile: &str, fingerprint: Option<&str>) -> CowenResult<()> {
    // 1. Monolithic Migration (Single file -> Multiple files)
    let old_monolithic = root_dir.join(format!("{}.json", profile));
    if old_monolithic.exists() {
        tracing::info!(target: "sys", profile = %profile, "Migrating monolithic v2 vault to v3");
        let content = fs::read(&old_monolithic).map_err(|e| CowenError::Store(e.to_string()))?;
        
        let json_bytes = if let Some(fp) = fingerprint {
            let key = security::derive_key(fp);
            security::decrypt(&content, &key).map_err(|e| CowenError::Store(e.to_string()))?
        } else {
            content
        };

        let json = String::from_utf8(json_bytes).map_err(|e| CowenError::Store(e.to_string()))?;
        let data: serde_json::Value = serde_json::from_str(&json).map_err(|e| CowenError::Store(e.to_string()))?;
        let store = FileStore::new(root_dir, fingerprint)?;

        if let Some(obj) = data.as_object() {
            for (prefix, items) in obj {
                if let Some(items_obj) = items.as_object() {
                    for (id, val) in items_obj {
                        let path = store.get_path(profile, prefix, id, true);
                        let val_str = serde_json::to_string(val).map_err(|e| CowenError::Store(e.to_string()))?;
                        let final_data = if let Some(fp) = fingerprint {
                            let key = security::derive_key(fp);
                            security::encrypt(val_str.as_bytes(), &key).map_err(|e| CowenError::Store(e.to_string()))?
                        } else {
                            val_str.into_bytes()
                        };
                        fs::write(path, final_data).map_err(|e| CowenError::Store(e.to_string()))?;
                    }
                }
            }
        }

        // Backup old file
        let backup = old_monolithic.with_extension("json.v2_bak");
        fs::rename(old_monolithic, backup).map_err(|e| CowenError::Store(e.to_string()))?;
    }

    // 2. Directory Migration (Old domains -> New prefixes)
    let profile_dir = root_dir.join(profile);
    if profile_dir.is_dir() {
        let tok_v2 = profile_dir.join("tok_v2");
        if tok_v2.is_dir() {
             let tokens = profile_dir.join("tokens");
             if !tokens.exists() {
                 fs::rename(tok_v2, tokens).map_err(|e| CowenError::Store(e.to_string()))?;
             }
        }
        
        let dlq_old = profile_dir.join("dlq");
        if dlq_old.is_dir() {
            let mut to_move = Vec::new();
            if let Ok(topics) = fs::read_dir(&dlq_old) {
                for topic in topics.flatten() {
                    if topic.path().is_dir() {
                        if let Ok(msgs) = fs::read_dir(topic.path()) {
                            for msg in msgs.flatten() {
                                to_move.push((msg.path(), dlq_old.join(msg.file_name())));
                            }
                        }
                    }
                }
            }
            
            for (old, new) in to_move {
                if !new.exists() {
                    let _ = fs::rename(old, new);
                }
            }
        }
    }

    Ok(())
}
