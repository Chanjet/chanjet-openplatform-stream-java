use anyhow::Result;
use sqlx::{sqlite::SqliteConnectOptions, ConnectOptions, SqlitePool};
use std::path::Path;
use std::time::Duration;
use tracing::info;

#[derive(Debug, Clone)]
pub struct TelemetryDb {
    pool: SqlitePool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct TelemetryEventRecord {
    pub id: i64,
    pub profile: String,
    pub event_type: String,
    pub old_status: Option<String>,
    pub new_status: Option<String>,
    pub details: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl TelemetryDb {
    pub async fn new(db_path: &Path) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .busy_timeout(Duration::from_secs(5))
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .log_statements(log::LevelFilter::Trace);

        let pool = SqlitePool::connect_with(options).await?;

        // Initialize schema
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS telemetry_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                profile TEXT NOT NULL,
                event_type TEXT NOT NULL,
                old_status TEXT,
                new_status TEXT,
                details TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS idx_telemetry_events_profile ON telemetry_events(profile);
            CREATE INDEX IF NOT EXISTS idx_telemetry_events_created_at ON telemetry_events(created_at);"
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    pub async fn insert_event(
        &self,
        profile: &str,
        event_type: &str,
        old_status: Option<&str>,
        new_status: Option<&str>,
        details: Option<&str>,
    ) -> Result<()> {
        // Retry logic for SQLite Busy: up to 10 times with 10-50ms jitter
        use rand::Rng;
        let mut retries = 0;
        loop {
            let res = sqlx::query(
                "INSERT INTO telemetry_events (profile, event_type, old_status, new_status, details)
                 VALUES (?, ?, ?, ?, ?)"
            )
            .bind(profile)
            .bind(event_type)
            .bind(old_status)
            .bind(new_status)
            .bind(details)
            .execute(&self.pool)
            .await;

            match res {
                Ok(_) => return Ok(()),
                Err(sqlx::Error::Database(e))
                    if e.message().contains("database is locked") && retries < 10 =>
                {
                    retries += 1;
                    let delay = rand::thread_rng().gen_range(10..=50);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    pub async fn list_events(
        &self,
        profile: Option<&str>,
        limit: i64,
    ) -> Result<Vec<TelemetryEventRecord>> {
        let query = if let Some(p) = profile {
            sqlx::query_as(
                "SELECT id, profile, event_type, old_status, new_status, details, created_at
                 FROM telemetry_events WHERE profile = ? ORDER BY created_at DESC LIMIT ?",
            )
            .bind(p)
            .bind(limit)
        } else {
            sqlx::query_as(
                "SELECT id, profile, event_type, old_status, new_status, details, created_at
                 FROM telemetry_events ORDER BY created_at DESC LIMIT ?",
            )
            .bind(limit)
        };

        let records = query.fetch_all(&self.pool).await?;
        Ok(records)
    }

    pub async fn run_gc(&self) -> Result<()> {
        // 1. Delete events older than 15 days
        let deleted_by_time =
            sqlx::query("DELETE FROM telemetry_events WHERE created_at < date('now', '-15 days')")
                .execute(&self.pool)
                .await?;

        // 2. Keep only latest 10000 events
        let deleted_by_count = sqlx::query(
            "DELETE FROM telemetry_events 
             WHERE id NOT IN (SELECT id FROM telemetry_events ORDER BY id DESC LIMIT 10000)",
        )
        .execute(&self.pool)
        .await?;

        if deleted_by_time.rows_affected() > 0 || deleted_by_count.rows_affected() > 0 {
            info!(
                target: "sys",
                "Telemetry GC: removed {} by time, {} by count",
                deleted_by_time.rows_affected(),
                deleted_by_count.rows_affected()
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_telemetry_concurrent_insertion() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("telemetry.db");

        let db = TelemetryDb::new(&db_path).await.unwrap();

        let mut handles = vec![];
        let db_arc = std::sync::Arc::new(db);
        for i in 0..50 {
            let db_clone = db_arc.clone();
            let handle = tokio::spawn(async move {
                db_clone
                    .insert_event(
                        "profile_test",
                        "type_test",
                        Some("old"),
                        Some("new"),
                        Some(&format!("detail {}", i)),
                    )
                    .await
            });
            handles.push(handle);
        }

        let mut results = vec![];
        for handle in handles {
            results.push(handle.await.unwrap());
        }

        for res in results {
            assert!(res.is_ok(), "Expected Ok, got error: {:?}", res);
        }
    }
}
