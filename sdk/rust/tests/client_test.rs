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
    // 32-character invalid hex key should fail with our specific error message
    let key_32 = "1234567890123456123456789012345g"; // 'g' makes it invalid hex
    let res = connector_sdk::crypto::aes_decrypt("ZW5jcnlwdGVkX3N0dWZm", key_32);
    assert!(res.is_err());
    let err_msg = res.err().unwrap().to_string();
    assert!(err_msg.contains("AES-128 key must be 16 bytes") || err_msg.contains("Failed to decode 32-character decryption key as hex"));

    // 16-character key should check base64 decode first
    let key_16 = "1234567890123456";
    let res_16 = connector_sdk::crypto::aes_decrypt("invalid_base64_stuff!!!", key_16);
    assert!(res_16.is_err());
    assert!(res_16.err().unwrap().to_string().contains("Base64 decode failed"));
}

#[tokio::test]
async fn test_client_start_fails_early_with_invalid_key_length() {
    let options = connector_sdk::ClientOptions {
        app_key: "dummy_key".to_string(),
        app_secret: "12345678901234567890".to_string(), // 20 bytes, not 16
        encrypt_key: None,
        gateway_url: "http://localhost:8080".to_string(),
        ..Default::default()
    };
    let client = connector_sdk::GatewayClient::new(options);
    let res = client.start().await;
    assert!(res.is_err());
    let err_msg = res.err().unwrap().to_string();
    assert!(err_msg.contains("Decryption key (encrypt_key or fallback app_secret) must be exactly 16 bytes"));
}

#[tokio::test]
async fn test_client_start_fails_early_with_invalid_encrypt_key_length() {
    let options = connector_sdk::ClientOptions {
        app_key: "dummy_key".to_string(),
        app_secret: "1234567890123456".to_string(), // 16 bytes, valid
        encrypt_key: Some("too_short_key".to_string()), // 13 bytes, invalid
        gateway_url: "http://localhost:8080".to_string(),
        ..Default::default()
    };
    let client = connector_sdk::GatewayClient::new(options);
    let res = client.start().await;
    assert!(res.is_err());
    let err_msg = res.err().unwrap().to_string();
    assert!(err_msg.contains("Decryption key (encrypt_key or fallback app_secret) must be exactly 16 bytes"));
}

#[test]
fn test_aes_decrypt_with_32_character_hex_key() {
    // 32-character valid hex key should pass the length check and hit pkcs7 or other decrypt errors
    let key_32_hex = "12345678901234561234567890123456";
    let res = connector_sdk::crypto::aes_decrypt("ZW5jcnlwdGVkX3N0dWZm", key_32_hex);
    assert!(res.is_err());
    let err_msg = res.err().unwrap().to_string();
    // In TDD Red phase, this will fail because it actually complains about "AES-128 key must be 16 bytes"
    // In Green phase, it will pass length check, try to decrypt, and fail on PKCS7 unpad or other logic
    assert!(!err_msg.contains("AES-128 key must be 16 bytes"));

    // 32-character invalid hex key should fail the validation
    let key_32_invalid = "1234567890123456123456789012345g"; // 'g' is not valid hex
    let res_inv = connector_sdk::crypto::aes_decrypt("ZW5jcnlwdGVkX3N0dWZm", key_32_invalid);
    assert!(res_inv.is_err());
}

#[tokio::test]
async fn test_client_start_passes_validation_with_32_character_hex_key() {
    let options = connector_sdk::ClientOptions {
        app_key: "dummy_key".to_string(),
        app_secret: "1234567890123456".to_string(),
        encrypt_key: Some("12345678901234561234567890123456".to_string()), // 32-char hex, valid
        gateway_url: "http://localhost:8080".to_string(),
        ..Default::default()
    };
    let client = connector_sdk::GatewayClient::new(options);
    let res = client.start().await;
    assert!(res.is_err());
    let err_msg = res.err().unwrap().to_string();
    // It should fail with network connection error, NOT decrypt key length validation error
    assert!(!err_msg.contains("Decryption key (encrypt_key or fallback app_secret) must be exactly 16 bytes for AES-128"));
}

#[test]
fn test_ent_auth_code_message_deserialization_missing_state() {
    let json_data = r#"{
        "id": "7e7fe844-a1e1-800c-2d66-e2a3728601cb",
        "msgType": "TEMP_AUTH_CODE",
        "appKey": "dqOk3anb",
        "appId": "",
        "time": "1779437358871",
        "headers": {},
        "bizContent": {
            "tempAuthCode": "test_temp_auth_code"
        }
    }"#;

    let msg: Result<connector_sdk::EntAuthCodeMessage, _> = serde_json::from_str(json_data);
    assert!(msg.is_ok(), "Expected EntAuthCodeMessage to be successfully deserialized even without state, but got: {:?}", msg.err());
    let parsed = msg.unwrap();
    assert_eq!(parsed.biz_content.temp_auth_code, "test_temp_auth_code");
    assert!(parsed.biz_content.state.is_none());
}

#[test]
fn test_ent_auth_code_message_deserialization_with_state() {
    let json_data = r#"{
        "id": "7e7fe844-a1e1-800c-2d66-e2a3728601cb",
        "msgType": "TEMP_AUTH_CODE",
        "appKey": "dqOk3anb",
        "appId": "",
        "time": "1779437358871",
        "headers": {},
        "bizContent": {
            "tempAuthCode": "test_temp_auth_code",
            "state": "my_custom_state"
        }
    }"#;

    let msg: Result<connector_sdk::EntAuthCodeMessage, _> = serde_json::from_str(json_data);
    assert!(msg.is_ok(), "Expected EntAuthCodeMessage to be successfully deserialized with state, but got: {:?}", msg.err());
    let parsed = msg.unwrap();
    assert_eq!(parsed.biz_content.temp_auth_code, "test_temp_auth_code");
    assert_eq!(parsed.biz_content.state, Some("my_custom_state".to_string()));
}



