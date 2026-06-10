use cowen_common::store::Store;
use cowen_store::FileStore;
use tempfile::tempdir;

#[tokio::test]
async fn test_file_store_basic_crud() {
    let tmp = tempdir().unwrap();
    let store = FileStore::new(tmp.path(), Some("fingerprint")).unwrap();

    // Config
    store
        .set_config("test_profile", "key1", "val1")
        .await
        .unwrap();
    let val = store.get_config("test_profile", "key1").await.unwrap();
    assert_eq!(val, "val1");

    let configs = store.list_configs("test_profile").await.unwrap();
    assert!(configs.contains(&"key1".to_string()));

    store.delete_config("test_profile", "key1").await.unwrap();
    assert!(store.get_config("test_profile", "key1").await.is_err());
}

#[tokio::test]
async fn test_file_store_secret() {
    let tmp = tempdir().unwrap();
    let store = FileStore::new(tmp.path(), Some("fingerprint")).unwrap();

    store.set_secret("p1", "s1", "v1").await.unwrap();
    assert_eq!(store.get_secret("p1", "s1").await.unwrap(), "v1");
}

#[tokio::test]
async fn test_file_store_profiles() {
    let tmp = tempdir().unwrap();
    let store = FileStore::new(tmp.path(), Some("fingerprint")).unwrap();

    store.set_config("p1", "k1", "v1").await.unwrap();
    store.set_config("p2", "k2", "v2").await.unwrap();

    let profiles = store.list_all_profiles().await.unwrap();
    assert!(profiles.contains(&"p1".to_string()));
    assert!(profiles.contains(&"p2".to_string()));
}
