use cowen_common::{CowenResult, CowenError, security};
use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Path, PathBuf};
use cowen_common::models::StoreItem;

pub struct FileStore {
    root_dir: PathBuf,
    fingerprint: Option<String>,
}

impl FileStore {
    pub fn new<P: AsRef<Path>>(root_dir: P, fingerprint: Option<&str>) -> CowenResult<Self> {
        let root_dir = root_dir.as_ref().to_path_buf();
        if !root_dir.exists() {
            fs::create_dir_all(&root_dir).map_err(|e| CowenError::Store(e.to_string()))?;
        }
        Ok(Self { 
            root_dir, 
            fingerprint: fingerprint.map(|s| s.to_string()) 
        })
    }

    pub fn get_path(&self, profile: &str, prefix: &str, id: &str, create: bool) -> PathBuf {
        let dir = self.root_dir.join(profile).join(prefix);
        if create && !dir.exists() {
            let _ = fs::create_dir_all(&dir);
        }
        dir.join(id.replace(":", "_"))
    }

    pub fn save<T: StoreItem>(&self, profile: &str, id: &str, item: &T) -> CowenResult<()> {
        let path = self.get_path(profile, T::key_prefix(), id, true);
        let temp_path = path.with_extension("tmp");
        
        let json = serde_json::to_string(item).map_err(|e| CowenError::Store(e.to_string()))?;
        let data = if let Some(fp) = &self.fingerprint {
            let key = security::derive_key(fp);
            security::encrypt(json.as_bytes(), &key).map_err(|e| CowenError::Store(e.to_string()))?
        } else {
            json.into_bytes()
        };

        cowen_infra::sys::fs::secure_open_write(&temp_path)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(&data)
            })
            .map_err(|e| CowenError::Store(e.to_string()))?;
        fs::rename(temp_path, path).map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    pub fn load<T: StoreItem + DeserializeOwned>(&self, profile: &str, id: &str) -> CowenResult<T> {
        let path = self.get_path(profile, T::key_prefix(), id, false);
        let content = fs::read(path).map_err(|e| CowenError::Store(e.to_string()))?;
        
        let json_bytes = if let Some(fp) = &self.fingerprint {
            let key = security::derive_key(fp);
            security::decrypt(&content, &key).map_err(|e| CowenError::Store(e.to_string()))?
        } else {
            content
        };

        let json = String::from_utf8(json_bytes).map_err(|e| CowenError::Store(e.to_string()))?;
        serde_json::from_str(&json).map_err(|e| CowenError::Store(e.to_string()))
    }

    pub fn delete<T: StoreItem>(&self, profile: &str, id: &str) -> CowenResult<()> {
        let path = self.get_path(profile, T::key_prefix(), id, false);
        if path.exists() {
            fs::remove_file(path).map_err(|e| CowenError::Store(e.to_string()))?;
        }
        Ok(())
    }

    pub fn list<T: StoreItem>(&self, profile: &str) -> CowenResult<Vec<String>> {
        let dir = self.root_dir.join(profile).join(T::key_prefix());
        if !dir.exists() { return Ok(vec![]); }
        
        let mut ids = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    ids.push(name.to_string());
                }
            }
        }
        Ok(ids)
    }

    pub fn list_all_paged<T: StoreItem + DeserializeOwned>(&self, profile: &str, offset: usize, limit: usize) -> CowenResult<Vec<T>> {
        let ids = self.list::<T>(profile)?;
        let mut items = Vec::new();
        for id in ids.into_iter().skip(offset).take(limit) {
            if let Ok(item) = self.load::<T>(profile, &id) {
                items.push(item);
            }
        }
        Ok(items)
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn fingerprint(&self) -> Option<&str> {
        self.fingerprint.as_deref()
    }
}
