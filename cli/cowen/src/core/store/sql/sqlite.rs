use anyhow::Result;
use async_trait::async_trait;
use sqlx::{Sqlite, Pool};
use std::sync::Arc;
use super::{SqlDriver, SqlBuilder};

pub struct SqliteDriver {
    pool: Pool<Sqlite>,
}

impl SqliteDriver {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SqlDriver for SqliteDriver {
    // --- Config Domain ---
    async fn get_config(&self, profile: &str, key: &str) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_config WHERE profile = ? AND item_key = ?")
            .bind(profile).bind(key).fetch_one(&self.pool).await?;
        Ok(row.0)
    }
    async fn get_config_full(&self, profile: &str, key: &str) -> Result<super::super::Item> {
        let row: (String, String, String, i64, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
            "SELECT profile, item_key, item_value, version, updated_at FROM cowen_config WHERE profile = ? AND item_key = ?"
        ).bind(profile).bind(key).fetch_one(&self.pool).await?;
        Ok(super::super::Item {
            profile: row.0,
            key: row.1,
            value: row.2,
            version: row.3 as u64,
            updated_at: row.4.timestamp(),
        })
    }
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()> {
        sqlx::query("INSERT INTO cowen_config (profile, item_key, item_value, version) VALUES (?, ?, ?, 0) ON CONFLICT(profile, item_key) DO UPDATE SET item_value = excluded.item_value, version = version + 1")
            .bind(profile).bind(key).bind(value).execute(&self.pool).await?;
        Ok(())
    }
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> Result<()> {
        if expected_version == 0 {
            let res = sqlx::query("INSERT INTO cowen_config (profile, item_key, item_value, version) VALUES (?, ?, ?, 1) ON CONFLICT(profile, item_key) DO NOTHING")
                .bind(profile).bind(key).bind(value).execute(&self.pool).await?;
            if res.rows_affected() == 0 {
                return Err(anyhow::anyhow!("Conflict: Config has been modified by another node (expected version 0, but found different)"));
            }
            return Ok(());
        }

        let res = sqlx::query("UPDATE cowen_config SET item_value = ?, version = version + 1 WHERE profile = ? AND item_key = ? AND version = ?")
            .bind(value).bind(profile).bind(key).bind(expected_version as i64).execute(&self.pool).await?;
        if res.rows_affected() == 0 {
            return Err(anyhow::anyhow!("Conflict: Config has been modified by another node (expected version {}, but found different)", expected_version));
        }
        Ok(())
    }
    async fn list_configs(&self, profile: &str) -> Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>("SELECT item_key FROM cowen_config WHERE profile = ?")
            .bind(profile).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }
    async fn delete_config(&self, profile: &str, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM cowen_config WHERE profile = ? AND item_key = ?")
            .bind(profile).bind(key).execute(&self.pool).await?;
        Ok(())
    }

    // --- Secret Domain ---
    async fn get_secret(&self, profile: &str, key: &str) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_secret WHERE profile = ? AND item_key = ?")
            .bind(profile).bind(key).fetch_one(&self.pool).await?;
        Ok(row.0)
    }
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()> {
        sqlx::query("INSERT INTO cowen_secret (profile, item_key, item_value) VALUES (?, ?, ?) ON CONFLICT(profile, item_key) DO UPDATE SET item_value = excluded.item_value")
            .bind(profile).bind(key).bind(value).execute(&self.pool).await?;
        Ok(())
    }
    async fn delete_secret(&self, profile: &str, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM cowen_secret WHERE profile = ? AND item_key = ?")
            .bind(profile).bind(key).execute(&self.pool).await?;
        Ok(())
    }
    async fn list_secrets(&self, profile: &str) -> Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>("SELECT item_key FROM cowen_secret WHERE profile = ?")
            .bind(profile).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    // --- Token Domain ---
    async fn get_access_token(&self, profile: &str) -> Result<crate::auth::models::Token> {
        let row: (String, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
            "SELECT token_value, expires_at, created_at FROM cowen_tenant_token WHERE profile = ? AND token_type = 'access'"
        ).bind(profile).fetch_one(&self.pool).await?;
        
        Ok(crate::auth::models::Token {
            value: row.0,
            expires_at: row.1,
            created_at: row.2,
        })
    }
    async fn save_access_token(&self, profile: &str, token: crate::auth::models::Token) -> Result<()> {
        sqlx::query("INSERT INTO cowen_tenant_token (profile, token_type, token_value, expires_at, created_at) VALUES (?, 'access', ?, ?, ?) ON CONFLICT(profile, token_type) DO UPDATE SET token_value = excluded.token_value, expires_at = excluded.expires_at, created_at = excluded.created_at")
            .bind(profile).bind(&token.value).bind(token.expires_at).bind(token.created_at).execute(&self.pool).await?;
        Ok(())
    }
    async fn delete_access_token(&self, profile: &str) -> Result<()> {
        sqlx::query("DELETE FROM cowen_tenant_token WHERE profile = ? AND token_type = 'access'")
            .bind(profile).execute(&self.pool).await?;
        Ok(())
    }
    async fn get_app_access_token(&self, app_key: &str) -> Result<crate::auth::models::Token> {
        let row: (String, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
            "SELECT token_value, expires_at, created_at FROM cowen_app_token WHERE app_key = ?"
        ).bind(app_key).fetch_one(&self.pool).await?;
        
        Ok(crate::auth::models::Token {
            value: row.0,
            expires_at: row.1,
            created_at: row.2,
        })
    }
    async fn save_app_access_token(&self, app_key: &str, token: crate::auth::models::Token) -> Result<()> {
        sqlx::query("INSERT INTO cowen_app_token (app_key, token_value, expires_at, created_at) VALUES (?, ?, ?, ?) ON CONFLICT(app_key) DO UPDATE SET token_value = excluded.token_value, expires_at = excluded.expires_at, created_at = excluded.created_at")
            .bind(app_key).bind(&token.value).bind(token.expires_at).bind(token.created_at).execute(&self.pool).await?;
        Ok(())
    }

    // --- Ticket Domain ---
    async fn get_app_ticket(&self, app_key: &str) -> Result<crate::auth::models::Ticket> {
        let row: (String, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
            "SELECT ticket_value, created_at FROM cowen_ticket WHERE app_key = ?"
        ).bind(app_key).fetch_one(&self.pool).await?;
        
        Ok(crate::auth::models::Ticket {
            value: row.0,
            created_at: row.1,
        })
    }
    async fn save_app_ticket(&self, app_key: &str, ticket: crate::auth::models::Ticket) -> Result<()> {
        sqlx::query("INSERT INTO cowen_ticket (app_key, ticket_value, created_at) VALUES (?, ?, ?) ON CONFLICT(app_key) DO UPDATE SET ticket_value = excluded.ticket_value, created_at = excluded.created_at")
            .bind(app_key).bind(&ticket.value).bind(ticket.created_at).execute(&self.pool).await?;
        Ok(())
    }

    // --- Permanent Code Domain ---
    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT code_value FROM cowen_permanent_code WHERE app_key = ? AND org_id = ? AND user_id = '' AND code_type = 'org_permanent'")
            .bind(app_key).bind(org_id).fetch_one(&self.pool).await?;
        Ok(row.0)
    }
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> Result<()> {
        sqlx::query("INSERT INTO cowen_permanent_code (app_key, org_id, user_id, code_type, code_value) VALUES (?, ?, '', 'org_permanent', ?) ON CONFLICT(app_key, org_id, user_id, code_type) DO UPDATE SET code_value = excluded.code_value")
            .bind(app_key).bind(org_id).bind(code).execute(&self.pool).await?;
        Ok(())
    }
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT code_value FROM cowen_permanent_code WHERE app_key = ? AND org_id = ? AND user_id = ? AND code_type = 'user_permanent'")
            .bind(app_key).bind(org_id).bind(user_id).fetch_one(&self.pool).await?;
        Ok(row.0)
    }
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> Result<()> {
        sqlx::query("INSERT INTO cowen_permanent_code (app_key, org_id, user_id, code_type, code_value) VALUES (?, ?, ?, 'user_permanent', ?) ON CONFLICT(app_key, org_id, user_id, code_type) DO UPDATE SET code_value = excluded.code_value")
            .bind(app_key).bind(org_id).bind(user_id).bind(code).execute(&self.pool).await?;
        Ok(())
    }

    // --- Legacy Support ---
    async fn get_token(&self, profile: &str, key: &str) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_token WHERE profile = ? AND item_key = ? AND (expires_at IS NULL OR expires_at > datetime('now'))")
            .bind(profile).bind(key).fetch_one(&self.pool).await?;
        Ok(row.0)
    }
    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> Result<()> {
        sqlx::query("INSERT INTO cowen_token (profile, item_key, item_value, expires_at) VALUES (?, ?, ?, datetime('now', '+' || ? || ' seconds')) ON CONFLICT(profile, item_key) DO UPDATE SET item_value = excluded.item_value, expires_at = excluded.expires_at")
            .bind(profile).bind(key).bind(value).bind(expires_in_secs as i64).execute(&self.pool).await?;
        Ok(())
    }
    async fn delete_token(&self, profile: &str, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM cowen_token WHERE profile = ? AND item_key = ?")
            .bind(profile).bind(key).execute(&self.pool).await?;
        Ok(())
    }
    async fn list_tokens(&self, profile: &str) -> Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>("SELECT item_key FROM cowen_token WHERE profile = ? AND (expires_at IS NULL OR expires_at > datetime('now'))")
            .bind(profile).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    // --- Audit Domain ---
    async fn save_audit(&self, entry: &super::super::AuditEntry) -> Result<()> {
        sqlx::query("INSERT INTO cowen_audit (id, profile, timestamp, level, target, message, fields) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&entry.id).bind(&entry.profile).bind(entry.timestamp).bind(&entry.level).bind(&entry.target).bind(&entry.message).bind(serde_json::to_string(&entry.fields)?).execute(&self.pool).await?;
        Ok(())
    }
    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<super::super::AuditEntry>> {
        let rows = sqlx::query_as::<_, (String, chrono::DateTime<chrono::Utc>, String, String, String, String, String)>(
            "SELECT id, timestamp, profile, level, target, message, fields FROM cowen_audit WHERE profile = ? ORDER BY timestamp DESC LIMIT ?"
        ).bind(profile).bind(limit as i64).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| super::super::AuditEntry {
            id: r.0, timestamp: r.1, profile: r.2, level: r.3, target: r.4, message: r.5, fields: serde_json::from_str(&r.6).unwrap_or_default()
        }).collect())
    }

    // --- DLQ Domain ---
    async fn push_dlq(&self, msg: &super::super::DlqMessage) -> Result<()> {
        sqlx::query("INSERT INTO cowen_dlq (profile, topic, payload, retry_count, error) VALUES (?, ?, ?, ?, ?)")
            .bind(&msg.profile).bind(&msg.topic).bind(&msg.payload).bind(msg.retry_count).bind(&msg.error).execute(&self.pool).await?;
        Ok(())
    }
    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<super::super::DlqMessage>> {
        let row = sqlx::query_as::<_, (i64, String, String, String, i32, Option<String>, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = ? AND topic = ? ORDER BY id ASC LIMIT 1"
        ).bind(profile).bind(topic).fetch_optional(&self.pool).await?;
        if let Some(r) = row {
            sqlx::query("DELETE FROM cowen_dlq WHERE id = ?").bind(r.0).execute(&self.pool).await?;
            Ok(Some(super::super::DlqMessage { id: Some(r.0), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6 }))
        } else { Ok(None) }
    }
    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<super::super::DlqMessage>> {
        let rows = sqlx::query_as::<_, (i64, String, String, String, i32, Option<String>, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = ? ORDER BY id DESC LIMIT ?"
        ).bind(profile).bind(limit as i64).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| super::super::DlqMessage {
            id: Some(r.0), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }).collect())
    }
    async fn list_all_dlq(&self, profile: &str) -> Result<Vec<super::super::DlqMessage>> {
        let rows = sqlx::query_as::<_, (i64, String, String, String, i32, Option<String>, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = ?"
        ).bind(profile).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| super::super::DlqMessage {
            id: Some(r.0), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }).collect())
    }


    // --- Management ---
    async fn clear_profile(&self, profile: &str) -> Result<()> {
        for table in ["cowen_config", "cowen_secret", "cowen_token", "cowen_tenant_token", "cowen_audit", "cowen_dlq"] {
            let sql = format!("DELETE FROM {} WHERE profile = ?", table);
            sqlx::query(&sql).bind(profile).execute(&self.pool).await?;
        }
        Ok(())
    }
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<()> {
        for table in ["cowen_config", "cowen_secret", "cowen_token", "cowen_tenant_token", "cowen_audit", "cowen_dlq"] {
            let sql = format!("UPDATE {} SET profile = ? WHERE profile = ?", table);
            sqlx::query(&sql).bind(new_name).bind(old_name).execute(&self.pool).await?;
        }
        Ok(())
    }
    async fn list_all_profiles(&self) -> Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>("SELECT DISTINCT profile FROM cowen_config")
            .fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

}

pub struct SqliteBuilder;
#[async_trait]
impl SqlBuilder for SqliteBuilder {
    fn scheme(&self) -> &str { "sqlite" }
    async fn build(&self, url: &str) -> Result<Arc<dyn SqlDriver>> {
        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;

        let options = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .busy_timeout(std::time::Duration::from_secs(5))
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        let pool = sqlx::SqlitePool::connect_with(options).await?;
        
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

        for sql in ddl { sqlx::query(sql).execute(&pool).await?; }
        
        // Indices
        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_profile_ts ON cowen_audit (profile, timestamp)").execute(&pool).await;
        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_dlq_profile_topic ON cowen_dlq (profile, topic)").execute(&pool).await;

        Ok(Arc::new(SqliteDriver::new(pool)))
    }
}

inventory::submit! { super::SqlBuilderRegistration { builder: &SqliteBuilder } }
