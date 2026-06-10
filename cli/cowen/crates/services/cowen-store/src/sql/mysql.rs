use async_trait::async_trait;
use cowen_common::{CowenError, CowenResult};

use crate::sql::{SqlBuilder, SqlBuilderRegistration, SqlDriver};

use sqlx::{MySql, Pool};
use std::sync::Arc;

pub struct MySqlDriver {
    pool: Pool<MySql>,
}

impl MySqlDriver {
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }
}

crate::define_sql_driver! {
    MySqlDriver,
    sqlx::MySql,
    false,
    "INSERT INTO cowen_config (profile, item_key, item_value, version) VALUES (?, ?, ?, 1) ON DUPLICATE KEY UPDATE item_value=VALUES(item_value), version=version+1",
    "INSERT INTO cowen_secret (profile, item_key, item_value) VALUES (?, ?, ?) ON DUPLICATE KEY UPDATE item_value=VALUES(item_value)",
    "INSERT INTO cowen_token (profile, item_key, item_value, expires_at) VALUES (?, ?, ?, ?) ON DUPLICATE KEY UPDATE item_value=VALUES(item_value), expires_at=VALUES(expires_at)",
    "INSERT INTO cowen_ticket (app_key, ticket_value, created_at) VALUES (?, ?, ?) ON DUPLICATE KEY UPDATE ticket_value=VALUES(ticket_value), created_at=VALUES(created_at)",
    "INSERT INTO cowen_app_token (app_key, token_value, expires_at, created_at) VALUES (?, ?, ?, ?) ON DUPLICATE KEY UPDATE token_value=VALUES(token_value), expires_at=VALUES(expires_at), created_at=VALUES(created_at)",
    "INSERT INTO cowen_tenant_token (profile, token_type, token_value, expires_at, created_at) VALUES (?, ?, ?, ?, ?) ON DUPLICATE KEY UPDATE token_value=VALUES(token_value), expires_at=VALUES(expires_at), created_at=VALUES(created_at)",
    "INSERT INTO cowen_permanent_code (app_key, org_id, user_id, code_value, code_type) VALUES (?, ?, ?, ?, ?) ON DUPLICATE KEY UPDATE code_value=VALUES(code_value)"
}

pub struct MySqlBuilder;

#[async_trait]
impl SqlBuilder for MySqlBuilder {
    fn scheme(&self) -> &str {
        "mysql"
    }
    async fn build(&self, url: &str) -> CowenResult<Arc<dyn SqlDriver>> {
        let pool = sqlx::MySqlPool::connect(url)
            .await
            .map_err(|e| CowenError::Store(e.to_string()))?;

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
            sqlx::query(sql)
                .execute(&pool)
                .await
                .map_err(|e| CowenError::Store(e.to_string()))?;
        }

        Ok(Arc::new(MySqlDriver::new(pool)))
    }
}

inventory::submit! { SqlBuilderRegistration { builder: &MySqlBuilder } }

crate::implement_schema_migration! {MySqlDriver, false}
