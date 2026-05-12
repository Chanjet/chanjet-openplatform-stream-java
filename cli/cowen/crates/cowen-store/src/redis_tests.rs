#[cfg(test)]
mod tests {
    use crate::redis_store::RedisStore;
    use cowen_common::store::Store;
    use redis::AsyncCommands;

    #[tokio::test]
    async fn test_redis_store_sharing() -> Result<(), Box<dyn std::error::Error>> {
        let client = redis::Client::open("redis://127.0.0.1:6379/0")?;
        let conn1 = client.get_multiplexed_tokio_connection().await?;
        let conn2 = client.get_multiplexed_tokio_connection().await?;

        let store1 = RedisStore::new(conn1, "redis://127.0.0.1:6379/0".to_string());
        let store2 = RedisStore::new(conn2, "redis://127.0.0.1:6379/0".to_string());

        let token = cowen_common::models::Token {
            value: "shared_token".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            created_at: chrono::Utc::now(),
        };

        store1.save_app_access_token("AK_TEST", token.clone()).await?;
        
        let loaded = store2.get_app_access_token("AK_TEST").await?;
        assert_eq!(loaded.value, "shared_token");
        
        Ok(())
    }
}
