use std::borrow::Cow;

#[inline]
pub fn map_not_found(e: sqlx::Error, msg: &str) -> cowen_common::CowenError {
    match e {
        sqlx::Error::RowNotFound => cowen_common::CowenError::NotFound(msg.to_string()),
        _ => cowen_common::CowenError::Store(e.to_string()),
    }
}

#[inline]
pub fn map_dlq_row(
    r: (
        i32,
        String,
        String,
        String,
        i32,
        Option<String>,
        chrono::DateTime<chrono::Utc>,
    ),
) -> cowen_common::models::DlqMessage {
    cowen_common::models::DlqMessage {
        id: Some(r.0 as i64),
        profile: r.1,
        topic: r.2,
        payload: r.3,
        retry_count: r.4,
        error: r.5,
        created_at: r.6,
    }
}

pub fn adapt_sql(sql: &'static str, is_postgres: bool) -> Cow<'static, str> {
    if !is_postgres || !sql.contains('?') {
        return Cow::Borrowed(sql);
    }
    let mut out = String::with_capacity(sql.len() + 10);
    let mut count = 1;
    for c in sql.chars() {
        if c == '?' {
            out.push('$');
            out.push_str(&count.to_string());
            count += 1;
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

#[macro_export]
macro_rules! sqlx_get_string {
    ($pool:expr, $sql_template:expr, $is_postgres:expr, $profile:expr, $key:expr) => {{
        let sql = $crate::sql::macros::adapt_sql($sql_template, $is_postgres);
        let row: (String,) = sqlx::query_as(&sql)
            .bind($profile)
            .bind($key)
            .fetch_one($pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => cowen_common::CowenError::NotFound(format!(
                    "Key '{}' not found in profile '{}'",
                    $key, $profile
                )),
                _ => cowen_common::CowenError::Store(e.to_string()),
            })?;
        Ok(row.0)
    }};
}

#[macro_export]
macro_rules! sqlx_execute {
    ($pool:expr, $sql_template:expr, $is_postgres:expr, $profile:expr, $key:expr) => {{
        let sql = $crate::sql::macros::adapt_sql($sql_template, $is_postgres);
        sqlx::query(&sql)
            .bind($profile)
            .bind($key)
            .execute($pool)
            .await
            .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
        Ok(())
    }};
}

#[macro_export]
macro_rules! sqlx_list_strings {
    ($pool:expr, $sql_template:expr, $is_postgres:expr, $profile:expr) => {{
        let sql = $crate::sql::macros::adapt_sql($sql_template, $is_postgres);
        let rows: Vec<(String,)> = sqlx::query_as(&sql)
            .bind($profile)
            .fetch_all($pool)
            .await
            .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }};
}

#[macro_export]
macro_rules! sqlx_get_token {
    ($pool:expr, $sql_template:expr, $is_postgres:expr, $profile:expr, $err_msg:expr) => {{
        let sql = $crate::sql::macros::adapt_sql($sql_template, $is_postgres);
        let row: (
            String,
            chrono::DateTime<chrono::Utc>,
            chrono::DateTime<chrono::Utc>,
        ) = sqlx::query_as(&sql)
            .bind($profile)
            .fetch_one($pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => cowen_common::CowenError::NotFound($err_msg),
                _ => cowen_common::CowenError::Store(e.to_string()),
            })?;
        Ok(cowen_common::models::Token {
            value: row.0,
            expires_at: row.1,
            created_at: row.2,
        })
    }};
}

#[macro_export]
macro_rules! sqlx_delete_token {
    ($pool:expr, $sql_template:expr, $is_postgres:expr, $profile:expr) => {{
        let sql = $crate::sql::macros::adapt_sql($sql_template, $is_postgres);
        sqlx::query(&sql)
            .bind($profile)
            .execute($pool)
            .await
            .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
        Ok(())
    }};
}

#[macro_export]
macro_rules! define_sql_driver {
    (
        $driver_name:ident,
        $db_type:ty,
        $is_postgres:expr,
        $upsert_config:expr,
        $upsert_secret:expr,
        $upsert_token:expr,
        $upsert_ticket:expr,
        $upsert_app_token:expr,
        $upsert_tenant_token:expr,
        $upsert_permanent_code:expr
    ) => {
        #[async_trait::async_trait]
        impl $crate::sql::SqlDriver for $driver_name {
            async fn shutdown(&self) -> cowen_common::CowenResult<()> {
                self.pool.close().await;
                Ok(())
            }

                async fn get_config(&self, profile: &str, key: &str) -> cowen_common::CowenResult<String> {
        $crate::sqlx_get_string!(&self.pool, "SELECT item_value FROM cowen_config WHERE profile = ? AND item_key = ?", $is_postgres, profile, key)
    }

            async fn get_config_metadata(&self, profile: &str, key: &str) -> cowen_common::CowenResult<(u64, i64)> {
                let sql = $crate::sql::macros::adapt_sql("SELECT version, updated_at FROM cowen_config WHERE profile = ? AND item_key = ?", $is_postgres);
                let row: (i64, chrono::DateTime<chrono::Utc>) = sqlx::query_as(&sql)
                    .bind(profile)
                    .bind(key)
                    .fetch_one(&self.pool).await
                    .map_err(|e| $crate::sql::macros::map_not_found(e, &format!("Key '{}' not found in profile '{}'", key, profile)))?;
                Ok((row.0 as u64, row.1.timestamp()))
            }

            async fn get_config_full(&self, profile: &str, key: &str) -> cowen_common::CowenResult<cowen_common::models::Item> {
                let sql = $crate::sql::macros::adapt_sql("SELECT profile, item_key, item_value, version, updated_at FROM cowen_config WHERE profile = ? AND item_key = ?", $is_postgres);
                let row: (String, String, String, i64, chrono::DateTime<chrono::Utc>) = sqlx::query_as(&sql)
                    .bind(profile)
                    .bind(key)
                    .fetch_one(&self.pool).await
                    .map_err(|e| crate::sql::macros::map_not_found(e, &format!("Key '{}' not found in profile '{}'", key, profile)))?;
                Ok(cowen_common::models::Item {
                    profile: row.0,
                    key: row.1,
                    value: row.2,
                    version: row.3 as u64,
                    updated_at: row.4.timestamp(),
                })
            }

            async fn set_config(&self, profile: &str, key: &str, value: &str) -> cowen_common::CowenResult<()> {
                let sql = $crate::sql::macros::adapt_sql($upsert_config, $is_postgres);
                sqlx::query(&sql)
                    .bind(profile).bind(key).bind(value)
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

            async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> cowen_common::CowenResult<()> {
                let sql = $crate::sql::macros::adapt_sql("UPDATE cowen_config SET item_value = ?, version = version + 1 WHERE profile = ? AND item_key = ? AND version = ?", $is_postgres);
                let res = sqlx::query(&sql)
                    .bind(value).bind(profile).bind(key).bind(expected_version as i64)
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;

                if res.rows_affected() == 0 {
                    return Err(cowen_common::CowenError::Store("CAS failed: version mismatch or record not found".to_string()));
                }
                Ok(())
            }

                async fn list_configs(&self, profile: &str) -> cowen_common::CowenResult<Vec<String>> {
        $crate::sqlx_list_strings!(&self.pool, "SELECT item_key FROM cowen_config WHERE profile = ?", $is_postgres, profile)
    }

                async fn delete_config(&self, profile: &str, key: &str) -> cowen_common::CowenResult<()> {
        $crate::sqlx_execute!(&self.pool, "DELETE FROM cowen_config WHERE profile = ? AND item_key = ?", $is_postgres, profile, key)
    }

                async fn get_secret(&self, profile: &str, key: &str) -> cowen_common::CowenResult<String> {
        $crate::sqlx_get_string!(&self.pool, "SELECT item_value FROM cowen_secret WHERE profile = ? AND item_key = ?", $is_postgres, profile, key)
    }

            async fn set_secret(&self, profile: &str, key: &str, value: &str) -> cowen_common::CowenResult<()> {
                let sql = $crate::sql::macros::adapt_sql($upsert_secret, $is_postgres);
                sqlx::query(&sql)
                    .bind(profile).bind(key).bind(value)
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

                async fn delete_secret(&self, profile: &str, key: &str) -> cowen_common::CowenResult<()> {
        $crate::sqlx_execute!(&self.pool, "DELETE FROM cowen_secret WHERE profile = ? AND item_key = ?", $is_postgres, profile, key)
    }

                async fn list_secrets(&self, profile: &str) -> cowen_common::CowenResult<Vec<String>> {
        $crate::sqlx_list_strings!(&self.pool, "SELECT item_key FROM cowen_secret WHERE profile = ?", $is_postgres, profile)
    }

                async fn get_access_token(&self, profile: &str) -> cowen_common::CowenResult<cowen_common::models::Token> {
        $crate::sqlx_get_token!(&self.pool, "SELECT token_value, expires_at, created_at FROM cowen_tenant_token WHERE profile = ? AND token_type = 'access_token'", $is_postgres, profile, format!("AccessToken not found for profile '{}'", profile))
    }

            async fn save_access_token(&self, profile: &str, token: cowen_common::models::Token) -> cowen_common::CowenResult<()> {
                let sql = $crate::sql::macros::adapt_sql($upsert_tenant_token, $is_postgres);
                sqlx::query(&sql)
                    .bind(profile).bind(token.value).bind(token.expires_at).bind(token.created_at).bind("access_token")
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

                async fn delete_access_token(&self, profile: &str) -> cowen_common::CowenResult<()> {
        $crate::sqlx_delete_token!(&self.pool, "DELETE FROM cowen_tenant_token WHERE profile = ? AND token_type = 'access_token'", $is_postgres, profile)
    }

                async fn get_refresh_token(&self, profile: &str) -> cowen_common::CowenResult<cowen_common::models::Token> {
        $crate::sqlx_get_token!(&self.pool, "SELECT token_value, expires_at, created_at FROM cowen_tenant_token WHERE profile = ? AND token_type = 'refresh_token'", $is_postgres, profile, format!("RefreshToken not found for profile '{}'", profile))
    }

            async fn save_refresh_token(&self, profile: &str, token: cowen_common::models::Token) -> cowen_common::CowenResult<()> {
                let sql = $crate::sql::macros::adapt_sql($upsert_tenant_token, $is_postgres);
                sqlx::query(&sql)
                    .bind(profile).bind(token.value).bind(token.expires_at).bind(token.created_at).bind("refresh_token")
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

                async fn delete_refresh_token(&self, profile: &str) -> cowen_common::CowenResult<()> {
        $crate::sqlx_delete_token!(&self.pool, "DELETE FROM cowen_tenant_token WHERE profile = ? AND token_type = 'refresh_token'", $is_postgres, profile)
    }

            async fn get_app_access_token(&self, app_key: &str) -> cowen_common::CowenResult<cowen_common::models::Token> {
                let sql = $crate::sql::macros::adapt_sql("SELECT token_value, expires_at, created_at FROM cowen_app_token WHERE app_key = ?", $is_postgres);
                let row: (String, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) = sqlx::query_as(&sql)
                    .bind(app_key)
                    .fetch_one(&self.pool).await
                    .map_err(|e| crate::sql::macros::map_not_found(e, &format!("AppToken not found for key '{}'", app_key)))?;
                Ok(cowen_common::models::Token { value: row.0, expires_at: row.1, created_at: row.2 })
            }

            async fn save_app_access_token(&self, app_key: &str, token: cowen_common::models::Token) -> cowen_common::CowenResult<()> {
                let sql = $crate::sql::macros::adapt_sql($upsert_app_token, $is_postgres);
                sqlx::query(&sql)
                    .bind(app_key).bind(token.value).bind(token.expires_at).bind(token.created_at)
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

            async fn delete_app_access_token(&self, app_key: &str) -> cowen_common::CowenResult<()> {
                let sql = $crate::sql::macros::adapt_sql("DELETE FROM cowen_app_token WHERE app_key = ?", $is_postgres);
                sqlx::query(&sql)
                    .bind(app_key)
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

            async fn get_app_ticket(&self, app_key: &str) -> cowen_common::CowenResult<cowen_common::models::Ticket> {
                let sql = $crate::sql::macros::adapt_sql("SELECT ticket_value, created_at FROM cowen_ticket WHERE app_key = ?", $is_postgres);
                let row: (String, chrono::DateTime<chrono::Utc>) = sqlx::query_as(&sql)
                    .bind(app_key)
                    .fetch_one(&self.pool).await
                    .map_err(|e| crate::sql::macros::map_not_found(e, &format!("AppTicket not found for key '{}'", app_key)))?;
                Ok(cowen_common::models::Ticket { value: row.0, created_at: row.1 })
            }

            async fn save_app_ticket(&self, app_key: &str, ticket: cowen_common::models::Ticket) -> cowen_common::CowenResult<()> {
                let sql = $crate::sql::macros::adapt_sql($upsert_ticket, $is_postgres);
                sqlx::query(&sql)
                    .bind(app_key).bind(ticket.value).bind(ticket.created_at)
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

            async fn delete_app_ticket(&self, app_key: &str) -> cowen_common::CowenResult<()> {
                let sql = $crate::sql::macros::adapt_sql("DELETE FROM cowen_ticket WHERE app_key = ?", $is_postgres);
                sqlx::query(&sql)
                    .bind(app_key)
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

            async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> cowen_common::CowenResult<String> {
                let sql = $crate::sql::macros::adapt_sql("SELECT code_value FROM cowen_permanent_code WHERE app_key = ? AND org_id = ? AND code_type = 'org_permanent'", $is_postgres);
                let row: (String,) = sqlx::query_as(&sql)
                    .bind(app_key).bind(org_id)
                    .fetch_one(&self.pool).await
                    .map_err(|e| crate::sql::macros::map_not_found(e, &format!("OrgPermanentCode not found for app '{}' and org '{}'", app_key, org_id)))?;
                Ok(row.0)
            }

            async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> cowen_common::CowenResult<()> {
                let sql = $crate::sql::macros::adapt_sql($upsert_permanent_code, $is_postgres);
                sqlx::query(&sql)
                    .bind(app_key).bind(org_id).bind("").bind(code).bind("org_permanent")
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

            async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> cowen_common::CowenResult<String> {
                let sql = $crate::sql::macros::adapt_sql("SELECT code_value FROM cowen_permanent_code WHERE app_key = ? AND org_id = ? AND user_id = ? AND code_type = 'user_permanent'", $is_postgres);
                let row: (String,) = sqlx::query_as(&sql)
                    .bind(app_key).bind(org_id).bind(user_id)
                    .fetch_one(&self.pool).await
                    .map_err(|e| crate::sql::macros::map_not_found(e, &format!("UserPermanentCode not found for app '{}', org '{}' and user '{}'", app_key, org_id, user_id)))?;
                Ok(row.0)
            }

            async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> cowen_common::CowenResult<()> {
                let sql = $crate::sql::macros::adapt_sql($upsert_permanent_code, $is_postgres);
                sqlx::query(&sql)
                    .bind(app_key).bind(org_id).bind(user_id).bind(code).bind("user_permanent")
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

                async fn get_token(&self, profile: &str, key: &str) -> cowen_common::CowenResult<String> {
        $crate::sqlx_get_string!(&self.pool, "SELECT item_value FROM cowen_token WHERE profile = ? AND item_key = ?", $is_postgres, profile, key)
    }

            async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> cowen_common::CowenResult<()> {
                let exp = chrono::Utc::now() + chrono::Duration::seconds(expires_in_secs as i64);
                let sql = $crate::sql::macros::adapt_sql($upsert_token, $is_postgres);
                sqlx::query(&sql)
                    .bind(profile).bind(key).bind(value).bind(exp)
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

                async fn delete_token(&self, profile: &str, key: &str) -> cowen_common::CowenResult<()> {
        $crate::sqlx_execute!(&self.pool, "DELETE FROM cowen_token WHERE profile = ? AND item_key = ?", $is_postgres, profile, key)
    }

                async fn list_tokens(&self, profile: &str) -> cowen_common::CowenResult<Vec<String>> {
        $crate::sqlx_list_strings!(&self.pool, "SELECT item_key FROM cowen_token WHERE profile = ?", $is_postgres, profile)
    }

            async fn save_audit(&self, entry: &cowen_common::models::AuditEntry) -> cowen_common::CowenResult<()> {
                let fields_json = serde_json::to_string(&entry.fields).unwrap_or_default();
                let sql = $crate::sql::macros::adapt_sql("INSERT INTO cowen_audit (id, profile, timestamp, level, target, message, fields) VALUES (?, ?, ?, ?, ?, ?, ?)", $is_postgres);
                sqlx::query(&sql)
                    .bind(&entry.id).bind(&entry.profile).bind(entry.timestamp).bind(&entry.level).bind(&entry.target).bind(&entry.message).bind(fields_json)
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

            async fn list_audit(&self, profile: &str, limit: usize) -> cowen_common::CowenResult<Vec<cowen_common::models::AuditEntry>> {
                let sql = $crate::sql::macros::adapt_sql("SELECT id, profile, timestamp, level, target, message, fields FROM cowen_audit WHERE profile = ? ORDER BY timestamp DESC LIMIT ?", $is_postgres);
                let rows: Vec<(String, String, chrono::DateTime<chrono::Utc>, String, String, String, String)> = sqlx::query_as(&sql)
                    .bind(profile).bind(limit as i64)
                    .fetch_all(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;

                Ok(rows.into_iter().map(|r| cowen_common::models::AuditEntry {
                    id: r.0, profile: r.1, timestamp: r.2, level: r.3, target: r.4, message: r.5,
                    fields: serde_json::from_str(&r.6).unwrap_or_default(),
                }).collect())
            }

            async fn push_dlq(&self, msg: &cowen_common::models::DlqMessage) -> cowen_common::CowenResult<()> {
                let sql = $crate::sql::macros::adapt_sql("INSERT INTO cowen_dlq (profile, topic, payload, retry_count, error, created_at) VALUES (?, ?, ?, ?, ?, ?)", $is_postgres);
                sqlx::query(&sql)
                    .bind(&msg.profile).bind(&msg.topic).bind(&msg.payload).bind(msg.retry_count).bind(&msg.error).bind(msg.created_at)
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

            async fn pop_dlq(&self, profile: &str, topic: &str) -> cowen_common::CowenResult<Option<cowen_common::models::DlqMessage>> {
                let sql = $crate::sql::macros::adapt_sql("SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = ? AND topic = ? LIMIT 1", $is_postgres);
                let row: Option<(i32, String, String, String, i32, Option<String>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(&sql)
                    .bind(profile).bind(topic)
                    .fetch_optional(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;

                if let Some(r) = row {
                    let del_sql = $crate::sql::macros::adapt_sql("DELETE FROM cowen_dlq WHERE id = ?", $is_postgres);
                    sqlx::query(&del_sql)
                        .bind(r.0)
                        .execute(&self.pool).await
                        .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;

                    Ok(Some(cowen_common::models::DlqMessage {
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

            async fn list_dlq(&self, profile: &str, limit: usize) -> cowen_common::CowenResult<Vec<cowen_common::models::DlqMessage>> {
                let sql = $crate::sql::macros::adapt_sql("SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = ? LIMIT ?", $is_postgres);
                let rows: Vec<(i32, String, String, String, i32, Option<String>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(&sql)
                    .bind(profile).bind(limit as i64)
                    .fetch_all(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;

                Ok(rows.into_iter().map(crate::sql::macros::map_dlq_row).collect())
            }

            async fn list_all_dlq(&self, profile: &str) -> cowen_common::CowenResult<Vec<cowen_common::models::DlqMessage>> {
                let sql = $crate::sql::macros::adapt_sql("SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = ?", $is_postgres);
                let rows: Vec<(i32, String, String, String, i32, Option<String>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(&sql)
                    .bind(profile)
                    .fetch_all(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;

                Ok(rows.into_iter().map(crate::sql::macros::map_dlq_row).collect())
            }

            async fn get_dlq_by_id(&self, id: i64) -> cowen_common::CowenResult<Option<cowen_common::models::DlqMessage>> {
                let sql = $crate::sql::macros::adapt_sql("SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE id = ?", $is_postgres);
                let row: Option<(i32, String, String, String, i32, Option<String>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(&sql)
                    .bind(id as i32)
                    .fetch_optional(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;

                Ok(row.map(|r| cowen_common::models::DlqMessage {
                    id: Some(r.0 as i64), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
                }))
            }

            async fn list_dlq_paged(&self, profile: &str, offset: usize, limit: usize) -> cowen_common::CowenResult<Vec<cowen_common::models::DlqMessage>> {
                let sql = $crate::sql::macros::adapt_sql("SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = ? LIMIT ? OFFSET ?", $is_postgres);
                let rows: Vec<(i32, String, String, String, i32, Option<String>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(&sql)
                    .bind(profile).bind(limit as i64).bind(offset as i64)
                    .fetch_all(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;

                Ok(rows.into_iter().map(crate::sql::macros::map_dlq_row).collect())
            }

            async fn delete_dlq_by_id(&self, id: i64) -> cowen_common::CowenResult<()> {
                let sql = $crate::sql::macros::adapt_sql("DELETE FROM cowen_dlq WHERE id = ?", $is_postgres);
                sqlx::query(&sql)
                    .bind(id as i32)
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

            async fn migrate(&self) -> cowen_common::CowenResult<()> {
                use crate::sql::migration_trait::SchemaMigration;
                self.run_migration().await
            }

            async fn clear_profile(&self, profile: &str) -> cowen_common::CowenResult<()> {
                let queries = [
                    "DELETE FROM cowen_config WHERE profile = ?",
                    "DELETE FROM cowen_secret WHERE profile = ?",
                    "DELETE FROM cowen_token WHERE profile = ?",
                    "DELETE FROM cowen_audit WHERE profile = ?",
                    "DELETE FROM cowen_dlq WHERE profile = ?",
                    "DELETE FROM cowen_tenant_token WHERE profile = ?",
                ];

                for q in queries {
                    let sql = $crate::sql::macros::adapt_sql(q, $is_postgres);
                    if let Err(e) = sqlx::query(&sql).bind(profile).execute(&self.pool).await {
                        let err_msg = e.to_string();
                        if !err_msg.contains("no such table") {
                            return Err(cowen_common::CowenError::Store(err_msg));
                        }
                    }
                }
                Ok(())
            }

            async fn rename_profile(&self, old_name: &str, new_name: &str) -> cowen_common::CowenResult<()> {
                let tables = [
                    "cowen_config", "cowen_secret", "cowen_token",
                    "cowen_tenant_token", "cowen_audit", "cowen_dlq"
                ];

                let mut tx = self.pool.begin().await.map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                for table in tables {
                    let q = format!("UPDATE {} SET profile = ? WHERE profile = ?", table);
                    let sql = $crate::sql::macros::adapt_sql(Box::leak(q.into_boxed_str()), $is_postgres);
                    sqlx::query(&sql).bind(new_name).bind(old_name).execute(&mut *tx).await.map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                }
                tx.commit().await.map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

            async fn list_all_profiles(&self) -> cowen_common::CowenResult<Vec<String>> {
                let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT profile FROM cowen_config").fetch_all(&self.pool).await.map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(rows.into_iter().map(|r| r.0).collect())
            }

            async fn raw_del(&self, key: &str) -> cowen_common::CowenResult<()> {
                let sql = $crate::sql::macros::adapt_sql("DELETE FROM cowen_config WHERE item_key = ?", $is_postgres);
                sqlx::query(&sql).bind(key).execute(&self.pool).await.map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }
        }
    };
}

#[macro_export]
macro_rules! implement_schema_migration {
    ($driver:ident, $is_postgres:expr) => {
        #[async_trait::async_trait]
        impl $crate::sql::migration_trait::SchemaMigration for $driver {
            async fn get_current_version(&self) -> cowen_common::CowenResult<u32> {
                sqlx::query("CREATE TABLE IF NOT EXISTS schema_migrations (version INT PRIMARY KEY)")
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;

                let row: Option<(i32,)> = sqlx::query_as("SELECT MAX(version) FROM schema_migrations")
                    .fetch_optional(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;

                Ok(row.map_or(0, |r| r.0 as u32))
            }

            async fn apply_sql(&self, sql: &str) -> cowen_common::CowenResult<()> {
                sqlx::query(sql).execute(&self.pool).await.map_err(|e| cowen_common::CowenError::Store(format!("SQL apply error: {} ({})", e, sql)))?;
                Ok(())
            }

            async fn set_version(&self, version: u32) -> cowen_common::CowenResult<()> {
                let sql = if $is_postgres {
                    "INSERT INTO schema_migrations (version) VALUES ($1)"
                } else {
                    "INSERT INTO schema_migrations (version) VALUES (?)"
                };
                sqlx::query(sql)
                    .bind(version as i32)
                    .execute(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;
                Ok(())
            }

            async fn run_migration(&self) -> cowen_common::CowenResult<()> {
                let sql = if $is_postgres {
                    "SELECT column_name FROM information_schema.columns WHERE table_name = 'cowen_dlq' AND column_name = 'id'"
                } else {
                    "SELECT column_name FROM information_schema.columns WHERE table_name = 'cowen_dlq' AND column_name = 'id' AND table_schema = DATABASE()"
                };
                let row: Option<(String,)> = sqlx::query_as(sql)
                    .fetch_optional(&self.pool).await
                    .map_err(|e| cowen_common::CowenError::Store(e.to_string()))?;

                if row.is_none() {
                    tracing::info!(target: "sys", "Migrating cowen_dlq schema (adding 'id' column)...");
                    let alter_sql = if $is_postgres {
                        "ALTER TABLE cowen_dlq ADD COLUMN id SERIAL PRIMARY KEY"
                    } else {
                        "ALTER TABLE cowen_dlq ADD COLUMN id BIGINT PRIMARY KEY AUTO_INCREMENT FIRST"
                    };
                    self.apply_sql(alter_sql).await?;
                    tracing::info!(target: "sys", "DLQ migration completed.");
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
    };
}
