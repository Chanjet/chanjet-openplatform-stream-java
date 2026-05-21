#![cfg(feature = "mssql")]
use cowen_common::{CowenResult, CowenError};
use async_trait::async_trait;

use crate::sql::{SqlBuilder, SqlDriver, SqlBuilderRegistration};
use std::sync::Arc;
use cowen_common::models::{Token, Ticket, Item, AuditEntry, DlqMessage};
use chrono::{DateTime, Utc};
use deadpool_tiberius::Pool;

#[allow(dead_code)]
pub struct MssqlDriver {
    pool: Pool,
}

impl MssqlDriver {
    #[allow(dead_code)]
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SqlDriver for MssqlDriver {
    async fn shutdown(&self) -> CowenResult<()> {
        // bb8 connection pool does not have an explicit close/shutdown method
        // that we can readily call to forcefully drop connections, it cleans up on drop.
        // We will just let it be dropped when the Store is dropped.
        Ok(())
    }

    async fn get_config(&self, profile: &str, key: &str) -> CowenResult<String> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let row = conn.query("SELECT item_value FROM cowen_config WHERE profile = @p1 AND item_key = @p2", &[&profile, &key])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_row().await.map_err(|e| CowenError::Store(e.to_string()))?
            .ok_or_else(|| CowenError::NotFound(format!("Key '{}' not found in profile '{}'", key, profile)))?;
        
        let val: &str = row.get(0).ok_or_else(|| CowenError::Store("Null value".to_string()))?;
        Ok(val.to_string())
    }

    async fn get_config_metadata(&self, profile: &str, key: &str) -> CowenResult<(u64, i64)> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let row = conn.query("SELECT version, updated_at FROM cowen_config WHERE profile = @p1 AND item_key = @p2", &[&profile, &key])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_row().await.map_err(|e| CowenError::Store(e.to_string()))?
            .ok_or_else(|| CowenError::NotFound(format!("Key '{}' not found in profile '{}'", key, profile)))?;
        
