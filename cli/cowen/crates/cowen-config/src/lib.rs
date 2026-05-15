use std::path::PathBuf;
use tokio::sync::watch;
use serde::Deserialize;
use anyhow::{Result, Context};
use notify::{Watcher, RecursiveMode, Event, EventKind};
use tracing::{info, error};

pub struct ConfigWatcher<T> {
    receiver: watch::Receiver<T>,
    // We need to keep the watcher alive
    _watcher: Box<dyn Watcher + Send + Sync>,
}

impl<T> ConfigWatcher<T> 
where 
    T: for<'de> Deserialize<'de> + Send + Sync + 'static + Clone + std::fmt::Debug
{
    pub fn new(path: PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read initial config from {:?}", path))?;
        let config: T = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse initial config")?;

        let (tx, rx) = watch::channel(config);
        
        // 1. File Watcher
        let path_clone = path.clone();
        let tx_file = tx.clone();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            match res {
                Ok(event) => {
                    if let EventKind::Modify(_) = event.kind {
                        info!("Config file modified, reloading...");
                        Self::reload(&path_clone, &tx_file);
                    }
                }
                Err(e) => error!("Watcher error: {}", e),
            }
        })?;
        watcher.watch(&path, RecursiveMode::NonRecursive)?;

        // 2. SIGHUP Listener (Unix only)
        #[cfg(unix)]
        {
            let path_sighup = path.clone();
            let tx_sighup = tx.clone();
            tokio::spawn(async move {
                use tokio::signal::unix::{signal, SignalKind};
                let mut stream = match signal(SignalKind::hangup()) {
                    Ok(s) => s,
                    Err(e) => {
                        error!("Failed to register SIGHUP handler: {}", e);
                        return;
                    }
                };
                while stream.recv().await.is_some() {
                    info!("Received SIGHUP, reloading config...");
                    Self::reload(&path_sighup, &tx_sighup);
                }
            });
        }

        Ok(Self {
            receiver: rx,
            _watcher: Box::new(watcher),
        })
    }

    fn reload(path: &PathBuf, tx: &watch::Sender<T>) {
        match std::fs::read_to_string(path) {
            Ok(new_content) => {
                match serde_yaml::from_str::<T>(&new_content) {
                    Ok(new_config) => {
                        info!("Config reloaded successfully: {:?}", new_config);
                        if let Err(e) = tx.send(new_config) {
                            error!("Failed to send new config to watch channel: {}", e);
                        }
                    }
                    Err(e) => error!("Failed to parse reloaded config: {}", e),
                }
            }
            Err(e) => error!("Failed to read reloaded config file: {}", e),
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<T> {
        self.receiver.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use tokio::time::{sleep, Duration};
    use serde::Serialize;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestConfig {
        name: String,
        value: i32,
    }

    #[tokio::test]
    async fn test_config_hot_reload() -> Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("config.yaml");
        
        let initial_config = TestConfig { name: "initial".into(), value: 1 };
        fs::write(&config_path, serde_yaml::to_string(&initial_config)?)?;

        let watcher = ConfigWatcher::<TestConfig>::new(config_path.clone())?;
        let mut rx = watcher.subscribe();
        
        assert_eq!(*rx.borrow(), initial_config);

        // Update config
        let updated_config = TestConfig { name: "updated".into(), value: 2 };
        fs::write(&config_path, serde_yaml::to_string(&updated_config)?)?;

        // Give notify some time to pick up the change
        let mut success = false;
        for _ in 0..20 {
            if *rx.borrow() == updated_config {
                success = true;
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }

        assert!(success, "Config was not updated within timeout. Current: {:?}", *rx.borrow());
        
        Ok(())
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_config_sighup_reload() -> Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("config.yaml");
        
        let initial_config = TestConfig { name: "initial".into(), value: 1 };
        fs::write(&config_path, serde_yaml::to_string(&initial_config)?)?;

        let watcher = ConfigWatcher::<TestConfig>::new(config_path.clone())?;
        let mut rx = watcher.subscribe();
        
        assert_eq!(*rx.borrow(), initial_config);

        // Update config file but don't wait for notify (notify might trigger it anyway, but we want to test SIGHUP)
        let updated_config = TestConfig { name: "sighup".into(), value: 3 };
        fs::write(&config_path, serde_yaml::to_string(&updated_config)?)?;

        // Send SIGHUP to self
        unsafe {
            libc::kill(libc::getpid(), libc::SIGHUP);
        }

        let mut success = false;
        for _ in 0..20 {
            if *rx.borrow() == updated_config {
                success = true;
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }

        assert!(success, "Config was not updated via SIGHUP within timeout. Current: {:?}", *rx.borrow());
        
        Ok(())
    }
}
