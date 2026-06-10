/* jscpd:ignore-start */
#![cfg(feature = "postgres")]
use cowen_common::{CowenResult, CowenError};
use async_trait::async_trait;

use crate::sql::{SqlBuilder, SqlDriver, SqlBuilderRegistration};
use sqlx::{Postgres, Pool};
use std::sync::Arc;
use cowen_common::models::{Token, Ticket, Item, AuditEntry, DlqMessage};
use chrono::{DateTime, Utc};

pub struct PostgresDriver {
    pool: Pool<Postgres>,
}

impl PostgresDriver {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SqlDriver for PostgresDriver {
    async fn shutdown(&self) -> CowenResult<()> {
        self.pool.close().await;
        Ok(())
    }

    async fn get_config(&self, profile: &str, key: &str) -> CowenResult<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_config WHERE profile = $1 AND item_key = $2")
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
        let row: (i64, DateTime<Utc>) = sqlx::query_as("SELECT version, updated_at FROM cowen_config WHERE profile = $1 AND item_key = $2")
            .bind(profile)
            .bind(key)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("Key '{}' not found in profile '{}'", key, profile)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok((row.0 as u64, row.1.timestamp()))
    }

    async fn get_config_full(&self, profile: &str, key: &str) -> CowenResult<Item> {
        let row: (String, String, String, i64, DateTime<Utc>) = sqlx::query_as("SELECT profile, item_key, item_value, version, updated_at FROM cowen_config WHERE profile = $1 AND item_key = $2")
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
        sqlx::query("INSERT INTO cowen_config (profile, item_key, item_value, version) VALUES ($1, $2, $3, 1) 
                     ON CONFLICT(profile, item_key) DO UPDATE SET item_value=EXCLUDED.item_value, version=cowen_config.version+1")
            .bind(profile).bind(key).bind(value)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> CowenResult<()> {
        let res = sqlx::query("UPDATE cowen_config SET item_value = $1, version = version + 1 WHERE profile = $2 AND item_key = $3 AND version = $4")
            .bind(value).bind(profile).bind(key).bind(expected_version as i64)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        
        if res.rows_affected() == 0 {
            return Err(CowenError::Store("CAS failed: version mismatch or record not found".to_string()));
        }
        Ok(())
    }

    async fn list_configs(&self, profile: &str) -> CowenResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT item_key FROM cowen_config WHERE profile = $1")
            .bind(profile)
            .fetch_all(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn delete_config(&self, profile: &str, key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_config WHERE profile = $1 AND item_key = $2")
            .bind(profile).bind(key)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_secret(&self, profile: &str, key: &str) -> CowenResult<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_secret WHERE profile = $1 AND item_key = $2")
            .bind(profile).bind(key)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("Key '{}' not found in profile '{}'", key, profile)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(row.0)
    }

    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_secret (profile, item_key, item_value) VALUES ($1, $2, $3) 
                     ON CONFLICT(profile, item_key) DO UPDATE SET item_value=EXCLUDED.item_value")
            .bind(profile).bind(key).bind(value)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_secret(&self, profile: &str, key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_secret WHERE profile = $1 AND item_key = $2")
            .bind(profile).bind(key)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn list_secrets(&self, profile: &str) -> CowenResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT item_key FROM cowen_secret WHERE profile = $1")
            .bind(profile)
            .fetch_all(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn get_access_token(&self, profile: &str) -> CowenResult<Token> {
        let row: (String, DateTime<Utc>, DateTime<Utc>) = sqlx::query_as("SELECT token_value, expires_at, created_at FROM cowen_tenant_token WHERE profile = $1 AND token_type = 'access_token'")
            .bind(profile)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("AccessToken not found for profile '{}'", profile)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(Token { value: row.0, expires_at: row.1, created_at: row.2 })
    }

    async fn save_access_token(&self, profile: &str, token: Token) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_tenant_token (profile, token_type, token_value, expires_at, created_at) VALUES ($1, 'access_token', $2, $3, $4) 
                     ON CONFLICT(profile, token_type) DO UPDATE SET token_value=EXCLUDED.token_value, expires_at=EXCLUDED.expires_at, created_at=EXCLUDED.created_at")
            .bind(profile).bind(token.value).bind(token.expires_at).bind(token.created_at)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_access_token(&self, profile: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_tenant_token WHERE profile = $1 AND token_type = 'access_token'")
            .bind(profile)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_refresh_token(&self, profile: &str) -> CowenResult<Token> {
        let row: (String, DateTime<Utc>, DateTime<Utc>) = sqlx::query_as("SELECT token_value, expires_at, created_at FROM cowen_tenant_token WHERE profile = $1 AND token_type = 'refresh_token'")
            .bind(profile)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("RefreshToken not found for profile '{}'", profile)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(Token { value: row.0, expires_at: row.1, created_at: row.2 })
    }

    async fn save_refresh_token(&self, profile: &str, token: Token) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_tenant_token (profile, token_type, token_value, expires_at, created_at) VALUES ($1, 'refresh_token', $2, $3, $4) 
                     ON CONFLICT(profile, token_type) DO UPDATE SET token_value=EXCLUDED.token_value, expires_at=EXCLUDED.expires_at, created_at=EXCLUDED.created_at")
            .bind(profile).bind(token.value).bind(token.expires_at).bind(token.created_at)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_refresh_token(&self, profile: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_tenant_token WHERE profile = $1 AND token_type = 'refresh_token'")
            .bind(profile)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_app_access_token(&self, app_key: &str) -> CowenResult<Token> {
        let row: (String, DateTime<Utc>, DateTime<Utc>) = sqlx::query_as("SELECT token_value, expires_at, created_at FROM cowen_app_token WHERE app_key = $1")
            .bind(app_key)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("AppToken not found for key '{}'", app_key)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(Token { value: row.0, expires_at: row.1, created_at: row.2 })
    }

    async fn save_app_access_token(&self, app_key: &str, token: Token) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_app_token (app_key, token_value, expires_at, created_at) VALUES ($1, $2, $3, $4) 
                     ON CONFLICT(app_key) DO UPDATE SET token_value=EXCLUDED.token_value, expires_at=EXCLUDED.expires_at, created_at=EXCLUDED.created_at")
            .bind(app_key).bind(token.value).bind(token.expires_at).bind(token.created_at)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_app_access_token(&self, app_key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_app_token WHERE app_key = $1")
            .bind(app_key)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<Ticket> {
        let row: (String, DateTime<Utc>) = sqlx::query_as("SELECT ticket_value, created_at FROM cowen_ticket WHERE app_key = $1")
            .bind(app_key)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("AppTicket not found for key '{}'", app_key)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(Ticket { value: row.0, created_at: row.1 })
    }

    async fn save_app_ticket(&self, app_key: &str, ticket: Ticket) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_ticket (app_key, ticket_value, created_at) VALUES ($1, $2, $3) 
                     ON CONFLICT(app_key) DO UPDATE SET ticket_value=EXCLUDED.ticket_value, created_at=EXCLUDED.created_at")
            .bind(app_key).bind(ticket.value).bind(ticket.created_at)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_app_ticket(&self, app_key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_ticket WHERE app_key = $1")
            .bind(app_key)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> CowenResult<String> {
        let row: (String,) = sqlx::query_as("SELECT code_value FROM cowen_permanent_code WHERE app_key = $1 AND org_id = $2 AND code_type = 'org_permanent'")
            .bind(app_key).bind(org_id)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("OrgPermanentCode not found for app '{}' and org '{}'", app_key, org_id)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(row.0)
    }

    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_permanent_code (app_key, org_id, code_type, code_value) VALUES ($1, $2, 'org_permanent', $3) 
                     ON CONFLICT(app_key, org_id, user_id, code_type) DO UPDATE SET code_value=EXCLUDED.code_value")
            .bind(app_key).bind(org_id).bind(code)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> CowenResult<String> {
        let row: (String,) = sqlx::query_as("SELECT code_value FROM cowen_permanent_code WHERE app_key = $1 AND org_id = $2 AND user_id = $3 AND code_type = 'user_permanent'")
            .bind(app_key).bind(org_id).bind(user_id)
            .fetch_one(&self.pool).await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => CowenError::NotFound(format!("UserPermanentCode not found for app '{}', org '{}' and user '{}'", app_key, org_id, user_id)),
                _ => CowenError::Store(e.to_string()),
            })?;
        Ok(row.0)
    }

    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_permanent_code (app_key, org_id, user_id, code_type, code_value) VALUES ($1, $2, $3, 'user_permanent', $4) 
                     ON CONFLICT(app_key, org_id, user_id, code_type) DO UPDATE SET code_value=EXCLUDED.code_value")
            .bind(app_key).bind(org_id).bind(user_id).bind(code)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_token(&self, profile: &str, key: &str) -> CowenResult<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_token WHERE profile = $1 AND item_key = $2")
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
        sqlx::query("INSERT INTO cowen_token (profile, item_key, item_value, expires_at) VALUES ($1, $2, $3, $4) 
                     ON CONFLICT(profile, item_key) DO UPDATE SET item_value=EXCLUDED.item_value, expires_at=EXCLUDED.expires_at")
            .bind(profile).bind(key).bind(value).bind(exp)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_token(&self, profile: &str, key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_token WHERE profile = $1 AND item_key = $2")
            .bind(profile).bind(key)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn list_tokens(&self, profile: &str) -> CowenResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT item_key FROM cowen_token WHERE profile = $1")
            .bind(profile)
            .fetch_all(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn save_audit(&self, entry: &AuditEntry) -> CowenResult<()> {
        let fields_json = serde_json::to_string(&entry.fields).unwrap_or_default();
        sqlx::query("INSERT INTO cowen_audit (id, profile, timestamp, level, target, message, fields) VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .bind(&entry.id).bind(&entry.profile).bind(entry.timestamp).bind(&entry.level).bind(&entry.target).bind(&entry.message).bind(fields_json)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn list_audit(&self, profile: &str, limit: usize) -> CowenResult<Vec<AuditEntry>> {
        let rows: Vec<(String, String, DateTime<Utc>, String, String, String, String)> = sqlx::query_as(
            "SELECT id, profile, timestamp, level, target, message, fields FROM cowen_audit WHERE profile = $1 ORDER BY timestamp DESC LIMIT $2"
        ).bind(profile).bind(limit as i64)
        .fetch_all(&self.pool).await
        .map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(rows.into_iter().map(|r| AuditEntry {
            id: r.0, profile: r.1, timestamp: r.2, level: r.3, target: r.4, message: r.5, 
            fields: serde_json::from_str(&r.6).unwrap_or_default(),
        }).collect())
    }

    async fn push_dlq(&self, msg: &DlqMessage) -> CowenResult<()> {
        sqlx::query("INSERT INTO cowen_dlq (profile, topic, payload, retry_count, error, created_at) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(&msg.profile).bind(&msg.topic).bind(&msg.payload).bind(msg.retry_count).bind(&msg.error).bind(msg.created_at)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn pop_dlq(&self, profile: &str, topic: &str) -> CowenResult<Option<DlqMessage>> {
        let row: Option<(i32, String, String, String, i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = $1 AND topic = $2 LIMIT 1"
        ).bind(profile).bind(topic)
        .fetch_optional(&self.pool).await
        .map_err(|e| CowenError::Store(e.to_string()))?;

        if let Some(r) = row {
            sqlx::query("DELETE FROM cowen_dlq WHERE id = $1")
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
        let rows: Vec<(i32, String, String, String, i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = $1 LIMIT $2"
        ).bind(profile).bind(limit as i64)
        .fetch_all(&self.pool).await
        .map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(rows.into_iter().map(|r| DlqMessage {
            id: Some(r.0 as i64), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }).collect())
    }

    async fn list_all_dlq(&self, profile: &str) -> CowenResult<Vec<DlqMessage>> {
        let rows: Vec<(i32, String, String, String, i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = $1"
        ).bind(profile)
        .fetch_all(&self.pool).await
        .map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(rows.into_iter().map(|r| DlqMessage {
            id: Some(r.0 as i64), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }).collect())
    }

    async fn get_dlq_by_id(&self, id: i64) -> CowenResult<Option<DlqMessage>> {
        let row: Option<(i32, String, String, String, i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE id = $1"
        ).bind(id as i32)
        .fetch_optional(&self.pool).await
        .map_err(|e| CowenError::Store(e.to_string()))?;

        Ok(row.map(|r| DlqMessage {
            id: Some(r.0 as i64), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }))
    }

    async fn list_dlq_paged(&self, profile: &str, offset: usize, limit: usize) -> CowenResult<Vec<DlqMessage>> {
        let rows: Vec<(i32, String, String, String, i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = $1 LIMIT $2 OFFSET $3"
        ).bind(profile).bind(limit as i64).bind(offset as i64)
        .fetch_all(&self.pool).await
        .map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(rows.into_iter().map(|r| DlqMessage {
            id: Some(r.0 as i64), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }).collect())
    }

    async fn delete_dlq_by_id(&self, id: i64) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_dlq WHERE id = $1")
            .bind(id as i32)
            .execute(&self.pool).await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn migrate(&self) -> CowenResult<()> {
        use crate::sql::migration_trait::SchemaMigration;
        self.run_migration().await
    }

    async fn clear_profile(&self, profile: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_config WHERE profile = $1").bind(profile).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("DELETE FROM cowen_secret WHERE profile = $1").bind(profile).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("DELETE FROM cowen_token WHERE profile = $1").bind(profile).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("DELETE FROM cowen_audit WHERE profile = $1").bind(profile).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("DELETE FROM cowen_dlq WHERE profile = $1").bind(profile).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn rename_profile(&self, old_name: &str, new_name: &str) -> CowenResult<()> {
        sqlx::query("UPDATE cowen_config SET profile = $1 WHERE profile = $2").bind(new_name).bind(old_name).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("UPDATE cowen_secret SET profile = $1 WHERE profile = $2").bind(new_name).bind(old_name).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("UPDATE cowen_token SET profile = $1 WHERE profile = $2").bind(new_name).bind(old_name).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("UPDATE cowen_tenant_token SET profile = $1 WHERE profile = $2").bind(new_name).bind(old_name).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("UPDATE cowen_audit SET profile = $1 WHERE profile = $2").bind(new_name).bind(old_name).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        sqlx::query("UPDATE cowen_dlq SET profile = $1 WHERE profile = $2").bind(new_name).bind(old_name).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn list_all_profiles(&self) -> CowenResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT profile FROM cowen_config").fetch_all(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn raw_del(&self, key: &str) -> CowenResult<()> {
        sqlx::query("DELETE FROM cowen_config WHERE item_key = $1").bind(key).execute(&self.pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }
}

pub struct PostgresBuilder;

#[async_trait]
impl SqlBuilder for PostgresBuilder {
    fn scheme(&self) -> &str { "postgres" }
    async fn build(&self, url: &str) -> CowenResult<Arc<dyn SqlDriver>> {
        let pool = sqlx::PgPool::connect(url).await.map_err(|e| CowenError::Store(e.to_string()))?;
        
        let ddl = [
            "CREATE TABLE IF NOT EXISTS cowen_config (profile TEXT NOT NULL, item_key TEXT NOT NULL, item_value TEXT NOT NULL, version BIGINT DEFAULT 0, updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (profile, item_key))",
            "CREATE TABLE IF NOT EXISTS cowen_secret (profile TEXT NOT NULL, item_key TEXT NOT NULL, item_value TEXT NOT NULL, updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (profile, item_key))",
            "CREATE TABLE IF NOT EXISTS cowen_token (profile TEXT NOT NULL, item_key TEXT NOT NULL, item_value TEXT NOT NULL, expires_at TIMESTAMP WITH TIME ZONE NULL, PRIMARY KEY (profile, item_key))",
            "CREATE TABLE IF NOT EXISTS cowen_ticket (app_key TEXT PRIMARY KEY, ticket_value TEXT NOT NULL, created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE IF NOT EXISTS cowen_app_token (app_key TEXT PRIMARY KEY, token_value TEXT NOT NULL, expires_at TIMESTAMP WITH TIME ZONE NOT NULL, created_at TIMESTAMP WITH TIME ZONE NOT NULL)",
            "CREATE TABLE IF NOT EXISTS cowen_tenant_token (profile TEXT NOT NULL, token_type TEXT NOT NULL, token_value TEXT NOT NULL, expires_at TIMESTAMP WITH TIME ZONE NOT NULL, created_at TIMESTAMP WITH TIME ZONE NOT NULL, PRIMARY KEY (profile, token_type))",
            "CREATE TABLE IF NOT EXISTS cowen_permanent_code (app_key TEXT NOT NULL, org_id TEXT NOT NULL, user_id TEXT DEFAULT '', code_type TEXT NOT NULL, code_value TEXT NOT NULL, created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (app_key, org_id, user_id, code_type))",
            "CREATE TABLE IF NOT EXISTS cowen_audit (id TEXT PRIMARY KEY, profile TEXT NOT NULL, timestamp TIMESTAMP WITH TIME ZONE NOT NULL, level TEXT NOT NULL, target TEXT NOT NULL, message TEXT NOT NULL, fields TEXT)",
            "CREATE TABLE IF NOT EXISTS cowen_dlq (id SERIAL PRIMARY KEY, profile TEXT NOT NULL, topic TEXT NOT NULL, payload TEXT NOT NULL, retry_count INT DEFAULT 0, error TEXT, created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP)",
        ];

        for sql in ddl {
            sqlx::query(sql).execute(&pool).await.map_err(|e| CowenError::Store(e.to_string()))?;
        }

        // Indices
        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_profile_ts ON cowen_audit (profile, timestamp)").execute(&pool).await;
        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_dlq_profile_topic ON cowen_dlq (profile, topic)").execute(&pool).await;

        Ok(Arc::new(PostgresDriver::new(pool)))
    }
}

inventory::submit! { SqlBuilderRegistration { builder: &PostgresBuilder } }

crate::implement_schema_migration!{PostgresDriver, true}


/* jscpd:ignore-end */
