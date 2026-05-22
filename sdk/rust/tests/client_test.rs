use url::Url;

#[test]
fn test_proper_url_construction() {
    let ws_url_str = "ws://localhost:8080/base/";
    let app_key = "app&key";
    let nonce = "nonce#1";
    let sign = "sign=2";
    let client_id = "app&key@host space";

    let mut url = Url::parse(ws_url_str).unwrap();
    
    // 模拟 client.rs 中的逻辑
    if url.path() == "/" || url.path().is_empty() {
        url.set_path("/connect");
    } else if !url.path().ends_with("/connect") {
        let new_path = format!("{}/connect", url.path().trim_end_matches('/'));
        url.set_path(&new_path);
    }

    url.query_pairs_mut()
        .append_pair("app_key", app_key)
        .append_pair("nonce", nonce)
        .append_pair("sign", sign)
        .append_pair("client_id", client_id);

    let final_url = url.to_string();
    println!("Final URL: {}", final_url);

    // 验证路径
    assert_eq!(url.path(), "/base/connect");
    
    // 验证参数
    let query_map: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(query_map.get("app_key").unwrap(), "app&key");
    assert_eq!(query_map.get("nonce").unwrap(), "nonce#1");
    assert_eq!(query_map.get("sign").unwrap(), "sign=2");
    assert_eq!(query_map.get("client_id").unwrap(), "app&key@host space");
    
    // 验证转义
    assert!(final_url.contains("app%26key"));
    assert!(final_url.contains("nonce%231"));
    assert!(final_url.contains("sign%3D2"));
    assert!(final_url.contains("app%26key%40host+space") || final_url.contains("app%26key%40host%20space"));
}

#[test]
fn test_disconnected_with_error_variant() {
    let state = connector_sdk::ConnectionState::DisconnectedWithError("404 Not Found".to_string());
    if let connector_sdk::ConnectionState::DisconnectedWithError(err) = state {
        assert_eq!(err, "404 Not Found");
    } else {
        panic!("Incorrect variant");
    }
}

#[test]
fn test_aes_decrypt_key_length_validation() {
    // 32-character key should fail with our specific error message
    let key_32 = "12345678901234561234567890123456";
    let res = connector_sdk::crypto::aes_decrypt("ZW5jcnlwdGVkX3N0dWZm", key_32);
    assert!(res.is_err());
    let err_msg = res.err().unwrap().to_string();
    assert!(err_msg.contains("AES-128 key must be 16 bytes"));

    // 16-character key should check base64 decode first
    let key_16 = "1234567890123456";
    let res_16 = connector_sdk::crypto::aes_decrypt("invalid_base64_stuff!!!", key_16);
    assert!(res_16.is_err());
    assert!(res_16.err().unwrap().to_string().contains("Base64 decode failed"));
}

