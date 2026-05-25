#![cfg(feature = "mysql")]
use cowen_common::{CowenResult, CowenError};
use async_trait::async_trait;

use crate::sql::{SqlBuilder, SqlDriver, SqlBuilderRegistration};
use sqlx::{MySql, Pool};
use std::sync::Arc;
use cowen_common::models::{Token, Ticket, Item, AuditEntry, DlqMessage};
use chrono::{DateTime, Utc};

pub struct MySqlDriver {
    pool: Pool<MySql>,
}

impl MySqlDriver {
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SqlDriver for MySqlDriver {
    async fn shutdown(&self) -> CowenResult<()> {
        self.pool.close().await;
        Ok(())
    }

    async fn get_config(&self, profile: &str, key: &str) -> CowenResult<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_config WHERE profile = ? AND item_key = ?")
            .bind(profile)
            .bind(key)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("Key '{}' not found in profile '{}'", key, profile)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(row.0)
    }

    async fn get_config_metadata(&self, profile: &str, key: &str) -> CowenResult<(u64, i64)> {
        let row: (u64, DateTime<Utc>) = sqlx::query_as("SELECT version, updated_at FROM cowen_config WHERE profile = ? AND item_key = ?")
            .bind(profile)
            .bind(key)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("Key '{}' not found in profile '{}'", key, profile)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok((row.0, row.1.timestamp()))
    }

    async fn get_config_full(&self, profile: &str, key: &str) -> CowenResult<Item> {
        let row: (String, String, String, i64, DateTime<Utc>) = sqlx::query_as("SELECT profile, item_key, item_value, version, updated_at FROM cowen_config WHERE profile = ? AND item_key = ?")
            .bind(profile)
            .bind(key)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("Key '{}' not found in profile '{}'", key, profile)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(Item {
            profile: row.0,
            key: row.1,
            value: row.2,
            version: row.3 as u64,
            updated_at: row.4.timestamp(),
        })
    }

