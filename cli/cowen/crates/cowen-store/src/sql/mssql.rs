#![cfg(feature = "mssql")]
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use std::sync::Arc;
use deadpool_tiberius::{Pool, Manager};
use crate::sql::{SqlDriver, SqlBuilder};
use crate::{AuditEntry, DlqMessage, Item};

pub struct MssqlDriver {
    pool: Pool,
}

impl MssqlDriver {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    async fn execute(&self, sql: &str, params: &[&dyn tiberius::ToSql]) -> Result<()> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        conn.execute(sql, params).await?;
        Ok(())
    }
}

#[async_trait]
impl SqlDriver for MssqlDriver {
    async fn get_config(&self, profile: &str, key: &str) -> Result<String> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query("SELECT item_value FROM cowen_config WHERE profile = @p1 AND item_key = @p2", &[&profile, &key])
            .await?.into_row().await?
            .ok_or_else(|| anyhow!("Config not found: {}/{}", profile, key))?;
        
        let val: &str = row.get(0).ok_or_else(|| anyhow!("Null value in config"))?;
        Ok(val.to_string())
    }
    async fn get_config_metadata(&self, profile: &str, key: &str) -> Result<(u64, i64)> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query("SELECT version, updated_at FROM cowen_config WHERE profile = @p1 AND item_key = @p2", &[&profile, &key])
            .await?.into_row().await?
            .ok_or_else(|| anyhow!("Config metadata not found: {}/{}", profile, key))?;
        
        let version: i32 = row.get(0).unwrap_or(0);
        let updated_at: chrono::DateTime<chrono::Utc> = row.get(1).unwrap_or_else(chrono::Utc::now);
        Ok((version as u64, updated_at.timestamp()))
    }

    async fn get_config_full(&self, profile: &str, key: &str) -> Result<Item> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query(
            "SELECT profile, item_key, item_value, version, updated_at FROM cowen_config WHERE profile = @p1 AND item_key = @p2",
            &[&profile, &key]
        ).await?.into_row().await?
        .ok_or_else(|| anyhow!("Config not found: {}/{}", profile, key))?;

        let p: &str = row.get(0).unwrap();
        let k: &str = row.get(1).unwrap();
        let v: &str = row.get(2).unwrap();
        let ver: i64 = row.get(3).unwrap();
        let ts: chrono::DateTime<chrono::Utc> = row.get(4).unwrap();

        Ok(Item {
            profile: p.to_string(),
            key: k.to_string(),
            value: v.to_string(),
            version: ver as u64,
            updated_at: ts.timestamp(),
        })
    }

    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()> {
        let sql = "
            MERGE cowen_config WITH (HOLDLOCK) AS target
            USING (SELECT @p1 AS p, @p2 AS k) AS source
            ON (target.profile = source.p AND target.item_key = source.k)
            WHEN MATCHED THEN
                UPDATE SET item_value = @p3, version = version + 1, updated_at = GETUTCDATE()
            WHEN NOT MATCHED THEN
                INSERT (profile, item_key, item_value, version)
                VALUES (@p1, @p2, @p3, 0);";
        self.execute(sql, &[&profile, &key, &value]).await
    }

    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> Result<()> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let res = conn.execute(
            "UPDATE cowen_config SET item_value = @p1, version = version + 1, updated_at = GETUTCDATE() WHERE profile = @p2 AND item_key = @p3 AND version = @p4",
            &[&value, &profile, &key, &(expected_version as i64)]
        ).await?;
        
        if res.total() == 0 {
            return Err(anyhow!("Conflict or missing record for conditional update"));
        }
        Ok(())
    }

    async fn list_configs(&self, profile: &str) -> Result<Vec<String>> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let rows = conn.query("SELECT item_key FROM cowen_config WHERE profile = @p1", &[&profile]).await?.into_first_result().await?;
        Ok(rows.into_iter().map(|r| r.get::<&str, _>(0).unwrap().to_string()).collect())
    }

    async fn delete_config(&self, profile: &str, key: &str) -> Result<()> {
        self.execute("DELETE FROM cowen_config WHERE profile = @p1 AND item_key = @p2", &[&profile, &key]).await
    }

    async fn get_secret(&self, profile: &str, key: &str) -> Result<String> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query("SELECT item_value FROM cowen_secret WHERE profile = @p1 AND item_key = @p2", &[&profile, &key])
            .await?.into_row().await?
            .ok_or_else(|| anyhow!("Secret not found"))?;
        let val: &str = row.get(0).unwrap();
        Ok(val.to_string())
    }

    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()> {
        let sql = "
            MERGE cowen_secret WITH (HOLDLOCK) AS target
            USING (SELECT @p1 AS p, @p2 AS k) AS source
            ON (target.profile = source.p AND target.item_key = source.k)
            WHEN MATCHED THEN
                UPDATE SET item_value = @p3, updated_at = GETUTCDATE()
            WHEN NOT MATCHED THEN
                INSERT (profile, item_key, item_value)
                VALUES (@p1, @p2, @p3);";
        self.execute(sql, &[&profile, &key, &value]).await
    }
    
    async fn delete_secret(&self, profile: &str, key: &str) -> Result<()> {
        self.execute("DELETE FROM cowen_secret WHERE profile = @p1 AND item_key = @p2", &[&profile, &key]).await
    }

    async fn list_secrets(&self, profile: &str) -> Result<Vec<String>> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let rows = conn.query("SELECT item_key FROM cowen_secret WHERE profile = @p1", &[&profile]).await?.into_first_result().await?;
        Ok(rows.into_iter().map(|r| r.get::<&str, _>(0).unwrap().to_string()).collect())
    }

    // --- Token Domain ---
    async fn get_access_token(&self, profile: &str) -> Result<cowen_common::models::Token> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query("SELECT token_value FROM cowen_tenant_token WHERE profile = @p1 AND token_type = 'access'", &[&profile])
            .await?.into_row().await?
            .ok_or_else(|| anyhow!("Access token not found for profile: {}", profile))?;
        let val: &str = row.get(0).unwrap();
        Ok(serde_json::from_str(val)?)
    }
    async fn save_access_token(&self, profile: &str, token: cowen_common::models::Token) -> Result<()> {
        let val = serde_json::to_string(&token)?;
        let sql = "
            MERGE cowen_tenant_token WITH (HOLDLOCK) AS target
            USING (SELECT @p1 AS p, 'access' AS t) AS source
            ON (target.profile = source.p AND target.token_type = source.t)
            WHEN MATCHED THEN
                UPDATE SET token_value = @p2, expires_at = @p3
            WHEN NOT MATCHED THEN
                INSERT (profile, token_type, token_value, expires_at)
                VALUES (@p1, 'access', @p2, @p3);";
        self.execute(sql, &[&profile, &val, &token.expires_at]).await
    }
    async fn delete_access_token(&self, profile: &str) -> Result<()> {
        self.execute("DELETE FROM cowen_tenant_token WHERE profile = @p1 AND token_type = 'access'", &[&profile]).await
    }
    async fn get_app_access_token(&self, app_key: &str) -> Result<cowen_common::models::Token> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query("SELECT access_token FROM cowen_app_token WHERE app_key = @p1", &[&app_key])
            .await?.into_row().await?
            .ok_or_else(|| anyhow!("App access token not found for app_key: {}", app_key))?;
        let val: &str = row.get(0).unwrap();
        Ok(serde_json::from_str(val)?)
    }
    async fn save_app_access_token(&self, app_key: &str, token: cowen_common::models::Token) -> Result<()> {
        let val = serde_json::to_string(&token)?;
        let sql = "
            MERGE cowen_app_token WITH (HOLDLOCK) AS target
            USING (SELECT @p1 AS k) AS source
            ON (target.app_key = source.k)
            WHEN MATCHED THEN
                UPDATE SET access_token = @p2, expires_at = @p3
            WHEN NOT MATCHED THEN
                INSERT (app_key, access_token, expires_at)
                VALUES (@p1, @p2, @p3);";
        self.execute(sql, &[&app_key, &val, &token.expires_at]).await
    }

    // --- Ticket Domain ---
    async fn get_app_ticket(&self, app_key: &str) -> Result<cowen_common::models::Ticket> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query("SELECT ticket_value FROM cowen_ticket WHERE app_key = @p1", &[&app_key])
            .await?.into_row().await?
            .ok_or_else(|| anyhow!("App ticket not found for app_key: {}", app_key))?;
        let val: &str = row.get(0).unwrap();
        Ok(serde_json::from_str(val)?)
    }
    async fn save_app_ticket(&self, app_key: &str, ticket: cowen_common::models::Ticket) -> Result<()> {
        let val = serde_json::to_string(&ticket)?;
        let sql = "
            MERGE cowen_ticket WITH (HOLDLOCK) AS target
            USING (SELECT @p1 AS k) AS source
            ON (target.app_key = source.k)
            WHEN MATCHED THEN
                UPDATE SET ticket_value = @p2
            WHEN NOT MATCHED THEN
                INSERT (app_key, ticket_value)
                VALUES (@p1, @p2);";
        self.execute(sql, &[&app_key, &val]).await
    }

    async fn delete_app_ticket(&self, app_key: &str) -> Result<()> {
        self.execute("DELETE FROM cowen_ticket WHERE app_key = @p1", &[&app_key]).await
    }

    // --- Permanent Code Domain ---
    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> Result<String> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query("SELECT code_value FROM cowen_permanent_code WHERE app_key = @p1 AND org_id = @p2 AND user_id = '' AND code_type = 'org_permanent'", &[&app_key, &org_id])
            .await?.into_row().await?
            .ok_or_else(|| anyhow!("Org permanent code not found"))?;
        let val: &str = row.get(0).unwrap();
        Ok(val.to_string())
    }
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> Result<()> {
        let sql = "
            MERGE cowen_permanent_code WITH (HOLDLOCK) AS target
            USING (SELECT @p1 AS ak, @p2 AS oi, '' AS ui, 'org_permanent' AS ct) AS source
            ON (target.app_key = source.ak AND target.org_id = source.oi AND target.user_id = source.ui AND target.code_type = source.ct)
            WHEN MATCHED THEN
                UPDATE SET code_value = @p3
            WHEN NOT MATCHED THEN
                INSERT (app_key, org_id, user_id, code_type, code_value)
                VALUES (@p1, @p2, '', 'org_permanent', @p3);";
        self.execute(sql, &[&app_key, &org_id, &code]).await
    }
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> Result<String> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query("SELECT code_value FROM cowen_permanent_code WHERE app_key = @p1 AND org_id = @p2 AND user_id = @p3 AND code_type = 'user_permanent'", &[&app_key, &org_id, &user_id])
            .await?.into_row().await?
            .ok_or_else(|| anyhow!("User permanent code not found"))?;
        let val: &str = row.get(0).unwrap();
        Ok(val.to_string())
    }
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> Result<()> {
        let sql = "
            MERGE cowen_permanent_code WITH (HOLDLOCK) AS target
            USING (SELECT @p1 AS ak, @p2 AS oi, @p3 AS ui, 'user_permanent' AS ct) AS source
            ON (target.app_key = source.ak AND target.org_id = source.oi AND target.user_id = source.ui AND target.code_type = source.ct)
            WHEN MATCHED THEN
                UPDATE SET code_value = @p4
            WHEN NOT MATCHED THEN
                INSERT (app_key, org_id, user_id, code_type, code_value)
                VALUES (@p1, @p2, @p3, 'user_permanent', @p4);";
        self.execute(sql, &[&app_key, &org_id, &user_id, &code]).await
    }

    // --- Legacy Support ---
    async fn get_token(&self, profile: &str, key: &str) -> Result<String> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query(
            "SELECT item_value FROM cowen_token WHERE profile = @p1 AND item_key = @p2 AND (expires_at IS NULL OR expires_at > GETUTCDATE())",
            &[&profile, &key]
        ).await?.into_row().await?
        .ok_or_else(|| anyhow!("Token not found or expired"))?;
        let val: &str = row.get(0).unwrap();
        Ok(val.to_string())
    }

    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> Result<()> {
        let sql = "
            MERGE cowen_token WITH (HOLDLOCK) AS target
            USING (SELECT @p1 AS p, @p2 AS k) AS source
            ON (target.profile = source.p AND target.item_key = source.k)
            WHEN MATCHED THEN
                UPDATE SET item_value = @p3, expires_at = DATEADD(second, @p4, GETUTCDATE())
            WHEN NOT MATCHED THEN
                INSERT (profile, item_key, item_value, expires_at)
                VALUES (@p1, @p2, @p3, DATEADD(second, @p4, GETUTCDATE()));";
        self.execute(sql, &[&profile, &key, &value, &(expires_in_secs as i64)]).await
    }

    async fn delete_token(&self, profile: &str, key: &str) -> Result<()> {
        self.execute("DELETE FROM cowen_token WHERE profile = @p1 AND item_key = @p2", &[&profile, &key]).await
    }

    async fn list_tokens(&self, profile: &str) -> Result<Vec<String>> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let rows = conn.query("SELECT item_key FROM cowen_token WHERE profile = @p1 AND (expires_at IS NULL OR expires_at > GETUTCDATE())", &[&profile]).await?.into_first_result().await?;
        Ok(rows.into_iter().map(|r| r.get::<&str, _>(0).unwrap().to_string()).collect())
    }

    async fn save_audit(&self, entry: &AuditEntry) -> Result<()> {
        let fields_json = serde_json::to_string(&entry.fields)?;
        self.execute(
            "INSERT INTO cowen_audit (id, profile, timestamp, level, target, message, fields) VALUES (@p1, @p2, @p3, @p4, @p5, @p6, @p7)",
            &[&entry.id, &entry.profile, &entry.timestamp, &entry.level, &entry.target, &entry.message, &fields_json]
        ).await
    }

    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<AuditEntry>> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let rows = conn.query(
            "SELECT TOP (@p1) id, timestamp, profile, level, target, message, fields FROM cowen_audit WHERE profile = @p2 ORDER BY timestamp DESC",
            &[&(limit as i64), &profile]
        ).await?.into_first_result().await?;
        
        Ok(rows.into_iter().map(|row| AuditEntry {
            id: row.get::<&str, _>(0).unwrap().to_string(),
            timestamp: row.get(1).unwrap(),
            profile: row.get::<&str, _>(2).unwrap().to_string(),
            level: row.get::<&str, _>(3).unwrap().to_string(),
            target: row.get::<&str, _>(4).unwrap().to_string(),
            message: row.get::<&str, _>(5).unwrap().to_string(),
            fields: serde_json::from_str(row.get::<&str, _>(6).unwrap()).unwrap_or_default(),
        }).collect())
    }

    async fn push_dlq(&self, msg: &DlqMessage) -> Result<()> {
        self.execute(
            "INSERT INTO cowen_dlq (profile, topic, payload, retry_count, error) VALUES (@p1, @p2, @p3, @p4, @p5)",
            &[&msg.profile, &msg.topic, &msg.payload, &msg.retry_count, &msg.error]
        ).await
    }

    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<DlqMessage>> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query(
            "SELECT TOP 1 id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p1 AND topic = @p2 ORDER BY id ASC",
            &[&profile, &topic]
        ).await?.into_row().await?;

        if let Some(r) = row {
            let id: i64 = r.get(0).unwrap();
            conn.execute("DELETE FROM cowen_dlq WHERE id = @p1", &[&id]).await?;
            Ok(Some(DlqMessage {
                id: Some(id),
                profile: r.get::<&str, _>(1).unwrap().to_string(),
                topic: r.get::<&str, _>(2).unwrap().to_string(),
                payload: r.get::<&str, _>(3).unwrap().to_string(),
                retry_count: r.get(4).unwrap(),
                error: r.get::<&str, _>(5).map(|s| s.to_string()),
                created_at: r.get(6).unwrap(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<DlqMessage>> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let rows = conn.query(
            "SELECT TOP (@p1) id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p2 ORDER BY id DESC",
            &[&(limit as i64), &profile]
        ).await?.into_first_result().await?;
        
        Ok(rows.into_iter().map(|row| DlqMessage {
            id: Some(row.get(0).unwrap()),
            profile: row.get::<&str, _>(1).unwrap().to_string(),
            topic: row.get::<&str, _>(2).unwrap().to_string(),
            payload: row.get::<&str, _>(3).unwrap().to_string(),
            retry_count: row.get(4).unwrap(),
            error: row.get::<&str, _>(5).map(|s| s.to_string()),
            created_at: row.get(6).unwrap(),
        }).collect())
    }

    async fn list_all_dlq(&self, profile: &str) -> Result<Vec<DlqMessage>> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let rows = conn.query("SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p1", &[&profile]).await?.into_first_result().await?;
        
        Ok(rows.into_iter().map(|row| DlqMessage {
            id: Some(row.get(0).unwrap()),
            profile: row.get::<&str, _>(1).unwrap().to_string(),
            topic: row.get::<&str, _>(2).unwrap().to_string(),
            payload: row.get::<&str, _>(3).unwrap().to_string(),
            retry_count: row.get(4).unwrap(),
            error: row.get::<&str, _>(5).map(|s| s.to_string()),
            created_at: row.get(6).unwrap(),
        }).collect())
    }


    async fn clear_profile(&self, profile: &str) -> Result<()> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        for table in ["cowen_config", "cowen_secret", "cowen_token", "cowen_tenant_token", "cowen_audit", "cowen_dlq"] {
            let sql = format!("DELETE FROM {} WHERE profile = @p1", table);
            conn.execute(sql, &[&profile]).await?;
        }
        Ok(())
    }

    async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<()> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        for table in ["cowen_config", "cowen_secret", "cowen_token", "cowen_tenant_token", "cowen_audit", "cowen_dlq"] {
            let sql = format!("UPDATE {} SET profile = @p1 WHERE profile = @p2", table);
            conn.execute(sql, &[&new_name, &old_name]).await?;
        }
        Ok(())
    }

    async fn raw_del(&self, _key: &str) -> Result<()> { Ok(()) }

    async fn list_all_profiles(&self) -> Result<Vec<String>> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let rows = conn.query("SELECT DISTINCT profile FROM cowen_config", &[]).await?.into_first_result().await?;
        Ok(rows.into_iter().map(|r| r.get::<&str, _>(0).unwrap().to_string()).collect())
    }

}

