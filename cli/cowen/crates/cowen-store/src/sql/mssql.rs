#![cfg(feature = "mssql")]
use cowen_common::{CowenResult, CowenError};
use async_trait::async_trait;

use crate::sql::{SqlBuilder, SqlDriver};
use tiberius::{Client, Config, AuthMethod};
use tokio::net::TcpStream;
use tokio_util::compat::{TokioAsyncWriteCompatExt, Compat};
use std::sync::Arc;
use cowen_common::models::{Token, Ticket, Item, AuditEntry, DlqMessage};
use chrono::{DateTime, Utc};

pub struct MssqlDriver {
    config: Config,
}

impl MssqlDriver {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    async fn connect(&self) -> CowenResult<Client<Compat<TcpStream>>> {
        let addr = self.config.get_addr();
        let tcp = TcpStream::connect(addr).await
            .map_err(|e| CowenError::Store(format!("Failed to connect to MSSQL: {}", e)))?;
        tcp.set_nodelay(true)?;
        let client = Client::connect(self.config.clone(), tcp.compat_write()).await
            .map_err(|e| CowenError::Store(format!("MSSQL handshake failed: {}", e)))?;
        Ok(client)
    }
}

#[async_trait]
impl SqlDriver for MssqlDriver {
    async fn get_config(&self, profile: &str, key: &str) -> CowenResult<String> {
        let mut conn = self.connect().await?;
        let rows = conn.query("SELECT value FROM cowen_config WHERE profile = @p1 AND key = @p2", &[&profile, &key]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
        
        let row = rows.first().ok_or_else(|| CowenError::Store("Not found".to_string()))?;
        let val: &str = row.get(0).ok_or_else(|| CowenError::Store("Null value".to_string()))?;
        Ok(val.to_string())
    }

    async fn get_config_metadata(&self, profile: &str, key: &str) -> CowenResult<(u64, i64)> {
        let mut conn = self.connect().await?;
        let rows = conn.query("SELECT version, updated_at FROM cowen_config WHERE profile = @p1 AND key = @p2", &[&profile, &key]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
            
        let row = rows.first().ok_or_else(|| CowenError::Store("Not found".to_string()))?;
        let version: i64 = row.get(0).ok_or_else(|| CowenError::Store("Null version".to_string()))?;
        let updated_at: i64 = row.get(1).ok_or_else(|| CowenError::Store("Null timestamp".to_string()))?;
        Ok((version as u64, updated_at))
    }

    async fn get_config_full(&self, profile: &str, key: &str) -> CowenResult<Item> {
        let mut conn = self.connect().await?;
        let rows = conn.query("SELECT profile, key, value, version, updated_at FROM cowen_config WHERE profile = @p1 AND key = @p2", &[&profile, &key]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
            
        let row = rows.first().ok_or_else(|| CowenError::Store("Not found".to_string()))?;
        Ok(Item {
            profile: row.get::<&str, _>(0).unwrap().to_string(),
            key: row.get::<&str, _>(1).unwrap().to_string(),
            value: row.get::<&str, _>(2).unwrap().to_string(),
            version: row.get::<i64, _>(3).unwrap() as u64,
            updated_at: row.get::<i64, _>(4).unwrap(),
        })
    }

    async fn set_config(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        let now = Utc::now().timestamp();
        let sql = "MERGE INTO cowen_config WITH (HOLDLOCK) AS target 
                   USING (SELECT @p1 AS profile, @p2 AS key) AS source
                   ON (target.profile = source.profile AND target.key = source.key)
                   WHEN MATCHED THEN 
                       UPDATE SET value = @p3, version = version + 1, updated_at = @p4
                   WHEN NOT MATCHED THEN 
                       INSERT (profile, key, value, version, updated_at) VALUES (@p1, @p2, @p3, 1, @p4);";
        conn.execute(sql, &[&profile, &key, &value, &now]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }

    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        let now = Utc::now().timestamp();
        let res = conn.execute("UPDATE cowen_config SET value = @p1, version = version + 1, updated_at = @p2 WHERE profile = @p3 AND key = @p4 AND version = @p5",
            &[&value, &now, &profile, &key, &(expected_version as i64)]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
            
        if res.rows_affected().first().cloned().unwrap_or(0) == 0 {
            return Err(CowenError::Store("CAS failed".to_string()));
        }
        Ok(())
    }

    async fn list_configs(&self, profile: &str) -> CowenResult<Vec<String>> {
        let mut conn = self.connect().await?;
        let rows = conn.query("SELECT key FROM cowen_config WHERE profile = @p1", &[&profile]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
        Ok(rows.into_iter().map(|r| r.get::<&str, _>(0).unwrap().to_string()).collect())
    }

    async fn delete_config(&self, profile: &str, key: &str) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        conn.execute("DELETE FROM cowen_config WHERE profile = @p1 AND key = @p2", &[&profile, &key]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }

    async fn get_secret(&self, profile: &str, key: &str) -> CowenResult<String> {
        let mut conn = self.connect().await?;
        let rows = conn.query("SELECT value FROM cowen_secret WHERE profile = @p1 AND key = @p2", &[&profile, &key]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
            
        let row = rows.first().ok_or_else(|| CowenError::Store("Not found".to_string()))?;
        let val: &str = row.get(0).ok_or_else(|| CowenError::Store("Null value".to_string()))?;
        Ok(val.to_string())
    }

    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        let now = Utc::now().timestamp();
        let sql = "MERGE INTO cowen_secret WITH (HOLDLOCK) AS target 
                   USING (SELECT @p1 AS profile, @p2 AS key) AS source
                   ON (target.profile = source.profile AND target.key = source.key)
                   WHEN MATCHED THEN 
                       UPDATE SET value = @p3, updated_at = @p4
                   WHEN NOT MATCHED THEN 
                       INSERT (profile, key, value, updated_at) VALUES (@p1, @p2, @p3, @p4);";
        conn.execute(sql, &[&profile, &key, &value, &now]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }

    async fn delete_secret(&self, profile: &str, key: &str) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        conn.execute("DELETE FROM cowen_secret WHERE profile = @p1 AND key = @p2", &[&profile, &key]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }

    async fn list_secrets(&self, profile: &str) -> CowenResult<Vec<String>> {
        let mut conn = self.connect().await?;
        let rows = conn.query("SELECT key FROM cowen_secret WHERE profile = @p1", &[&profile]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
        Ok(rows.into_iter().map(|r| r.get::<&str, _>(0).unwrap().to_string()).collect())
    }

    async fn get_access_token(&self, profile: &str) -> CowenResult<Token> {
        let mut conn = self.connect().await?;
        let rows = conn.query("SELECT value, expires_at, created_at FROM cowen_token WHERE profile = @p1 AND key = 'access_token'", &[&profile]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
            
        let row = rows.first().ok_or_else(|| CowenError::Store("Not found".to_string()))?;
        Ok(Token {
            value: row.get::<&str, _>(0).unwrap().to_string(),
            expires_at: row.get::<DateTime<Utc>, _>(1).unwrap(),
            created_at: row.get::<DateTime<Utc>, _>(2).unwrap(),
        })
    }

    async fn save_access_token(&self, profile: &str, token: Token) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        let sql = "MERGE INTO cowen_token WITH (HOLDLOCK) AS target 
                   USING (SELECT @p1 AS profile, 'access_token' AS key) AS source
                   ON (target.profile = source.profile AND target.key = source.key)
                   WHEN MATCHED THEN 
                       UPDATE SET value = @p2, expires_at = @p3, created_at = @p4
                   WHEN NOT MATCHED THEN 
                       INSERT (profile, key, value, expires_at, created_at) VALUES (@p1, 'access_token', @p2, @p3, @p4);";
        conn.execute(sql, &[&profile, &token.value, &token.expires_at, &token.created_at]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }

    async fn delete_access_token(&self, profile: &str) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        conn.execute("DELETE FROM cowen_token WHERE profile = @p1 AND key = 'access_token'", &[&profile]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }

    async fn get_app_access_token(&self, app_key: &str) -> CowenResult<Token> {
        let mut conn = self.connect().await?;
        let key = format!("app_token:{}", app_key);
        let rows = conn.query("SELECT value, expires_at, created_at FROM cowen_token WHERE profile = 'global' AND key = @p1", &[&key]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
            
        let row = rows.first().ok_or_else(|| CowenError::Store("Not found".to_string()))?;
        Ok(Token {
            value: row.get::<&str, _>(0).unwrap().to_string(),
            expires_at: row.get::<DateTime<Utc>, _>(1).unwrap(),
            created_at: row.get::<DateTime<Utc>, _>(2).unwrap(),
        })
    }

    async fn save_app_access_token(&self, app_key: &str, token: Token) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        let key = format!("app_token:{}", app_key);
        let sql = "MERGE INTO cowen_token WITH (HOLDLOCK) AS target 
                   USING (SELECT 'global' AS profile, @p1 AS key) AS source
                   ON (target.profile = source.profile AND target.key = source.key)
                   WHEN MATCHED THEN 
                       UPDATE SET value = @p2, expires_at = @p3, created_at = @p4
                   WHEN NOT MATCHED THEN 
                       INSERT (profile, key, value, expires_at, created_at) VALUES ('global', @p1, @p2, @p3, @p4);";
        conn.execute(sql, &[&key, &token.value, &token.expires_at, &token.created_at]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }

    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<Ticket> {
        let mut conn = self.connect().await?;
        let key = format!("app_ticket:{}", app_key);
        let rows = conn.query("SELECT value, created_at FROM cowen_token WHERE profile = 'global' AND key = @p1", &[&key]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
            
        let row = rows.first().ok_or_else(|| CowenError::Store("Not found".to_string()))?;
        Ok(Ticket {
            value: row.get::<&str, _>(0).unwrap().to_string(),
            created_at: row.get::<DateTime<Utc>, _>(1).unwrap(),
        })
    }

    async fn save_app_ticket(&self, app_key: &str, ticket: Ticket) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        let key = format!("app_ticket:{}", app_key);
        let sql = "MERGE INTO cowen_token WITH (HOLDLOCK) AS target 
                   USING (SELECT 'global' AS profile, @p1 AS key) AS source
                   ON (target.profile = source.profile AND target.key = source.key)
                   WHEN MATCHED THEN 
                       UPDATE SET value = @p2, created_at = @p3
                   WHEN NOT MATCHED THEN 
                       INSERT (profile, key, value, created_at) VALUES ('global', @p1, @p2, @p3);";
        conn.execute(sql, &[&key, &ticket.value, &ticket.created_at]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }

    async fn delete_app_ticket(&self, app_key: &str) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        let key = format!("app_ticket:{}", app_key);
        conn.execute("DELETE FROM cowen_token WHERE profile = 'global' AND key = @p1", &[&key]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }

    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> CowenResult<String> {
        let mut conn = self.connect().await?;
        let key = format!("opc:{}:{}", app_key, org_id);
        let rows = conn.query("SELECT value FROM cowen_token WHERE profile = 'global' AND key = @p1", &[&key]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
            
        let row = rows.first().ok_or_else(|| CowenError::Store("Not found".to_string()))?;
        Ok(row.get::<&str, _>(0).unwrap().to_string())
    }

    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        let key = format!("opc:{}:{}", app_key, org_id);
        let sql = "MERGE INTO cowen_token WITH (HOLDLOCK) AS target 
                   USING (SELECT 'global' AS profile, @p1 AS key) AS source
                   ON (target.profile = source.profile AND target.key = source.key)
                   WHEN MATCHED THEN UPDATE SET value = @p2
                   WHEN NOT MATCHED THEN INSERT (profile, key, value) VALUES ('global', @p1, @p2);";
        conn.execute(sql, &[&key, &code]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }

    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> CowenResult<String> {
        let mut conn = self.connect().await?;
        let key = format!("upc:{}:{}:{}", app_key, org_id, user_id);
        let rows = conn.query("SELECT value FROM cowen_token WHERE profile = 'global' AND key = @p1", &[&key]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
            
        let row = rows.first().ok_or_else(|| CowenError::Store("Not found".to_string()))?;
        Ok(row.get::<&str, _>(0).unwrap().to_string())
    }

    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        let key = format!("upc:{}:{}:{}", app_key, org_id, user_id);
        let sql = "MERGE INTO cowen_token WITH (HOLDLOCK) AS target 
                   USING (SELECT 'global' AS profile, @p1 AS key) AS source
                   ON (target.profile = source.profile AND target.key = source.key)
                   WHEN MATCHED THEN UPDATE SET value = @p2
                   WHEN NOT MATCHED THEN INSERT (profile, key, value) VALUES ('global', @p1, @p2);";
        conn.execute(sql, &[&key, &code]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }

    async fn get_token(&self, profile: &str, key: &str) -> CowenResult<String> {
        let mut conn = self.connect().await?;
        let rows = conn.query("SELECT value FROM cowen_token WHERE profile = @p1 AND key = @p2", &[&profile, &key]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
            
        let row = rows.first().ok_or_else(|| CowenError::Store("Not found".to_string()))?;
        Ok(row.get::<&str, _>(0).unwrap().to_string())
    }

    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        let exp = Utc::now() + chrono::Duration::seconds(expires_in_secs as i64);
        let sql = "MERGE INTO cowen_token WITH (HOLDLOCK) AS target 
                   USING (SELECT @p1 AS profile, @p2 AS key) AS source
                   ON (target.profile = source.profile AND target.key = source.key)
                   WHEN MATCHED THEN UPDATE SET value = @p3, expires_at = @p4
                   WHEN NOT MATCHED THEN INSERT (profile, key, value, expires_at) VALUES (@p1, @p2, @p3, @p4);";
        conn.execute(sql, &[&profile, &key, &value, &exp]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }

    async fn delete_token(&self, profile: &str, key: &str) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        conn.execute("DELETE FROM cowen_token WHERE profile = @p1 AND key = @p2", &[&profile, &key]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }

    async fn list_tokens(&self, profile: &str) -> CowenResult<Vec<String>> {
        let mut conn = self.connect().await?;
        let rows = conn.query("SELECT key FROM cowen_token WHERE profile = @p1", &[&profile]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
        Ok(rows.into_iter().map(|r| r.get::<&str, _>(0).unwrap().to_string()).collect())
    }

    async fn save_audit(&self, entry: &AuditEntry) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        conn.execute("INSERT INTO cowen_audit (id, timestamp, profile, level, target, message, fields) VALUES (@p1, @p2, @p3, @p4, @p5, @p6, @p7)",
            &[&entry.id, &entry.timestamp, &entry.profile, &entry.level, &entry.target, &entry.message, &serde_json::to_string(&entry.fields).unwrap()]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }

    async fn list_audit(&self, profile: &str, limit: usize) -> CowenResult<Vec<AuditEntry>> {
        let mut conn = self.connect().await?;
        let limit_i64 = limit as i64;
        let rows = conn.query("SELECT TOP (@p1) id, timestamp, profile, level, target, message, fields FROM cowen_audit WHERE profile = @p2 ORDER BY timestamp DESC",
            &[&limit_i64, &profile]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
            
        Ok(rows.into_iter().map(|r| AuditEntry {
            id: r.get::<&str, _>(0).unwrap().to_string(),
            timestamp: r.get::<DateTime<Utc>, _>(1).unwrap(),
            profile: r.get::<&str, _>(2).unwrap().to_string(),
            level: r.get::<&str, _>(3).unwrap().to_string(),
            target: r.get::<&str, _>(4).unwrap().to_string(),
            message: r.get::<&str, _>(5).unwrap().to_string(),
            fields: serde_json::from_str(r.get::<&str, _>(6).unwrap()).unwrap_or_default(),
        }).collect())
    }

    async fn push_dlq(&self, msg: &DlqMessage) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        conn.execute("INSERT INTO cowen_dlq (profile, topic, payload, retry_count, error, created_at) VALUES (@p1, @p2, @p3, @p4, @p5, @p6)",
            &[&msg.profile, &msg.topic, &msg.payload, &msg.retry_count, &msg.error, &msg.created_at]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }

    async fn pop_dlq(&self, profile: &str, topic: &str) -> CowenResult<Option<DlqMessage>> {
        let mut conn = self.connect().await?;
        let rows = conn.query("SELECT TOP (1) id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p1 AND topic = @p2 ORDER BY id ASC",
            &[&profile, &topic]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
        
        if let Some(r) = rows.first() {
            let id: i64 = r.get(0).unwrap();
            conn.execute("DELETE FROM cowen_dlq WHERE id = @p1", &[&id]).await
                .map_err(|e| CowenError::Store(format!("MSSQL delete failed: {}", e)))?;
            Ok(Some(DlqMessage {
                id: Some(id),
                profile: r.get::<&str, _>(1).unwrap().to_string(),
                topic: r.get::<&str, _>(2).unwrap().to_string(),
                payload: r.get::<&str, _>(3).unwrap().to_string(),
                retry_count: r.get::<i32, _>(4).unwrap(),
                error: r.get::<&str, _>(5).map(|s| s.to_string()),
                created_at: r.get::<DateTime<Utc>, _>(6).unwrap(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn list_dlq(&self, profile: &str, limit: usize) -> CowenResult<Vec<DlqMessage>> {
        let mut conn = self.connect().await?;
        let rows = conn.query(
            "SELECT TOP (@p1) id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p2 ORDER BY id DESC",
            &[&(limit as i64), &profile]
        ).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
        
        Ok(rows.into_iter().map(|r| DlqMessage {
            id: Some(r.get::<i64, _>(0).unwrap()),
            profile: r.get::<&str, _>(1).unwrap().to_string(),
            topic: r.get::<&str, _>(2).unwrap().to_string(),
            payload: r.get::<&str, _>(3).unwrap().to_string(),
            retry_count: r.get::<i32, _>(4).unwrap(),
            error: r.get::<&str, _>(5).map(|s| s.to_string()),
            created_at: r.get::<DateTime<Utc>, _>(6).unwrap(),
        }).collect())
    }

    async fn list_all_dlq(&self, profile: &str) -> CowenResult<Vec<DlqMessage>> {
        let mut conn = self.connect().await?;
        let rows = conn.query("SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p1", &[&profile]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
        Ok(rows.into_iter().map(|r| DlqMessage {
            id: Some(r.get::<i64, _>(0).unwrap()),
            profile: r.get::<&str, _>(1).unwrap().to_string(),
            topic: r.get::<&str, _>(2).unwrap().to_string(),
            payload: r.get::<&str, _>(3).unwrap().to_string(),
            retry_count: r.get::<i32, _>(4).unwrap(),
            error: r.get::<&str, _>(5).map(|s| s.to_string()),
            created_at: r.get::<DateTime<Utc>, _>(6).unwrap(),
        }).collect())
    }

    async fn clear_profile(&self, profile: &str) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        let queries = ["DELETE FROM cowen_config WHERE profile = @p1", "DELETE FROM cowen_secret WHERE profile = @p1", "DELETE FROM cowen_token WHERE profile = @p1", "DELETE FROM cowen_audit WHERE profile = @p1", "DELETE FROM cowen_dlq WHERE profile = @p1"];
        for sql in &queries {
            conn.execute(*sql, &[&profile]).await
                .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        }
        Ok(())
    }

    async fn rename_profile(&self, old_name: &str, new_name: &str) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        let queries = ["UPDATE cowen_config SET profile = @p1 WHERE profile = @p2", "UPDATE cowen_secret SET profile = @p1 WHERE profile = @p2", "UPDATE cowen_token SET profile = @p1 WHERE profile = @p2", "UPDATE cowen_audit SET profile = @p1 WHERE profile = @p2", "UPDATE cowen_dlq SET profile = @p1 WHERE profile = @p2"];
        for sql in &queries {
            conn.execute(*sql, &[&new_name, &old_name]).await
                .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        }
        Ok(())
    }

    async fn list_all_profiles(&self) -> CowenResult<Vec<String>> {
        let mut conn = self.connect().await?;
        let rows = conn.query("SELECT DISTINCT profile FROM cowen_config", &[]).await
            .map_err(|e| CowenError::Store(format!("MSSQL query failed: {}", e)))?
            .into_first_result().await
            .map_err(|e| CowenError::Store(format!("MSSQL fetch failed: {}", e)))?;
        Ok(rows.into_iter().map(|r| r.get::<&str, _>(0).unwrap().to_string()).collect())
    }

    async fn raw_del(&self, key: &str) -> CowenResult<()> {
        let mut conn = self.connect().await?;
        conn.execute("DELETE FROM cowen_config WHERE key = @p1", &[&key]).await
            .map_err(|e| CowenError::Store(format!("MSSQL execute failed: {}", e)))?;
        Ok(())
    }
}

pub struct MssqlBuilder;

#[async_trait]
impl SqlBuilder for MssqlBuilder {
    fn scheme(&self) -> &str { "mssql" }
    async fn build(&self, url: &str) -> CowenResult<Arc<dyn SqlDriver>> {
        let config = Config::from_ado_string(url).map_err(|e| CowenError::Store(format!("Invalid MSSQL URL: {}", e)))?;
        Ok(Arc::new(MssqlDriver::new(config)))
    }
}

inventory::submit! { crate::sql::SqlBuilderRegistration { builder: &MssqlBuilder } }
