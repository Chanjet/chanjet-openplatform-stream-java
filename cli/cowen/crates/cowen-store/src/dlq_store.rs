#[cfg(feature = "sqlite")]
pub mod sqlite {
    use anyhow::{anyhow, Result};
    use connector_sdk::dlq::DlqProvider;
    use sqlx::{SqlitePool, Row};
    use std::path::Path;

    pub struct SqliteDlqProvider {
        pool: SqlitePool,
    }

    impl SqliteDlqProvider {
        pub async fn new(db_path: &Path) -> Result<Self> {
            let path_str = db_path.to_string_lossy();
            let url = format!("sqlite:{}?mode=rwc", path_str);
            let pool = SqlitePool::connect(&url).await.map_err(|e| anyhow!("Failed to connect to DLQ sqlite: {}", e))?;

            // Initialize table
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS dlq_messages (
                    msg_id TEXT PRIMARY KEY,
                    payload TEXT NOT NULL,
                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
                );
                "#
            )
            .execute(&pool)
            .await
            .map_err(|e| anyhow!("Failed to create dlq_messages table: {}", e))?;

            Ok(Self { pool })
        }
    }

    #[async_trait::async_trait]
    impl DlqProvider for SqliteDlqProvider {
        async fn store(&self, msg_id: &str, payload: &str) -> Result<()> {
            sqlx::query(
                "INSERT INTO dlq_messages (msg_id, payload) VALUES (?, ?)"
            )
            .bind(msg_id)
            .bind(payload)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow!("Failed to insert into DLQ: {}", e))?;
            
            Ok(())
        }

        async fn remove(&self, msg_id: &str) -> Result<()> {
            sqlx::query(
                "DELETE FROM dlq_messages WHERE msg_id = ?"
            )
            .bind(msg_id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow!("Failed to delete from DLQ: {}", e))?;

            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use tempfile::tempdir;

        #[tokio::test]
        async fn test_sqlite_dlq_provider() -> Result<()> {
            let temp_dir = tempdir()?;
            let db_path = temp_dir.path().join("dlq.db");

            let provider = SqliteDlqProvider::new(&db_path).await?;
            
            // Test store
            let msg_id = "test-msg-123";
            let payload = "{\"msg_type\":\"event\"}";
            provider.store(msg_id, payload).await?;

            // Verify it was stored
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM dlq_messages WHERE msg_id = ?")
                .bind(msg_id)
                .fetch_one(&provider.pool)
                .await?;
            assert_eq!(count, 1);

            // Test remove
            provider.remove(msg_id).await?;

            // Verify it was removed
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM dlq_messages WHERE msg_id = ?")
                .bind(msg_id)
                .fetch_one(&provider.pool)
                .await?;
            assert_eq!(count, 0);

            Ok(())
        }
    }
}