    async fn set_config(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_config (profile, item_key, item_value, version) VALUES (?, ?, ?, 1) 
                     ON DUPLICATE KEY UPDATE item_value=VALUES(item_value), version=version+1")
            .bind(profile).bind(key).bind(value)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> CowenResult<()> {
        let res = sqlx::query("UPDATE cowen_config SET item_value = ?, version = version + 1 WHERE profile = ? AND item_key = ? AND version = ?")
            .bind(value).bind(profile).bind(key).bind(expected_version as i64)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        
        if res.rows_affected() == 0 {
            return Err(CowenError::Store("CAS failed: version mismatch or record not found".to_string()));
        }
        Ok(())
    }

    async fn list_configs(&self, profile: &str) -> CowenResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT item_key FROM cowen_config WHERE profile = ?")
            .bind(profile)
            .fetch_all(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn delete_config(&self, profile: &str, key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_config WHERE profile = ? AND item_key = ?")
            .bind(profile).bind(key)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_secret(&self, profile: &str, key: &str) -> CowenResult<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_secret WHERE profile = ? AND item_key = ?")
            .bind(profile).bind(key)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("Key '{}' not found in profile '{}'", key, profile)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(row.0)
    }

    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_secret (profile, item_key, item_value) VALUES (?, ?, ?) 
                     ON DUPLICATE KEY UPDATE item_value=VALUES(item_value)")
            .bind(profile).bind(key).bind(value)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_secret(&self, profile: &str, key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_secret WHERE profile = ? AND item_key = ?")
            .bind(profile).bind(key)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn list_secrets(&self, profile: &str) -> CowenResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT item_key FROM cowen_secret WHERE profile = ?")
            .bind(profile)
            .fetch_all(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn get_access_token(&self, profile: &str) -> CowenResult<Token> {
        let row: (String, DateTime<Utc>, DateTime<Utc>) = sqlx::query_as("SELECT token_value, expires_at, created_at FROM cowen_tenant_token WHERE profile = ? AND token_type = 'access_token'")
            .bind(profile)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("AccessToken not found for profile '{}'", profile)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(Token { value: row.0, expires_at: row.1, created_at: row.2 })
    }

    async fn save_access_token(&self, profile: &str, token: Token) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_tenant_token (profile, token_type, token_value, expires_at, created_at) VALUES (?, 'access_token', ?, ?, ?) 
                     ON DUPLICATE KEY UPDATE token_value=VALUES(token_value), expires_at=VALUES(expires_at), created_at=VALUES(created_at)")
            .bind(profile).bind(token.value).bind(token.expires_at).bind(token.created_at)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_access_token(&self, profile: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_tenant_token WHERE profile = ? AND token_type = 'access_token'")
            .bind(profile)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_refresh_token(&self, profile: &str) -> CowenResult<Token> {
        let row: (String, DateTime<Utc>, DateTime<Utc>) = sqlx::query_as("SELECT token_value, expires_at, created_at FROM cowen_tenant_token WHERE profile = ? AND token_type = 'refresh_token'")
            .bind(profile)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("RefreshToken not found for profile '{}'", profile)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(Token { value: row.0, expires_at: row.1, created_at: row.2 })
    }

    async fn save_refresh_token(&self, profile: &str, token: Token) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_tenant_token (profile, token_type, token_value, expires_at, created_at) VALUES (?, 'refresh_token', ?, ?, ?) 
                     ON DUPLICATE KEY UPDATE token_value=VALUES(token_value), expires_at=VALUES(expires_at), created_at=VALUES(created_at)")
            .bind(profile).bind(token.value).bind(token.expires_at).bind(token.created_at)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_refresh_token(&self, profile: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_tenant_token WHERE profile = ? AND token_type = 'refresh_token'")
            .bind(profile)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_app_access_token(&self, app_key: &str) -> CowenResult<Token> {
        let row: (String, DateTime<Utc>, DateTime<Utc>) = sqlx::query_as("SELECT token_value, expires_at, created_at FROM cowen_app_token WHERE app_key = ?")
            .bind(app_key)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("AppToken not found for key '{}'", app_key)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(Token { value: row.0, expires_at: row.1, created_at: row.2 })
    }

    async fn save_app_access_token(&self, app_key: &str, token: Token) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_app_token (app_key, token_value, expires_at, created_at) VALUES (?, ?, ?, ?) 
                     ON DUPLICATE KEY UPDATE token_value=VALUES(token_value), expires_at=VALUES(expires_at), created_at=VALUES(created_at)")
            .bind(app_key).bind(token.value).bind(token.expires_at).bind(token.created_at)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_app_access_token(&self, app_key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_app_token WHERE app_key = ?")
            .bind(app_key)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<Ticket> {
        let row: (String, DateTime<Utc>) = sqlx::query_as("SELECT ticket_value, created_at FROM cowen_ticket WHERE app_key = ?")
            .bind(app_key)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("AppTicket not found for key '{}'", app_key)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(Ticket { value: row.0, created_at: row.1 })
    }

    async fn save_app_ticket(&self, app_key: &str, ticket: Ticket) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_ticket (app_key, ticket_value, created_at) VALUES (?, ?, ?) 
                     ON DUPLICATE KEY UPDATE ticket_value=VALUES(ticket_value), created_at=VALUES(created_at)")
            .bind(app_key).bind(ticket.value).bind(ticket.created_at)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_app_ticket(&self, app_key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_ticket WHERE app_key = ?")
            .bind(app_key)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> CowenResult<String> {
        let row: (String,) = sqlx::query_as("SELECT code_value FROM cowen_permanent_code WHERE app_key = ? AND org_id = ? AND code_type = 'org_permanent'")
            .bind(app_key).bind(org_id)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("OrgPermanentCode not found for app '{}' and org '{}'", app_key, org_id)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(row.0)
    }

    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_permanent_code (app_key, org_id, code_type, code_value) VALUES (?, ?, 'org_permanent', ?) 
                     ON DUPLICATE KEY UPDATE code_value=VALUES(code_value)")
            .bind(app_key).bind(org_id).bind(code)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> CowenResult<String> {
        let row: (String,) = sqlx::query_as("SELECT code_value FROM cowen_permanent_code WHERE app_key = ? AND org_id = ? AND user_id = ? AND code_type = 'user_permanent'")
            .bind(app_key).bind(org_id).bind(user_id)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("UserPermanentCode not found for app '{}', org '{}' and user '{}'", app_key, org_id, user_id)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(row.0)
    }

    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_permanent_code (app_key, org_id, user_id, code_type, code_value) VALUES (?, ?, ?, 'user_permanent', ?) 
                     ON DUPLICATE KEY UPDATE code_value=VALUES(code_value)")
            .bind(app_key).bind(org_id).bind(user_id).bind(code)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_token(&self, profile: &str, key: &str) -> CowenResult<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_token WHERE profile = ? AND item_key = ?")
            .bind(profile).bind(key)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("Key '{}' not found in profile '{}'", key, profile)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(row.0)
    }

    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> CowenResult<()> {
        let exp = Utc::now() + chrono::Duration::seconds(expires_in_secs as i64);
        sqlx::query("INSERT INTO cowen_token (profile, item_key, item_value, expires_at) VALUES (?, ?, ?, ?) 
                     ON DUPLICATE KEY UPDATE item_value=VALUES(item_value), expires_at=VALUES(expires_at)")
            .bind(profile).bind(key).bind(value).bind(exp)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_token(&self, profile: &str, key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_token WHERE profile = ? AND item_key = ?")
            .bind(profile).bind(key)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn list_tokens(&self, profile: &str) -> CowenResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT item_key FROM cowen_token WHERE profile = ?")
            .bind(profile)
            .fetch_all(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn save_audit(&self, entry: &AuditEntry) -> CowenResult<()> {
        let fields_json = serde_json::to_string(&entry.fields).unwrap_or_default();
        sqlx::query("INSERT INTO cowen_audit (id, profile, timestamp, level, target, message, fields) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&entry.id).bind(&entry.profile).bind(entry.timestamp).bind(&entry.level).bind(&entry.target).bind(&entry.message).bind(fields_json)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn list_audit(&self, profile: &str, limit: usize) -> CowenResult<Vec<AuditEntry>> {
        let rows: Vec<(String, String, DateTime<Utc>, String, String, String, String)> = sqlx::query_as(
            "SELECT id, profile, timestamp, level, target, message, fields FROM cowen_audit WHERE profile = ? ORDER BY timestamp DESC LIMIT ?"
        ).bind(profile).bind(limit as i64)
        .fetch_all(&self.pool).await
        .map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(rows.into_iter().map(|r| AuditEntry {
            id: r.0, profile: r.1, timestamp: r.2, level: r.3, target: r.4, message: r.5, 
            fields: serde_json::from_str(&r.6).unwrap_or_default(),
        }).collect())
    }

    async fn push_dlq(&self, msg: &DlqMessage) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_dlq (profile, topic, payload, retry_count, error, created_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&msg.profile).bind(&msg.topic).bind(&msg.payload).bind(msg.retry_count).bind(&msg.error).bind(msg.created_at)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn pop_dlq(&self, profile: &str, topic: &str) -> CowenResult<Option<DlqMessage>> {
        let row: Option<(u64, String, String, String, i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = ? AND topic = ? LIMIT 1"
        ).bind(profile).bind(topic)
        .fetch_optional(&self.pool).await
        .map_err(|e| CowenError::Store(e.to_string()))?;

        if let Some(r) = row {
            sqlx::query("DELETE FROM cowen_dlq WHERE id = ?")
                .bind(r.0)
                .execute(&self.pool).await
                .map_err(|e| CowenError::Store(e.to_string()))?;

            Ok(Some(DlqMessage {
                id: Some(r.0 as i64),
                profile: r.1,
                topic: r.2,
                payload: r.3,
                retry_count: r.4,
                error: r.5,
                created_at: r.6,
            }))
        } else {
            Ok(None)
        }
    }

    async fn list_dlq(&self, profile: &str, limit: usize) -> CowenResult<Vec<DlqMessage>> {
        let rows: Vec<(u64, String, String, String, i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = ? LIMIT ?"
        ).bind(profile).bind(limit as i64)
        .fetch_all(&self.pool).await
        .map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(rows.into_iter().map(|r| DlqMessage {
            id: Some(r.0 as i64), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }).collect())
    }

    async fn list_all_dlq(&self, profile: &str) -> CowenResult<Vec<DlqMessage>> {
        let rows: Vec<(u64, String, String, String, i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = ?"
        ).bind(profile)
        .fetch_all(&self.pool).await
        .map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(rows.into_iter().map(|r| DlqMessage {
            id: Some(r.0 as i64), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }).collect())
    }

    async fn get_dlq_by_id(&self, id: i64) -> CowenResult<Option<DlqMessage>> {
        let row: Option<(u64, String, String, String, i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE id = ?"
        ).bind(id as u64)
        .fetch_optional(&self.pool).await
        .map_err(|e| CowenError::Store(e.to_string()))?;

        Ok(row.map(|r| DlqMessage {
            id: Some(r.0 as i64), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }))
    }

    async fn list_dlq_paged(&self, profile: &str, offset: usize, limit: usize) -> CowenResult<Vec<DlqMessage>> {
        let rows: Vec<(u64, String, String, String, i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = ? LIMIT ? OFFSET ?"
        ).bind(profile).bind(limit as u64).bind(offset as u64)
        .fetch_all(&self.pool).await
        .map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(rows.into_iter().map(|r| DlqMessage {
            id: Some(r.0 as i64), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }).collect())
    }

    async fn delete_dlq_by_id(&self, id: i64) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_dlq WHERE id = ?")
            .bind(id as u64)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn migrate(&self) -> CowenResult<()> {
        use crate::sql::migration_trait::SchemaMigration;
        self.run_migration().await
    }

    async fn clear_profile(&self, profile: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_config WHERE profile = ?").bind(profile).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("DELETE FROM cowen_secret WHERE profile = ?").bind(profile).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("DELETE FROM cowen_token WHERE profile = ?").bind(profile).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("DELETE FROM cowen_audit WHERE profile = ?").bind(profile).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("DELETE FROM cowen_dlq WHERE profile = ?").bind(profile).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn rename_profile(&self, old_name: &str, new_name: &str) -> CowenResult<()> {
        sqlx::query("UPDATE cowen_config SET profile = ? WHERE profile = ?").bind(new_name).bind(old_name).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("UPDATE cowen_secret SET profile = ? WHERE profile = ?").bind(new_name).bind(old_name).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("UPDATE cowen_token SET profile = ? WHERE profile = ?").bind(new_name).bind(old_name).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("UPDATE cowen_tenant_token SET profile = ? WHERE profile = ?").bind(new_name).bind(old_name).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("UPDATE cowen_audit SET profile = ? WHERE profile = ?").bind(new_name).bind(old_name).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("UPDATE cowen_dlq SET profile = ? WHERE profile = ?").bind(new_name).bind(old_name).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn list_all_profiles(&self) -> CowenResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT profile FROM cowen_config").fetch_all(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn raw_del(&self, key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_config WHERE item_key = ?").bind(key).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }
}

pub struct MySqlBuilder;

#[async_trait]
impl SqlBuilder for MySqlBuilder {
    fn scheme(&self) -> &str { "mysql" }
    async fn build(&self, url: &str) -> CowenResult<Arc<dyn SqlDriver>> {
        let pool = sqlx::MySqlPool::connect(url).await.map_err(|e| CowenError::Store(e.to_string()))?;
        
        let ddl = [
            "CREATE TABLE IF NOT EXISTS cowen_config (profile VARCHAR(64) NOT NULL, item_key VARCHAR(128) NOT NULL, item_value MEDIUMTEXT NOT NULL, version BIGINT DEFAULT 0, updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, PRIMARY KEY (profile, item_key))",
            "CREATE TABLE IF NOT EXISTS cowen_secret (profile VARCHAR(64) NOT NULL, item_key VARCHAR(128) NOT NULL, item_value MEDIUMTEXT NOT NULL, updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, PRIMARY KEY (profile, item_key))",
            "CREATE TABLE IF NOT EXISTS cowen_token (profile VARCHAR(64) NOT NULL, item_key VARCHAR(128) NOT NULL, item_value MEDIUMTEXT NOT NULL, expires_at TIMESTAMP NULL, PRIMARY KEY (profile, item_key))",
            "CREATE TABLE IF NOT EXISTS cowen_ticket (app_key VARCHAR(64) PRIMARY KEY, ticket_value MEDIUMTEXT NOT NULL, created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE IF NOT EXISTS cowen_app_token (app_key VARCHAR(64) PRIMARY KEY, token_value MEDIUMTEXT NOT NULL, expires_at TIMESTAMP NOT NULL, created_at TIMESTAMP NOT NULL)",
            "CREATE TABLE IF NOT EXISTS cowen_tenant_token (profile VARCHAR(64) NOT NULL, token_type VARCHAR(32) NOT NULL, token_value MEDIUMTEXT NOT NULL, expires_at TIMESTAMP NOT NULL, created_at TIMESTAMP NOT NULL, PRIMARY KEY (profile, token_type))",
            "CREATE TABLE IF NOT EXISTS cowen_permanent_code (app_key VARCHAR(64) NOT NULL, org_id VARCHAR(64) NOT NULL, user_id VARCHAR(64) DEFAULT '', code_type VARCHAR(32) NOT NULL, code_value VARCHAR(255) NOT NULL, created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (app_key, org_id, user_id, code_type))",
            "CREATE TABLE IF NOT EXISTS cowen_audit (id VARCHAR(64) PRIMARY KEY, profile VARCHAR(64) NOT NULL, timestamp TIMESTAMP NOT NULL, level VARCHAR(16) NOT NULL, target VARCHAR(64) NOT NULL, message TEXT NOT NULL, fields MEDIUMTEXT, INDEX(profile, timestamp))",
            "CREATE TABLE IF NOT EXISTS cowen_dlq (id BIGINT PRIMARY KEY AUTO_INCREMENT, profile VARCHAR(64) NOT NULL, topic VARCHAR(128) NOT NULL, payload MEDIUMTEXT NOT NULL, retry_count INT DEFAULT 0, error TEXT, created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, INDEX(profile, topic))",
        ];

        for sql in ddl {
            sqlx::query(sql).execute(&pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        }

        Ok(Arc::new(MySqlDriver::new(pool)))
    }
}

inventory::submit! { SqlBuilderRegistration { builder: &MySqlBuilder } }

#[async_trait]
impl crate::sql::migration_trait::SchemaMigration for MySqlDriver {
    async fn get_current_version(&self) -> CowenResult<u32> {
        sqlx::query("CREATE TABLE IF NOT EXISTS schema_migrations (version INT PRIMARY KEY)")
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
            
        let row: Option<(i32,)> = sqlx::query_as("SELECT MAX(version) FROM schema_migrations")
            .fetch_optional(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
            
        Ok(row.map_or(0, |r| r.0 as u32))
    }
    
    async fn apply_sql(&self, sql: &str) -> CowenResult<()> {
        sqlx::query(sql).execute(&self.pool).await.map_err(|e| CowenError::Store(format!("SQL apply error: {} ({})", e, sql)))?;
        Ok(())
    }
    
    async fn set_version(&self, version: u32) -> CowenResult<()> {
        sqlx::query("INSERT INTO schema_migrations (version) VALUES (?)")
            .bind(version as i32)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }
    
    async fn run_migration(&self) -> CowenResult<()> {
        let row: Option<(String,)> = sqlx::query_as("SELECT column_name FROM information_schema.columns WHERE table_name = 'cowen_dlq' AND column_name = 'id' AND table_schema = DATABASE()")
            .fetch_optional(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;

        if row.is_none() {
            tracing::info!(target: "sys", "Migrating MySQL cowen_dlq schema (adding 'id' column)...");
            self.apply_sql("ALTER TABLE cowen_dlq ADD COLUMN id BIGINT PRIMARY KEY AUTO_INCREMENT FIRST").await?;
            tracing::info!(target: "sys", "MySQL DLQ migration completed.");
        }
        
        let current_version = self.get_current_version().await.unwrap_or(0);
        for (version, sql) in self.get_migrations() {
            if current_version < version {
                tracing::info!(target: "sys", "Applying schema migration version {}...", version);
                self.apply_sql(sql).await?;
                self.set_version(version).await?;
                tracing::info!(target: "sys", "Migration version {} applied successfully.", version);
            }
        }
        Ok(())
    }
    
    fn get_migrations(&self) -> Vec<(u32, &'static str)> {
        vec![]
    }
}

