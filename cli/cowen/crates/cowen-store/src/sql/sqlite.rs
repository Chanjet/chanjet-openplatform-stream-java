#![cfg(feature = "sqlite")]
use cowen_common::CowenResult;
use async_trait::async_trait;

use sqlx::{Sqlite, Pool};
use std::sync::Arc;
use crate::sql::{SqlDriver, SqlBuilder, SqlBuilderRegistration};
use cowen_common::models::{Token, Ticket, Item, AuditEntry, DlqMessage};
use chrono::{DateTime, Utc};

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
    async fn get_config(&self, profile: &str, key: &str) -> CowenResult<String> {
        let row: (String,) = sqlx::query_as("SELECT value FROM cowen_config WHERE profile = ? AND key = ?")
            .bind(profile)
            .bind(key)
            .fetch_one(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(row.0)
    }

    async fn get_config_metadata(&self, profile: &str, key: &str) -> CowenResult<(u64, i64)> {
        let row: (i64, i64) = sqlx::query_as("SELECT version, updated_at FROM cowen_config WHERE profile = ? AND key = ?")
            .bind(profile)
            .bind(key)
            .fetch_one(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok((row.0 as u64, row.1))
    }

    async fn get_config_full(&self, profile: &str, key: &str) -> CowenResult<Item> {
        let row: (String, String, String, i64, i64) = sqlx::query_as("SELECT profile, key, value, version, updated_at FROM cowen_config WHERE profile = ? AND key = ?")
            .bind(profile)
            .bind(key)
            .fetch_one(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(Item {
            profile: row.0,
            key: row.1,
            value: row.2,
            version: row.3 as u64,
            updated_at: row.4,
        })
    }

    async fn set_config(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> {
        let now = Utc::now().timestamp();
        sqlx::query("INSERT INTO cowen_config (profile, key, value, version, updated_at) VALUES (?, ?, ?, 1, ?) 
                     ON CONFLICT(profile, key) DO UPDATE SET value=excluded.value, version=version+1, updated_at=excluded.updated_at")
            .bind(profile).bind(key).bind(value).bind(now)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(())
    }

    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> CowenResult<()> {
        let now = Utc::now().timestamp();
        let res = sqlx::query("UPDATE cowen_config SET value = ?, version = version + 1, updated_at = ? WHERE profile = ? AND key = ? AND version = ?")
            .bind(value).bind(now).bind(profile).bind(key).bind(expected_version as i64)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        
        if res.rows_affected() == 0 {
            return Err(anyhow::anyhow!("CAS failed: version mismatch or record not found").into());
        }
        Ok(())
    }

    async fn list_configs(&self, profile: &str) -> CowenResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT key FROM cowen_config WHERE profile = ?")
            .bind(profile)
            .fetch_all(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn delete_config(&self, profile: &str, key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_config WHERE profile = ? AND key = ?")
            .bind(profile).bind(key)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(())
    }

    async fn get_secret(&self, profile: &str, key: &str) -> CowenResult<String> {
        let row: (String,) = sqlx::query_as("SELECT value FROM cowen_secret WHERE profile = ? AND key = ?")
            .bind(profile).bind(key)
            .fetch_one(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(row.0)
    }

    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> {
        let now = Utc::now().timestamp();
        sqlx::query("INSERT INTO cowen_secret (profile, key, value, updated_at) VALUES (?, ?, ?, ?) 
                     ON CONFLICT(profile, key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at")
            .bind(profile).bind(key).bind(value).bind(now)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(())
    }

    async fn delete_secret(&self, profile: &str, key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_secret WHERE profile = ? AND key = ?")
            .bind(profile).bind(key)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(())
    }

    async fn list_secrets(&self, profile: &str) -> CowenResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT key FROM cowen_secret WHERE profile = ?")
            .bind(profile)
            .fetch_all(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn get_access_token(&self, profile: &str) -> CowenResult<Token> {
        let row: (String, DateTime<Utc>, DateTime<Utc>) = sqlx::query_as("SELECT value, expires_at, created_at FROM cowen_token WHERE profile = ? AND key = 'access_token'")
            .bind(profile)
            .fetch_one(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(Token { value: row.0, expires_at: row.1, created_at: row.2 })
    }

    async fn save_access_token(&self, profile: &str, token: Token) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_token (profile, key, value, expires_at, created_at) VALUES (?, 'access_token', ?, ?, ?) 
                     ON CONFLICT(profile, key) DO UPDATE SET value=excluded.value, expires_at=excluded.expires_at, created_at=excluded.created_at")
            .bind(profile).bind(token.value).bind(token.expires_at).bind(token.created_at)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(())
    }

    async fn delete_access_token(&self, profile: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_token WHERE profile = ? AND key = 'access_token'")
            .bind(profile)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(())
    }

    async fn get_app_access_token(&self, app_key: &str) -> CowenResult<Token> {
        let key = format!("app_token:{}", app_key);
        let row: (String, DateTime<Utc>, DateTime<Utc>) = sqlx::query_as("SELECT value, expires_at, created_at FROM cowen_token WHERE profile = 'global' AND key = ?")
            .bind(&key)
            .fetch_one(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(Token { value: row.0, expires_at: row.1, created_at: row.2 })
    }

    async fn save_app_access_token(&self, app_key: &str, token: Token) -> CowenResult<()> {
        let key = format!("app_token:{}", app_key);
        sqlx::query("INSERT INTO cowen_token (profile, key, value, expires_at, created_at) VALUES ('global', ?, ?, ?, ?) 
                     ON CONFLICT(profile, key) DO UPDATE SET value=excluded.value, expires_at=excluded.expires_at, created_at=excluded.created_at")
            .bind(&key).bind(token.value).bind(token.expires_at).bind(token.created_at)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(())
    }

    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<Ticket> {
        let key = format!("app_ticket:{}", app_key);
        let row: (String, DateTime<Utc>) = sqlx::query_as("SELECT value, created_at FROM cowen_token WHERE profile = 'global' AND key = ?")
            .bind(&key)
            .fetch_one(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(Ticket { value: row.0, created_at: row.1 })
    }

    async fn save_app_ticket(&self, app_key: &str, ticket: Ticket) -> CowenResult<()> {
        let key = format!("app_ticket:{}", app_key);
        sqlx::query("INSERT INTO cowen_token (profile, key, value, created_at) VALUES ('global', ?, ?, ?) 
                     ON CONFLICT(profile, key) DO UPDATE SET value=excluded.value, created_at=excluded.created_at")
            .bind(&key).bind(ticket.value).bind(ticket.created_at)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(())
    }

    async fn delete_app_ticket(&self, app_key: &str) -> CowenResult<()> {
        let key = format!("app_ticket:{}", app_key);
        sqlx::query("DELETE FROM cowen_token WHERE profile = 'global' AND key = ?")
            .bind(&key)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(())
    }

    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> CowenResult<String> {
        let key = format!("opc:{}:{}", app_key, org_id);
        let row: (String,) = sqlx::query_as("SELECT value FROM cowen_token WHERE profile = 'global' AND key = ?")
            .bind(&key)
            .fetch_one(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(row.0)
    }

    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> CowenResult<()> {
        let key = format!("opc:{}:{}", app_key, org_id);
        sqlx::query("INSERT INTO cowen_token (profile, key, value) VALUES ('global', ?, ?) 
                     ON CONFLICT(profile, key) DO UPDATE SET value=excluded.value")
            .bind(&key).bind(code)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(())
    }

    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> CowenResult<String> {
        let key = format!("upc:{}:{}:{}", app_key, org_id, user_id);
        let row: (String,) = sqlx::query_as("SELECT value FROM cowen_token WHERE profile = 'global' AND key = ?")
            .bind(&key)
            .fetch_one(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(row.0)
    }

    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> CowenResult<()> {
        let key = format!("upc:{}:{}:{}", app_key, org_id, user_id);
        sqlx::query("INSERT INTO cowen_token (profile, key, value) VALUES ('global', ?, ?) 
                     ON CONFLICT(profile, key) DO UPDATE SET value=excluded.value")
            .bind(&key).bind(code)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(())
    }

    async fn get_token(&self, profile: &str, key: &str) -> CowenResult<String> {
        let row: (String,) = sqlx::query_as("SELECT value FROM cowen_token WHERE profile = ? AND key = ?")
            .bind(profile).bind(key)
            .fetch_one(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(row.0)
    }

    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> CowenResult<()> {
        let exp = Utc::now() + chrono::Duration::seconds(expires_in_secs as i64);
        sqlx::query("INSERT INTO cowen_token (profile, key, value, expires_at) VALUES (?, ?, ?, ?) 
                     ON CONFLICT(profile, key) DO UPDATE SET value=excluded.value, expires_at=excluded.expires_at")
            .bind(profile).bind(key).bind(value).bind(exp)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(())
    }

    async fn delete_token(&self, profile: &str, key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_token WHERE profile = ? AND key = ?")
            .bind(profile).bind(key)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(())
    }

    async fn list_tokens(&self, profile: &str) -> CowenResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT key FROM cowen_token WHERE profile = ?")
            .bind(profile)
            .fetch_all(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn save_audit(&self, entry: &AuditEntry) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_audit (id, timestamp, profile, level, target, message, fields) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&entry.id).bind(entry.timestamp).bind(&entry.profile).bind(&entry.level).bind(&entry.target).bind(&entry.message).bind(&entry.fields)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(())
    }

    async fn list_audit(&self, profile: &str, limit: usize) -> CowenResult<Vec<AuditEntry>> {
        let rows: Vec<(String, DateTime<Utc>, String, String, String, String, serde_json::Value)> = sqlx::query_as(
            "SELECT id, timestamp, profile, level, target, message, fields FROM cowen_audit WHERE profile = ? ORDER BY timestamp DESC LIMIT ?"
        ).bind(profile).bind(limit as i64)
        .fetch_all(&self.pool).await
        .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        
        Ok(rows.into_iter().map(|r| AuditEntry {
            id: r.0, timestamp: r.1, profile: r.2, level: r.3, target: r.4, message: r.5, fields: r.6
        }).collect())
    }

    async fn push_dlq(&self, msg: &DlqMessage) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_dlq (profile, topic, payload, retry_count, error, created_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&msg.profile).bind(&msg.topic).bind(&msg.payload).bind(msg.retry_count).bind(&msg.error).bind(msg.created_at)
            .execute(&self.pool).await
            .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        Ok(())
    }

    async fn pop_dlq(&self, profile: &str, topic: &str) -> CowenResult<Option<DlqMessage>> {
        let row: Option<(i64, String, String, String, i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = ? AND topic = ? ORDER BY id ASC LIMIT 1"
        ).bind(profile).bind(topic)
        .fetch_optional(&self.pool).await
        .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;

        if let Some(r) = row {
            sqlx::query("DELETE FROM cowen_dlq WHERE id = ?").bind(r.0).execute(&self.pool).await?;
            Ok(Some(DlqMessage { id: Some(r.0), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6 }))
        } else {
            Ok(None)
        }
    }

    async fn list_dlq(&self, profile: &str, limit: usize) -> CowenResult<Vec<DlqMessage>> {
        let rows: Vec<(i64, String, String, String, i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = ? ORDER BY id DESC LIMIT ?"
        ).bind(profile).bind(limit as i64)
        .fetch_all(&self.pool).await
        .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        
        Ok(rows.into_iter().map(|r| DlqMessage {
            id: Some(r.0), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }).collect())
    }

    async fn list_all_dlq(&self, profile: &str) -> CowenResult<Vec<DlqMessage>> {
        let rows: Vec<(i64, String, String, String, i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = ? ORDER BY id DESC"
        ).bind(profile)
        .fetch_all(&self.pool).await
        .map_err(|e| anyhow::anyhow!("Sqlite error: {}", e))?;
        
        Ok(rows.into_iter().map(|r| DlqMessage {
            id: Some(r.0), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }).collect())
    }

    async fn clear_profile(&self, profile: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_config WHERE profile = ?").bind(profile).execute(&self.pool).await?;
        sqlx::query("DELETE FROM cowen_secret WHERE profile = ?").bind(profile).execute(&self.pool).await?;
        sqlx::query("DELETE FROM cowen_token WHERE profile = ?").bind(profile).execute(&self.pool).await?;
        sqlx::query("DELETE FROM cowen_audit WHERE profile = ?").bind(profile).execute(&self.pool).await?;
        sqlx::query("DELETE FROM cowen_dlq WHERE profile = ?").bind(profile).execute(&self.pool).await?;
        Ok(())
    }

    async fn rename_profile(&self, old_name: &str, new_name: &str) -> CowenResult<()> {
        sqlx::query("UPDATE cowen_config SET profile = ? WHERE profile = ?").bind(new_name).bind(old_name).execute(&self.pool).await?;
        sqlx::query("UPDATE cowen_secret SET profile = ? WHERE profile = ?").bind(new_name).bind(old_name).execute(&self.pool).await?;
        sqlx::query("UPDATE cowen_token SET profile = ? WHERE profile = ?").bind(new_name).bind(old_name).execute(&self.pool).await?;
        sqlx::query("UPDATE cowen_audit SET profile = ? WHERE profile = ?").bind(new_name).bind(old_name).execute(&self.pool).await?;
        sqlx::query("UPDATE cowen_dlq SET profile = ? WHERE profile = ?").bind(new_name).bind(old_name).execute(&self.pool).await?;
        Ok(())
    }

    async fn list_all_profiles(&self) -> CowenResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT profile FROM cowen_config").fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn raw_del(&self, key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_config WHERE key = ?").bind(key).execute(&self.pool).await?;
        Ok(())
    }
}

pub struct SqliteBuilder;

#[async_trait]
impl SqlBuilder for SqliteBuilder {
    fn scheme(&self) -> &str { "sqlite" }
    async fn build(&self, url: &str) -> CowenResult<Arc<dyn SqlDriver>> {
        let pool = Pool::connect(url).await.map_err(|e| anyhow::anyhow!("Failed to connect to sqlite: {}", e))?;
        Ok(Arc::new(SqliteDriver::new(pool)))
    }
}

inventory::submit! { SqlBuilderRegistration { builder: &SqliteBuilder } }
