#[cfg(test)]
mod tests {
    use crate::RedisStore;
    use cowen_common::store::Store;
    use cowen_common::CowenError;

    #[tokio::test]
    async fn test_redis_store_sharing() -> Result<(), Box<dyn std::error::Error>> {
        let client = redis::Client::open("redis://127.0.0.1:6379/0").map_err(|e| CowenError::Store(e.to_string()))?;
        let conn1 = client.get_multiplexed_tokio_connection().await.map_err(|e| CowenError::Store(e.to_string()))?;
        let conn2 = client.get_multiplexed_tokio_connection().await.map_err(|e| CowenError::Store(e.to_string()))?;

        let store1 = RedisStore::new(conn1, "redis://127.0.0.1:6379/0".to_string());
        let store2 = RedisStore::new(conn2, "redis://127.0.0.1:6379/0".to_string());

        store1.set_config("test", "key1", "val1").await?;
        let val = store2.get_config("test", "key1").await?;
        assert_eq!(val, "val1");

        Ok(())
    }
}