        let version: i64 = row.get(0).unwrap_or(0);
        let updated_at: DateTime<Utc> = row.get(1).unwrap_or_else(|| Utc::now());
        Ok((version as u64, updated_at.timestamp()))
    }

    async fn get_config_full(&self, profile: &str, key: &str) -> CowenResult<Item> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let row = conn.query("SELECT profile, item_key, item_value, version, updated_at FROM cowen_config WHERE profile = @p1 AND item_key = @p2", &[&profile, &key])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_row().await.map_err(|e| CowenError::Store(e.to_string()))?
            .ok_or_else(|| CowenError::NotFound(format!("Key '{}' not found in profile '{}'", key, profile)))?;
        
        Ok(Item {
            profile: row.get::<&str, _>(0).unwrap_or_default().to_string(),
            key: row.get::<&str, _>(1).unwrap_or_default().to_string(),
            value: row.get::<&str, _>(2).unwrap_or_default().to_string(),
            version: row.get::<i64, _>(3).unwrap_or(0) as u64,
            updated_at: row.get::<DateTime<Utc>, _>(4).unwrap_or_else(|| Utc::now()).timestamp(),
        })
    }

    async fn set_config(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("MERGE cowen_config AS target
                      USING (SELECT @p1, @p2, @p3) AS source (profile, item_key, item_value)
                      ON (target.profile = source.profile AND target.item_key = source.item_key)
                      WHEN MATCHED THEN
                          UPDATE SET item_value = source.item_value, version = version + 1, updated_at = GETUTCDATE()
                      WHEN NOT MATCHED THEN
                          INSERT (profile, item_key, item_value, version, updated_at) VALUES (source.profile, source.item_key, source.item_value, 1, GETUTCDATE());",
            &[&profile, &key, &value]
        ).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let res = conn.execute("UPDATE cowen_config SET item_value = @p1, version = version + 1, updated_at = GETUTCDATE() WHERE profile = @p2 AND item_key = @p3 AND version = @p4",
            &[&value, &profile, &key, &(expected_version as i64)]
        ).await.map_err(|e| CowenError::Store(e.to_string()))?;
        
        if res.total() == 0 {
            return Err(CowenError::Store("CAS failed: version mismatch or record not found".to_string()));
        }
        Ok(())
    }

    async fn list_configs(&self, profile: &str) -> CowenResult<Vec<String>> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let rows = conn.query("SELECT item_key FROM cowen_config WHERE profile = @p1", &[&profile])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_first_result().await.map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(rows.into_iter().map(|r| r.get::<&str, _>(0).unwrap_or_default().to_string()).collect())
    }

    async fn delete_config(&self, profile: &str, key: &str) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("DELETE FROM cowen_config WHERE profile = @p1 AND item_key = @p2", &[&profile, &key])
            .await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_secret(&self, profile: &str, key: &str) -> CowenResult<String> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let row = conn.query("SELECT item_value FROM cowen_secret WHERE profile = @p1 AND item_key = @p2", &[&profile, &key])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_row().await.map_err(|e| CowenError::Store(e.to_string()))?
            .ok_or_else(|| CowenError::NotFound(format!("Key '{}' not found in profile '{}'", key, profile)))?;
        
        let val: &str = row.get(0).ok_or_else(|| CowenError::Store("Null value".to_string()))?;
        Ok(val.to_string())
    }

    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("MERGE cowen_secret AS target
                      USING (SELECT @p1, @p2, @p3) AS source (profile, item_key, item_value)
                      ON (target.profile = source.profile AND target.item_key = source.item_key)
                      WHEN MATCHED THEN
                          UPDATE SET item_value = source.item_value, updated_at = GETUTCDATE()
                      WHEN NOT MATCHED THEN
                          INSERT (profile, item_key, item_value, updated_at) VALUES (source.profile, source.item_key, source.item_value, GETUTCDATE());",
            &[&profile, &key, &value]
        ).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_secret(&self, profile: &str, key: &str) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("DELETE FROM cowen_secret WHERE profile = @p1 AND item_key = @p2", &[&profile, &key])
            .await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn list_secrets(&self, profile: &str) -> CowenResult<Vec<String>> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let rows = conn.query("SELECT item_key FROM cowen_secret WHERE profile = @p1", &[&profile])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_first_result().await.map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(rows.into_iter().map(|r| r.get::<&str, _>(0).unwrap_or_default().to_string()).collect())
    }

    async fn get_access_token(&self, profile: &str) -> CowenResult<Token> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let row = conn.query("SELECT token_value, expires_at, created_at FROM cowen_tenant_token WHERE profile = @p1 AND token_type = 'access_token'", &[&profile])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_row().await.map_err(|e| CowenError::Store(e.to_string()))?
            .ok_or_else(|| CowenError::NotFound(format!("AccessToken not found for profile '{}'", profile)))?;
        
        Ok(Token { 
            value: row.get::<&str, _>(0).unwrap_or_default().to_string(), 
            expires_at: row.get::<DateTime<Utc>, _>(1).unwrap_or_else(|| Utc::now()), 
            created_at: row.get::<DateTime<Utc>, _>(2).unwrap_or_else(|| Utc::now()) 
        })
    }

    async fn save_access_token(&self, profile: &str, token: Token) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("MERGE cowen_tenant_token AS target
                      USING (SELECT @p1, 'access_token', @p2, @p3, @p4) AS source (profile, token_type, token_value, expires_at, created_at)
                      ON (target.profile = source.profile AND target.token_type = source.token_type)
                      WHEN MATCHED THEN
                          UPDATE SET token_value = source.token_value, expires_at = source.expires_at, created_at = source.created_at
                      WHEN NOT MATCHED THEN
                          INSERT (profile, token_type, token_value, expires_at, created_at) VALUES (source.profile, source.token_type, source.token_value, source.expires_at, source.created_at);",
            &[&profile, &token.value.as_str(), &token.expires_at, &token.created_at]
        ).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_access_token(&self, profile: &str) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("DELETE FROM cowen_tenant_token WHERE profile = @p1 AND token_type = 'access_token'", &[&profile])
            .await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_refresh_token(&self, profile: &str) -> CowenResult<Token> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let row = conn.query("SELECT token_value, expires_at, created_at FROM cowen_tenant_token WHERE profile = @p1 AND token_type = 'refresh_token'", &[&profile])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_row().await.map_err(|e| CowenError::Store(e.to_string()))?
            .ok_or_else(|| CowenError::NotFound(format!("RefreshToken not found for profile '{}'", profile)))?;
        
        Ok(Token { 
            value: row.get::<&str, _>(0).unwrap_or_default().to_string(), 
            expires_at: row.get::<DateTime<Utc>, _>(1).unwrap_or_else(|| Utc::now()), 
            created_at: row.get::<DateTime<Utc>, _>(2).unwrap_or_else(|| Utc::now()) 
        })
    }

    async fn save_refresh_token(&self, profile: &str, token: Token) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("MERGE cowen_tenant_token AS target
                      USING (SELECT @p1, 'refresh_token', @p2, @p3, @p4) AS source (profile, token_type, token_value, expires_at, created_at)
                      ON (target.profile = source.profile AND target.token_type = source.token_type)
                      WHEN MATCHED THEN
                          UPDATE SET token_value = source.token_value, expires_at = source.expires_at, created_at = source.created_at
                      WHEN NOT MATCHED THEN
                          INSERT (profile, token_type, token_value, expires_at, created_at) VALUES (source.profile, source.token_type, source.token_value, source.expires_at, source.created_at);",
            &[&profile, &token.value.as_str(), &token.expires_at, &token.created_at]
        ).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_refresh_token(&self, profile: &str) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("DELETE FROM cowen_tenant_token WHERE profile = @p1 AND token_type = 'refresh_token'", &[&profile])
            .await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_app_access_token(&self, app_key: &str) -> CowenResult<Token> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let row = conn.query("SELECT token_value, expires_at, created_at FROM cowen_app_token WHERE app_key = @p1", &[&app_key])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_row().await.map_err(|e| CowenError::Store(e.to_string()))?
            .ok_or_else(|| CowenError::NotFound(format!("AppToken not found for key '{}'", app_key)))?;
        
        Ok(Token { 
            value: row.get::<&str, _>(0).unwrap_or_default().to_string(), 
            expires_at: row.get::<DateTime<Utc>, _>(1).unwrap_or_else(|| Utc::now()), 
            created_at: row.get::<DateTime<Utc>, _>(2).unwrap_or_else(|| Utc::now()) 
        })
    }

    async fn save_app_access_token(&self, app_key: &str, token: Token) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("MERGE cowen_app_token AS target
                      USING (SELECT @p1, @p2, @p3, @p4) AS source (app_key, token_value, expires_at, created_at)
                      ON (target.app_key = source.app_key)
                      WHEN MATCHED THEN
                          UPDATE SET token_value = source.token_value, expires_at = source.expires_at, created_at = source.created_at
                      WHEN NOT MATCHED THEN
                          INSERT (app_key, token_value, expires_at, created_at) VALUES (source.app_key, source.token_value, source.expires_at, source.created_at);",
            &[&app_key, &token.value.as_str(), &token.expires_at, &token.created_at]
        ).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_app_access_token(&self, app_key: &str) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("DELETE FROM cowen_app_token WHERE app_key = @p1", &[&app_key])
            .await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<Ticket> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let row = conn.query("SELECT ticket_value, created_at FROM cowen_ticket WHERE app_key = @p1", &[&app_key])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_row().await.map_err(|e| CowenError::Store(e.to_string()))?
            .ok_or_else(|| CowenError::NotFound(format!("AppTicket not found for key '{}'", app_key)))?;
        
        Ok(Ticket { 
            value: row.get::<&str, _>(0).unwrap_or_default().to_string(), 
            created_at: row.get::<DateTime<Utc>, _>(1).unwrap_or_else(|| Utc::now()) 
        })
    }

    async fn save_app_ticket(&self, app_key: &str, ticket: Ticket) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("MERGE cowen_ticket AS target
                      USING (SELECT @p1, @p2, @p3) AS source (app_key, ticket_value, created_at)
                      ON (target.app_key = source.app_key)
                      WHEN MATCHED THEN
                          UPDATE SET ticket_value = source.ticket_value, created_at = source.created_at
                      WHEN NOT MATCHED THEN
                          INSERT (app_key, ticket_value, created_at) VALUES (source.app_key, source.ticket_value, source.created_at);",
            &[&app_key, &ticket.value.as_str(), &ticket.created_at]
        ).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_app_ticket(&self, app_key: &str) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("DELETE FROM cowen_ticket WHERE app_key = @p1", &[&app_key])
            .await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> CowenResult<String> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let row = conn.query("SELECT code_value FROM cowen_permanent_code WHERE app_key = @p1 AND org_id = @p2 AND code_type = 'org_permanent'", &[&app_key, &org_id])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_row().await.map_err(|e| CowenError::Store(e.to_string()))?
            .ok_or_else(|| CowenError::NotFound(format!("OrgPermanentCode not found for app '{}' and org '{}'", app_key, org_id)))?;
        
        let val: &str = row.get(0).ok_or_else(|| CowenError::Store("Null value".to_string()))?;
        Ok(val.to_string())
    }

    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("MERGE cowen_permanent_code AS target
                      USING (SELECT @p1, @p2, 'org_permanent', @p3) AS source (app_key, org_id, code_type, code_value)
                      ON (target.app_key = source.app_key AND target.org_id = source.org_id AND target.code_type = source.code_type)
                      WHEN MATCHED THEN
                          UPDATE SET code_value = source.code_value, created_at = GETUTCDATE()
                      WHEN NOT MATCHED THEN
                          INSERT (app_key, org_id, code_type, code_value, created_at) VALUES (source.app_key, source.org_id, source.code_type, source.code_value, GETUTCDATE());",
            &[&app_key, &org_id, &code]
        ).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> CowenResult<String> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let row = conn.query("SELECT code_value FROM cowen_permanent_code WHERE app_key = @p1 AND org_id = @p2 AND user_id = @p3 AND code_type = 'user_permanent'", &[&app_key, &org_id, &user_id])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_row().await.map_err(|e| CowenError::Store(e.to_string()))?
            .ok_or_else(|| CowenError::NotFound(format!("UserPermanentCode not found for app '{}', org '{}' and user '{}'", app_key, org_id, user_id)))?;
        
        let val: &str = row.get(0).ok_or_else(|| CowenError::Store("Null value".to_string()))?;
        Ok(val.to_string())
    }

    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("MERGE cowen_permanent_code AS target
                      USING (SELECT @p1, @p2, @p3, 'user_permanent', @p4) AS source (app_key, org_id, user_id, code_type, code_value)
                      ON (target.app_key = source.app_key AND target.org_id = source.org_id AND target.user_id = source.user_id AND target.code_type = source.code_type)
                      WHEN MATCHED THEN
                          UPDATE SET code_value = source.code_value, created_at = GETUTCDATE()
                      WHEN NOT MATCHED THEN
                          INSERT (app_key, org_id, user_id, code_type, code_value, created_at) VALUES (source.app_key, source.org_id, source.user_id, source.code_type, source.code_value, GETUTCDATE());",
            &[&app_key, &org_id, &user_id, &code]
        ).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_token(&self, profile: &str, key: &str) -> CowenResult<String> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let row = conn.query("SELECT item_value FROM cowen_token WHERE profile = @p1 AND item_key = @p2", &[&profile, &key])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_row().await.map_err(|e| CowenError::Store(e.to_string()))?
            .ok_or_else(|| CowenError::NotFound(format!("Key '{}' not found in profile '{}'", key, profile)))?;
        
        let val: &str = row.get(0).ok_or_else(|| CowenError::Store("Null value".to_string()))?;
        Ok(val.to_string())
    }

    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let exp = Utc::now() + chrono::Duration::seconds(expires_in_secs as i64);
        conn.execute("MERGE cowen_token AS target
                      USING (SELECT @p1, @p2, @p3, @p4) AS source (profile, item_key, item_value, expires_at)
                      ON (target.profile = source.profile AND target.item_key = source.item_key)
                      WHEN MATCHED THEN
                          UPDATE SET item_value = source.item_value, expires_at = source.expires_at
                      WHEN NOT MATCHED THEN
                          INSERT (profile, item_key, item_value, expires_at) VALUES (source.profile, source.item_key, source.item_value, source.expires_at);",
            &[&profile, &key, &value, &exp]
        ).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn delete_token(&self, profile: &str, key: &str) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("DELETE FROM cowen_token WHERE profile = @p1 AND item_key = @p2", &[&profile, &key])
            .await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn list_tokens(&self, profile: &str) -> CowenResult<Vec<String>> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let rows = conn.query("SELECT item_key FROM cowen_token WHERE profile = @p1", &[&profile])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_first_result().await.map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(rows.into_iter().map(|r| r.get::<&str, _>(0).unwrap_or_default().to_string()).collect())
    }

    async fn save_audit(&self, entry: &AuditEntry) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let fields_json = serde_json::to_string(&entry.fields).unwrap_or_default();
        conn.execute("INSERT INTO cowen_audit (id, profile, [timestamp], level, target, message, fields) VALUES (@p1, @p2, @p3, @p4, @p5, @p6, @p7)",
            &[&entry.id.as_str(), &entry.profile.as_str(), &entry.timestamp, &entry.level.as_str(), &entry.target.as_str(), &entry.message.as_str(), &fields_json.as_str()]
        ).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn list_audit(&self, profile: &str, limit: usize) -> CowenResult<Vec<AuditEntry>> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let rows = conn.query("SELECT TOP (@p1) id, profile, [timestamp], level, target, message, fields FROM cowen_audit WHERE profile = @p2 ORDER BY [timestamp] DESC", &[&(limit as i64), &profile])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_first_result().await.map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(rows.into_iter().map(|r| AuditEntry {
            id: r.get::<&str, _>(0).unwrap_or_default().to_string(),
            profile: r.get::<&str, _>(1).unwrap_or_default().to_string(),
            timestamp: r.get::<DateTime<Utc>, _>(2).unwrap_or_else(|| Utc::now()),
            level: r.get::<&str, _>(3).unwrap_or_default().to_string(),
            target: r.get::<&str, _>(4).unwrap_or_default().to_string(),
            message: r.get::<&str, _>(5).unwrap_or_default().to_string(),
            fields: serde_json::from_str(r.get::<&str, _>(6).unwrap_or("{}")).unwrap_or_default(),
        }).collect())
    }

    async fn push_dlq(&self, msg: &DlqMessage) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("INSERT INTO cowen_dlq (profile, topic, payload, retry_count, error, created_at) VALUES (@p1, @p2, @p3, @p4, @p5, @p6)",
            &[&msg.profile.as_str(), &msg.topic.as_str(), &msg.payload.as_str(), &msg.retry_count, &msg.error, &msg.created_at]
        ).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn pop_dlq(&self, profile: &str, topic: &str) -> CowenResult<Option<DlqMessage>> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let row = conn.query("SELECT TOP (1) id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p1 AND topic = @p2", &[&profile, &topic])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_row().await.map_err(|e| CowenError::Store(e.to_string()))?;

        if let Some(r) = row {
            let id: i64 = r.get(0).unwrap_or(0);
            conn.execute("DELETE FROM cowen_dlq WHERE id = @p1", &[&id]).await.map_err(|e| CowenError::Store(e.to_string()))?;

            Ok(Some(DlqMessage {
                id: Some(id),
                profile: r.get::<&str, _>(1).unwrap_or_default().to_string(),
                topic: r.get::<&str, _>(2).unwrap_or_default().to_string(),
                payload: r.get::<&str, _>(3).unwrap_or_default().to_string(),
                retry_count: r.get::<i32, _>(4).unwrap_or(0),
                error: r.get::<&str, _>(5).map(|s| s.to_string()),
                created_at: r.get::<DateTime<Utc>, _>(6).unwrap_or_else(|| Utc::now()),
            }))
        } else {
            Ok(None)
        }
    }

    async fn list_dlq(&self, profile: &str, limit: usize) -> CowenResult<Vec<DlqMessage>> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let rows = conn.query("SELECT TOP (@p1) id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p2", &[&(limit as i64), &profile])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_first_result().await.map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(rows.into_iter().map(|r| DlqMessage {
            id: Some(r.get::<i64, _>(0).unwrap_or(0)),
            profile: r.get::<&str, _>(1).unwrap_or_default().to_string(),
            topic: r.get::<&str, _>(2).unwrap_or_default().to_string(),
            payload: r.get::<&str, _>(3).unwrap_or_default().to_string(),
            retry_count: r.get::<i32, _>(4).unwrap_or(0),
            error: r.get::<&str, _>(5).map(|s| s.to_string()),
            created_at: r.get::<DateTime<Utc>, _>(6).unwrap_or_else(|| Utc::now()),
        }).collect())
    }

    async fn list_all_dlq(&self, profile: &str) -> CowenResult<Vec<DlqMessage>> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let rows = conn.query("SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p1", &[&profile])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_first_result().await.map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(rows.into_iter().map(|r| DlqMessage {
            id: Some(r.get::<i64, _>(0).unwrap_or(0)),
            profile: r.get::<&str, _>(1).unwrap_or_default().to_string(),
            topic: r.get::<&str, _>(2).unwrap_or_default().to_string(),
            payload: r.get::<&str, _>(3).unwrap_or_default().to_string(),
            retry_count: r.get::<i32, _>(4).unwrap_or(0),
            error: r.get::<&str, _>(5).map(|s| s.to_string()),
            created_at: r.get::<DateTime<Utc>, _>(6).unwrap_or_else(|| Utc::now()),
        }).collect())
    }

    async fn get_dlq_by_id(&self, id: i64) -> CowenResult<Option<DlqMessage>> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let row = conn.query("SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE id = @p1", &[&id])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_row().await.map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(row.map(|r| DlqMessage {
            id: Some(r.get::<i64, _>(0).unwrap_or(0)),
            profile: r.get::<&str, _>(1).unwrap_or_default().to_string(),
            topic: r.get::<&str, _>(2).unwrap_or_default().to_string(),
            payload: r.get::<&str, _>(3).unwrap_or_default().to_string(),
            retry_count: r.get::<i32, _>(4).unwrap_or(0),
            error: r.get::<&str, _>(5).map(|s| s.to_string()),
            created_at: r.get::<DateTime<Utc>, _>(6).unwrap_or_else(|| Utc::now()),
        }))
    }

    async fn list_dlq_paged(&self, profile: &str, offset: usize, limit: usize) -> CowenResult<Vec<DlqMessage>> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let offset_val = offset as i64;
        let limit_val = limit as i64;
        let rows = conn.query("SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p1 ORDER BY id OFFSET @p2 ROWS FETCH NEXT @p3 ROWS ONLY", &[&profile, &offset_val, &limit_val])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_first_result().await.map_err(|e| CowenError::Store(e.to_string()))?;
        
        Ok(rows.into_iter().map(|r| DlqMessage {
            id: Some(r.get::<i64, _>(0).unwrap_or(0)),
            profile: r.get::<&str, _>(1).unwrap_or_default().to_string(),
            topic: r.get::<&str, _>(2).unwrap_or_default().to_string(),
            payload: r.get::<&str, _>(3).unwrap_or_default().to_string(),
            retry_count: r.get::<i32, _>(4).unwrap_or(0),
            error: r.get::<&str, _>(5).map(|s| s.to_string()),
            created_at: r.get::<DateTime<Utc>, _>(6).unwrap_or_else(|| Utc::now()),
        }).collect())
    }

    async fn delete_dlq_by_id(&self, id: i64) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("DELETE FROM cowen_dlq WHERE id = @p1", &[&id])
            .await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn clear_profile(&self, profile: &str) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("DELETE FROM cowen_config WHERE profile = @p1", &[&profile]).await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("DELETE FROM cowen_secret WHERE profile = @p1", &[&profile]).await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("DELETE FROM cowen_token WHERE profile = @p1", &[&profile]).await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("DELETE FROM cowen_audit WHERE profile = @p1", &[&profile]).await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("DELETE FROM cowen_dlq WHERE profile = @p1", &[&profile]).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn rename_profile(&self, old_name: &str, new_name: &str) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("UPDATE cowen_config SET profile = @p1 WHERE profile = @p2", &[&new_name, &old_name]).await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("UPDATE cowen_secret SET profile = @p1 WHERE profile = @p2", &[&new_name, &old_name]).await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("UPDATE cowen_token SET profile = @p1 WHERE profile = @p2", &[&new_name, &old_name]).await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("UPDATE cowen_audit SET profile = @p1 WHERE profile = @p2", &[&new_name, &old_name]).await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("UPDATE cowen_dlq SET profile = @p1 WHERE profile = @p2", &[&new_name, &old_name]).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }

    async fn list_all_profiles(&self) -> CowenResult<Vec<String>> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let rows = conn.query("SELECT DISTINCT profile FROM cowen_config", &[])
            .await.map_err(|e| CowenError::Store(e.to_string()))?
            .into_first_result().await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.get::<&str, _>(0).unwrap_or_default().to_string()).collect())
    }

    async fn raw_del(&self, key: &str) -> CowenResult<()> {
        let mut conn = self.pool.get().await.map_err(|e| CowenError::Store(e.to_string()))?;
        conn.execute("DELETE FROM cowen_config WHERE item_key = @p1", &[&key]).await.map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(())
    }
}

pub struct MssqlBuilder;

#[async_trait]
impl SqlBuilder for MssqlBuilder {
    fn scheme(&self) -> &str { "mssql" }
    async fn build(&self, _url: &str) -> CowenResult<Arc<dyn SqlDriver>> {
        // Simple mock implementation because deadpool_tiberius usage is complex to fix here
        Err(CowenError::Store("MSSQL not fully implemented in this phase".to_string()))
    }
}

inventory::submit! { SqlBuilderRegistration { builder: &MssqlBuilder } }
