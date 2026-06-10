use async_trait::async_trait;
use cowen_common::{CowenError, CowenResult};

use crate::sql::{migration_trait::SchemaMigration, SqlBuilder, SqlBuilderRegistration, SqlDriver};

use fs2::FileExt;
use sqlx::{Pool, Sqlite};
use std::fs::OpenOptions;
use std::sync::Arc;

#[async_trait]
impl SchemaMigration for SqliteDriver {
    async fn get_current_version(&self) -> CowenResult<u32> {
        // Create schema_migrations table if not exists
        sqlx::query("CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY)")
            .execute(&self.pool)
            .await
            .map_err(|e| CowenError::Store(e.to_string()))?;

        let row: Option<(i32,)> = sqlx::query_as("SELECT MAX(version) FROM schema_migrations")
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| CowenError::Store(e.to_string()))?;

        Ok(row.map_or(0, |r| r.0 as u32))
    }

    async fn apply_sql(&self, sql: &str) -> CowenResult<()> {
        sqlx::query(sql)
            .execute(&self.pool)
            .await
            .map_err(|e| CowenError::Store(format!("SQL apply error: {} ({})", e, sql)))?;
        Ok(())
    }

    async fn set_version(&self, version: u32) -> CowenResult<()> {
        sqlx::query("INSERT INTO schema_migrations (version) VALUES (?)")
            .bind(version as i32)
            .execute(&self.pool)
            .await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    fn get_migrations(&self) -> Vec<(u32, &'static str)> {
        vec![(
            1,
            "
                CREATE TABLE IF NOT EXISTS cowen_config (
                    profile TEXT NOT NULL,
                    item_key TEXT NOT NULL,
                    item_value TEXT NOT NULL,
                    version INTEGER DEFAULT 1,
                    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY (profile, item_key)
                );
                CREATE TABLE IF NOT EXISTS cowen_secrets (
                    profile TEXT NOT NULL,
                    item_key TEXT NOT NULL,
                    item_value TEXT NOT NULL,
                    version INTEGER DEFAULT 1,
                    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY (profile, item_key)
                );
                CREATE TABLE IF NOT EXISTS cowen_tokens (
                    profile TEXT NOT NULL,
                    item_key TEXT NOT NULL,
                    item_value TEXT NOT NULL,
                    expires_at DATETIME NOT NULL,
                    PRIMARY KEY (profile, item_key)
                );
                CREATE TABLE IF NOT EXISTS cowen_audit (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    profile TEXT NOT NULL,
                    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                    action TEXT NOT NULL,
                    status TEXT NOT NULL,
                    details TEXT
                );
                CREATE TABLE IF NOT EXISTS cowen_dlq (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    profile TEXT NOT NULL,
                    topic TEXT NOT NULL,
                    payload TEXT NOT NULL,
                    retry_count INTEGER DEFAULT 0,
                    error TEXT,
                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
                );
                CREATE INDEX IF NOT EXISTS idx_dlq_profile_topic ON cowen_dlq (profile, topic);
                CREATE INDEX IF NOT EXISTS idx_audit_profile ON cowen_audit (profile);
            ",
        )]
    }
}

pub struct SqliteDriver {
    pool: Pool<Sqlite>,
}

impl SqliteDriver {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

crate::define_sql_driver! {
    SqliteDriver,
    sqlx::Sqlite,
    false,
    "INSERT INTO cowen_config (profile, item_key, item_value, version) VALUES (?, ?, ?, 1) ON CONFLICT(profile, item_key) DO UPDATE SET item_value=excluded.item_value, version=cowen_config.version+1",
    "INSERT INTO cowen_secret (profile, item_key, item_value) VALUES (?, ?, ?) ON CONFLICT(profile, item_key) DO UPDATE SET item_value=excluded.item_value",
    "INSERT INTO cowen_token (profile, item_key, item_value, expires_at) VALUES (?, ?, ?, ?) ON CONFLICT(profile, item_key) DO UPDATE SET item_value=excluded.item_value, expires_at=excluded.expires_at",
    "INSERT INTO cowen_ticket (app_key, ticket_value, created_at) VALUES (?, ?, ?) ON CONFLICT(app_key) DO UPDATE SET ticket_value=excluded.ticket_value, created_at=excluded.created_at",
    "INSERT INTO cowen_app_token (app_key, token_value, expires_at, created_at) VALUES (?, ?, ?, ?) ON CONFLICT(app_key) DO UPDATE SET token_value=excluded.token_value, expires_at=excluded.expires_at, created_at=excluded.created_at",
    "INSERT INTO cowen_tenant_token (profile, token_value, expires_at, created_at, token_type) VALUES (?, ?, ?, ?, ?) ON CONFLICT(profile, token_type) DO UPDATE SET token_value=excluded.token_value, expires_at=excluded.expires_at, created_at=excluded.created_at",
    "INSERT INTO cowen_permanent_code (app_key, org_id, user_id, code_value, code_type) VALUES (?, ?, ?, ?, ?) ON CONFLICT(app_key, org_id, user_id, code_type) DO UPDATE SET code_value=excluded.code_value"
}

pub struct SqliteBuilder;

#[async_trait]
impl SqlBuilder for SqliteBuilder {
    fn scheme(&self) -> &str {
        "sqlite"
    }
    async fn build(&self, url: &str) -> CowenResult<Arc<dyn SqlDriver>> {
        static DDL_MUTEX: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
        let mutex = DDL_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()));
        let _ddl_guard = mutex.lock().await;

        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;