pub struct MssqlBuilder;
#[async_trait]
impl SqlBuilder for MssqlBuilder {
    fn scheme(&self) -> &str { "mssql" }
    async fn build(&self, url: &str) -> Result<Arc<dyn SqlDriver>> {
        let mgr = Manager::from_jdbc_string(url)
            .map_err(|e| anyhow!("Failed to parse MSSQL URL for pool: {}", e))?;
        let pool = Pool::builder(mgr).max_size(16).build().map_err(|e| anyhow!("Pool build error: {}", e))?;
        
        let mut conn = pool.get().await.map_err(|e| anyhow!("Failed to connect to MSSQL: {}", e))?;
        
        let ddl = [
            "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='cowen_config' AND xtype='U')
             CREATE TABLE cowen_config (profile NVARCHAR(255) NOT NULL, item_key NVARCHAR(255) NOT NULL, item_value NVARCHAR(MAX) NOT NULL, version BIGINT DEFAULT 0, updated_at DATETIME2 DEFAULT GETUTCDATE(), PRIMARY KEY (profile, item_key))",
            "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='cowen_secret' AND xtype='U')
             CREATE TABLE cowen_secret (profile NVARCHAR(255) NOT NULL, item_key NVARCHAR(255) NOT NULL, item_value NVARCHAR(MAX) NOT NULL, updated_at DATETIME2 DEFAULT GETUTCDATE(), PRIMARY KEY (profile, item_key))",
            "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='cowen_token' AND xtype='U')
             CREATE TABLE cowen_token (profile NVARCHAR(255) NOT NULL, item_key NVARCHAR(255) NOT NULL, item_value NVARCHAR(MAX) NOT NULL, expires_at DATETIME2 NULL, PRIMARY KEY (profile, item_key))",
            "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='cowen_ticket' AND xtype='U')
             CREATE TABLE cowen_ticket (app_key NVARCHAR(255) PRIMARY KEY, ticket_value NVARCHAR(MAX) NOT NULL, created_at DATETIME2 DEFAULT GETUTCDATE())",
            "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='cowen_app_token' AND xtype='U')
             CREATE TABLE cowen_app_token (app_key NVARCHAR(255) PRIMARY KEY, access_token NVARCHAR(MAX) NOT NULL, expires_at DATETIME2 NOT NULL)",
            "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='cowen_tenant_token' AND xtype='U')
             CREATE TABLE cowen_tenant_token (profile NVARCHAR(255) NOT NULL, token_type NVARCHAR(50) NOT NULL, token_value NVARCHAR(MAX) NOT NULL, expires_at DATETIME2 NULL, PRIMARY KEY (profile, token_type))",
            "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='cowen_permanent_code' AND xtype='U')
             CREATE TABLE cowen_permanent_code (app_key NVARCHAR(255) NOT NULL, org_id NVARCHAR(255) NOT NULL, user_id NVARCHAR(255) DEFAULT '', code_type NVARCHAR(50) NOT NULL, code_value NVARCHAR(MAX) NOT NULL, created_at DATETIME2 DEFAULT GETUTCDATE(), PRIMARY KEY (app_key, org_id, user_id, code_type))",
            "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='cowen_audit' AND xtype='U')
             CREATE TABLE cowen_audit (id NVARCHAR(255) PRIMARY KEY, profile NVARCHAR(255) NOT NULL, timestamp DATETIME2 NOT NULL, level NVARCHAR(50) NOT NULL, target NVARCHAR(255) NOT NULL, message NVARCHAR(MAX) NOT NULL, fields NVARCHAR(MAX))",
            "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='cowen_dlq' AND xtype='U')
             CREATE TABLE cowen_dlq (id BIGINT IDENTITY(1,1) PRIMARY KEY, profile NVARCHAR(255) NOT NULL, topic NVARCHAR(255) NOT NULL, payload NVARCHAR(MAX) NOT NULL, retry_count INT DEFAULT 0, error NVARCHAR(MAX), created_at DATETIME2 DEFAULT GETUTCDATE())",
        ];

        for sql in ddl {
            conn.execute(sql, &[]).await?;
        }

        let _ = conn.execute("IF COL_LENGTH('cowen_tenant_token', 'created_at') IS NULL ALTER TABLE cowen_tenant_token ADD created_at DATETIME2 DEFAULT GETUTCDATE()", &[]).await;
        let _ = conn.execute("IF COL_LENGTH('cowen_app_token', 'created_at') IS NULL ALTER TABLE cowen_app_token ADD created_at DATETIME2 DEFAULT GETUTCDATE()", &[]).await;

        Ok(Arc::new(MssqlDriver::new(pool)))
    }
}

inventory::submit! { super::SqlBuilderRegistration { builder: &MssqlBuilder } }
