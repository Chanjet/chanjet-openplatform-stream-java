use crate::file::core::FileStore;
use cowen_common::{security, CowenError, CowenResult};
use std::fs;
use std::path::Path;

fn read_monolithic_v2(path: &Path, fingerprint: Option<&str>) -> CowenResult<serde_json::Value> {
    let content = fs::read(path).map_err(|e| CowenError::Store(e.to_string()))?;
    let json_bytes = if let Some(fp) = fingerprint {
        let key = security::derive_key(fp);
        security::decrypt(&content, &key).map_err(|e| CowenError::Store(e.to_string()))?
    } else {
        content
    };
    let json = String::from_utf8(json_bytes).map_err(|e| CowenError::Store(e.to_string()))?;
    serde_json::from_str(&json).map_err(|e| CowenError::Store(e.to_string()))
}

fn process_monolithic_items(
    store: &FileStore,
    profile: &str,
    fingerprint: Option<&str>,
    obj: &serde_json::Map<String, serde_json::Value>,
) -> CowenResult<()> {
    for (prefix, items) in obj {
        let actual_prefix = if prefix == "config" { "cfg" } else { prefix };
        if let Some(items_obj) = items.as_object() {
            for (id, val) in items_obj {
                let path = store.get_path(profile, actual_prefix, id, true);
                let val_str =
                    serde_json::to_string(val).map_err(|e| CowenError::Store(e.to_string()))?;
                let final_data = if let Some(fp) = fingerprint {
                    let key = security::derive_key(fp);
                    security::encrypt(val_str.as_bytes(), &key)
                        .map_err(|e| CowenError::Store(e.to_string()))?
                } else {
                    val_str.into_bytes()
                };
                fs::write(path, final_data).map_err(|e| CowenError::Store(e.to_string()))?;
            }
        }
    }
    Ok(())
}

async fn migrate_monolithic_v2_to_v3(
    root_dir: &Path,
    profile: &str,
    fingerprint: Option<&str>,
) -> CowenResult<()> {
    let old_monolithic = root_dir.join(format!("{}.json", profile));
    if !old_monolithic.exists() {
        return Ok(());
    }

    tracing::info!(target: "sys", profile = %profile, "Migrating monolithic v2 vault to v3");
    let data = read_monolithic_v2(&old_monolithic, fingerprint)?;
    let store = FileStore::new(root_dir, fingerprint)?;

    if let Some(obj) = data.as_object() {
        process_monolithic_items(&store, profile, fingerprint, obj)?;
    }

    // Backup old file
    let backup = old_monolithic.with_extension("json.v2_bak");
    fs::rename(old_monolithic, backup).map_err(|e| CowenError::Store(e.to_string()))?;
    Ok(())
}

fn migrate_dlq_v2_to_v3(profile_dir: &Path) -> CowenResult<()> {
    let dlq_old = profile_dir.join("dlq");
    if !dlq_old.is_dir() {
        return Ok(());
    }

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
    Ok(())
}

async fn migrate_directory_v2_to_v3(root_dir: &Path, profile: &str) -> CowenResult<()> {
    let profile_dir = root_dir.join(profile);
    if !profile_dir.is_dir() {
        return Ok(());
    }

    let tok_v2 = profile_dir.join("tok_v2");
    if tok_v2.is_dir() {
        let tokens = profile_dir.join("tokens");
        if !tokens.exists() {
            fs::rename(tok_v2, tokens).map_err(|e| CowenError::Store(e.to_string()))?;
        }
    }

    migrate_dlq_v2_to_v3(&profile_dir)?;
    Ok(())
}

pub async fn migrate_v2_to_v3(
    root_dir: &Path,
    profile: &str,
    fingerprint: Option<&str>,
) -> CowenResult<()> {
    migrate_monolithic_v2_to_v3(root_dir, profile, fingerprint).await?;
    migrate_directory_v2_to_v3(root_dir, profile).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cowen_common::models::Item;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_migration_v2_to_v3() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let profile = "test_mig";

        // 1. Write monolithic V2 json
        let old_monolithic = root.join(format!("{}.json", profile));
        let monolithic_data = serde_json::json!({
            "config": {
                "key1": {
                    "profile": profile,
                    "key": "key1",
                    "value": "val1",
                    "version": 1,
                    "updated_at": 123456
                }
            }
        });
        std::fs::write(
            &old_monolithic,
            serde_json::to_string(&monolithic_data).unwrap(),
        )
        .unwrap();

        // 2. Setup directory V2 layout
        let profile_dir = root.join(profile);
        std::fs::create_dir_all(profile_dir.join("tok_v2")).unwrap();
        std::fs::write(profile_dir.join("tok_v2/access_token"), "token_val").unwrap();

        std::fs::create_dir_all(profile_dir.join("dlq/topic_a")).unwrap();
        std::fs::write(profile_dir.join("dlq/topic_a/msg1"), "dlq_val").unwrap();

        // 3. Run migration
        migrate_v2_to_v3(root, profile, None).await.unwrap();

        // 4. Verify results
        // 4.1 Monolithic renamed
        assert!(root.join(format!("{}.json.v2_bak", profile)).exists());
        assert!(!old_monolithic.exists());

        // 4.2 Config migrated to cfg/
        let store = FileStore::new(root, None).unwrap();
        let item: Item = store.load(profile, "key1").unwrap();
        assert_eq!(item.value, "val1");

        // 4.3 Tokens folder renamed
        assert!(profile_dir.join("tokens/access_token").exists());
        assert!(!profile_dir.join("tok_v2").exists());

        // 4.4 DLQ flattened
        assert!(profile_dir.join("dlq/msg1").exists());
    }
}