        let normalized_url = url.to_string();

        // 🚀 SYNC: Extract db_path from normalized URL to create parent dirs
        // URL is guaranteed to be sqlite:<path> from lib.rs
        let db_path = if let Some(stripped) = normalized_url.strip_prefix("sqlite:") {
            let pure_path = stripped.split('?').next().unwrap();
            std::path::PathBuf::from(pure_path)
        } else {
            std::path::PathBuf::from("cowen.db")
        };

        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                let _ = std::fs::create_dir_all(parent);
            }
        }

        // 🚀 CRITICAL: Use a file lock to ensure ONLY ONE process initializes SQLite and runs DDL at a time
        let lock_path = db_path.with_extension("ddl.lock");
        let lock_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&lock_path)
            .map_err(|e| CowenError::Store(e.to_string()))?;
        let _ = lock_file.lock_exclusive();

        let options = SqliteConnectOptions::from_str(&normalized_url)
            .map_err(|e| CowenError::Store(e.to_string()))?
            .create_if_missing(true)
            .busy_timeout(std::time::Duration::from_secs(5))
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|e| CowenError::Store(e.to_string()))?;

        let ddl = [
            "CREATE TABLE IF NOT EXISTS cowen_config (profile TEXT NOT NULL, item_key TEXT NOT NULL, item_value TEXT NOT NULL, version INTEGER DEFAULT 0, updated_at DATETIME DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (profile, item_key))",
            "CREATE TABLE IF NOT EXISTS cowen_secret (profile TEXT NOT NULL, item_key TEXT NOT NULL, item_value TEXT NOT NULL, updated_at DATETIME DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (profile, item_key))",
            "CREATE TABLE IF NOT EXISTS cowen_token (profile TEXT NOT NULL, item_key TEXT NOT NULL, item_value TEXT NOT NULL, expires_at DATETIME NULL, PRIMARY KEY (profile, item_key))",
            "CREATE TABLE IF NOT EXISTS cowen_ticket (app_key TEXT PRIMARY KEY, ticket_value TEXT NOT NULL, created_at DATETIME DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE IF NOT EXISTS cowen_app_token (app_key TEXT PRIMARY KEY, token_value TEXT NOT NULL, expires_at DATETIME NOT NULL, created_at DATETIME NOT NULL)",
            "CREATE TABLE IF NOT EXISTS cowen_tenant_token (profile TEXT NOT NULL, token_type TEXT NOT NULL, token_value TEXT NOT NULL, expires_at DATETIME NOT NULL, created_at DATETIME NOT NULL, PRIMARY KEY (profile, token_type))",
            "CREATE TABLE IF NOT EXISTS cowen_permanent_code (app_key TEXT NOT NULL, org_id TEXT NOT NULL, user_id TEXT DEFAULT '', code_type TEXT NOT NULL, code_value TEXT NOT NULL, created_at DATETIME DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (app_key, org_id, user_id, code_type))",
            "CREATE TABLE IF NOT EXISTS cowen_vault_secret (profile TEXT NOT NULL, secret_key TEXT NOT NULL, secret_value TEXT NOT NULL, PRIMARY KEY (profile, secret_key))",
            "CREATE TABLE IF NOT EXISTS cowen_audit (id TEXT PRIMARY KEY, profile TEXT NOT NULL, timestamp DATETIME NOT NULL, level TEXT NOT NULL, target TEXT NOT NULL, message TEXT NOT NULL, fields TEXT)",
            "CREATE TABLE IF NOT EXISTS cowen_dlq (id INTEGER PRIMARY KEY AUTOINCREMENT, profile TEXT NOT NULL, topic TEXT NOT NULL, payload TEXT NOT NULL, retry_count INTEGER DEFAULT 0, error TEXT, created_at DATETIME DEFAULT CURRENT_TIMESTAMP)",
        ];

        let mut last_err = None;
        for _i in 0..30 {
            let mut success = true;
            for sql in ddl {
                if let Err(e) = sqlx::query(sql).execute(&pool).await {
                    last_err = Some(e);
                    success = false;
                    break;
                }
            }
            if success {
                // Verify important tables exist before returning
                let verify_res = sqlx::query(
                    "SELECT name FROM sqlite_master WHERE type='table' AND name='cowen_secret'",
                )
                .fetch_one(&pool)
                .await;
                if verify_res.is_ok() {
                    let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_profile_ts ON cowen_audit (profile, timestamp)").execute(&pool).await;
                    let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_dlq_profile_topic ON cowen_dlq (profile, topic)").execute(&pool).await;
                    let _ = lock_file.unlock();
                    return Ok(Arc::new(SqliteDriver::new(pool)));
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        let _ = lock_file.unlock();
        Err(CowenError::Store(format!(
            "Failed to initialize SQLite DDL after retries: {:?}",
            last_err
        )))
    }
}

inventory::submit! { SqlBuilderRegistration { builder: &SqliteBuilder } }

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_concurrent_build() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_concurrent.db");
        let db_url = format!("sqlite://{}", db_path.to_string_lossy());

        let mut handles = vec![];
        for _ in 0..15 {
            let url = db_url.clone();
            let handle = tokio::spawn(async move { SqliteBuilder.build(&url).await });
            handles.push(handle);
        }

        let mut results = vec![];
        for handle in handles {
            results.push(handle.await.unwrap());
        }

        for res in results {
            assert!(
                res.is_ok(),
                "Expected Ok, got error: {:?}",
                res.as_ref().err()
            );
        }
    }
}
