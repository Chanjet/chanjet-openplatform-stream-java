use async_trait::async_trait;
use cowen_common::{CowenError, CowenResult};

use crate::sql::{SqlBuilder, SqlBuilderRegistration, SqlDriver};

use sqlx::{Pool, Postgres};
use std::sync::Arc;

pub struct PostgresDriver {
    pool: Pool<Postgres>,
}

impl PostgresDriver {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

crate::define_sql_driver! {
    PostgresDriver,
    sqlx::Postgres,
    true,
    "INSERT INTO cowen_config (profile, item_key, item_value, version) VALUES ($1, $2, $3, 1) ON CONFLICT (profile, item_key) DO UPDATE SET item_value=EXCLUDED.item_value, version=cowen_config.version+1",
    "INSERT INTO cowen_secret (profile, item_key, item_value) VALUES ($1, $2, $3) ON CONFLICT (profile, item_key) DO UPDATE SET item_value=EXCLUDED.item_value",
    "INSERT INTO cowen_token (profile, item_key, item_value, expires_at) VALUES ($1, $2, $3, $4) ON CONFLICT (profile, item_key) DO UPDATE SET item_value=EXCLUDED.item_value, expires_at=EXCLUDED.expires_at",
    "INSERT INTO cowen_ticket (app_key, ticket_value, created_at) VALUES ($1, $2, $3) ON CONFLICT (app_key) DO UPDATE SET ticket_value=EXCLUDED.ticket_value, created_at=EXCLUDED.created_at",
    "INSERT INTO cowen_app_token (app_key, token_value, expires_at, created_at) VALUES ($1, $2, $3, $4) ON CONFLICT (app_key) DO UPDATE SET token_value=EXCLUDED.token_value, expires_at=EXCLUDED.expires_at, created_at=EXCLUDED.created_at",
    "INSERT INTO cowen_tenant_token (profile, token_value, expires_at, created_at, token_type) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (profile, token_type) DO UPDATE SET token_value=EXCLUDED.token_value, expires_at=EXCLUDED.expires_at, created_at=EXCLUDED.created_at",
    "INSERT INTO cowen_permanent_code (app_key, org_id, user_id, code_value, code_type) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (app_key, org_id, user_id, code_type) DO UPDATE SET code_value=EXCLUDED.code_value"
}

pub struct PostgresBuilder;

#[async_trait]
impl SqlBuilder for PostgresBuilder {
    fn scheme(&self) -> &str {
        "postgres"
    }
    async fn build(&self, url: &str) -> CowenResult<Arc<dyn SqlDriver>> {
        let pool = sqlx::PgPool::connect(url)
            .await
            .map_err(|e| CowenError::Store(e.to_string()))?;

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
            sqlx::query(sql)
                .execute(&pool)
                .await
                .map_err(|e| CowenError::Store(e.to_string()))?;
        }

        // Indices
        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_profile_ts ON cowen_audit (profile, timestamp)",
        )
        .execute(&pool)
        .await;
        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_dlq_profile_topic ON cowen_dlq (profile, topic)",
        )
        .execute(&pool)
        .await;

        Ok(Arc::new(PostgresDriver::new(pool)))
    }
}

inventory::submit! { SqlBuilderRegistration { builder: &PostgresBuilder } }

crate::implement_schema_migration! {PostgresDriver, true}
