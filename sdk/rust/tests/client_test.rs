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

#[test]
fn test_aes_decrypt_key_trimming() {
    // A valid 16-character key wrapped with spaces, newlines AND zero-width spaces (\u{200b})
    let key_with_whitespace = "\n \u{200b}1234567890123456 \n\u{200b}";
    // We expect it to be successfully sanitized to 16 bytes.
    // If it is, it should bypass the length check and try to decrypt (failing on PKCS7 unpad or other decrypt errors)
    // Instead of failing with "AES-128 key must be 16 bytes"
    let res = connector_sdk::crypto::aes_decrypt("ZW5jcnlwdGVkX3N0dWZm", key_with_whitespace);
    assert!(res.is_err());
    let err_msg = res.err().unwrap().to_string();
    assert!(!err_msg.contains("AES-128 key must be 16 bytes"));
    
    // Also test 32-character hex key with whitespaces and zero-width non-joiners (\u{200c}, \u{feff})
    let key_32_with_whitespace = "  \u{200c}12345678901234561234567890123456 \n\u{feff}";
    let res_32 = connector_sdk::crypto::aes_decrypt("ZW5jcnlwdGVkX3N0dWZm", key_32_with_whitespace);
    assert!(res_32.is_err());
    let err_32 = res_32.err().unwrap().to_string();
    assert!(!err_32.contains("AES-128 key must be 16 bytes"));
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
    // Use timeout to prevent hanging forever in the connection loop since local port 8080 is not listening.
    // If it times out, it means the validation has successfully passed and we entered the connect loop.
    // If validation failed, it would have returned Err(...) immediately.
    let res = tokio::time::timeout(std::time::Duration::from_millis(500), client.start()).await;
    assert!(res.is_err(), "Expected client.start() to hang in connection loop (timeout), indicating validation passed successfully");
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

fn aes_encrypt(plaintext: &str, key: &[u8]) -> String {
    use aes::cipher::generic_array::GenericArray;
    use aes::cipher::{KeyInit, BlockEncrypt};
    use aes::Aes128;
    use base64::{engine::general_purpose, Engine as _};

    let cipher = Aes128::new(GenericArray::from_slice(key));
    let mut data = plaintext.as_bytes().to_vec();
    
    // PKCS7 padding
    let block_size = 16;
    let padding_len = block_size - (data.len() % block_size);
    data.extend(std::iter::repeat(padding_len as u8).take(padding_len));

    for chunk in data.chunks_mut(16) {
        let block = GenericArray::from_mut_slice(chunk);
        cipher.encrypt_block(block);
    }

    general_purpose::STANDARD.encode(&data)
}

#[test]
fn test_aes_decrypt_with_dirty_16_byte_key() {
    // 1. Raw 16-byte key and corresponding encrypted ciphertext
    let raw_key = b"my_secret_key_16";
    let plaintext = "Hello World Stream Channel!";
    let encrypted_base64 = aes_encrypt(plaintext, raw_key);

    // 2. Wrap the key with whitespace, newlines, and various zero-width characters to simulate clipboard pollution
    let dirty_key = "\n \u{200b}my_secret_key_16 \n\u{200d}";

    // 3. Perform decryption using the dirty key. It MUST successfully sanitize, decode, and match the original plaintext.
    let decrypted = connector_sdk::crypto::aes_decrypt(&encrypted_base64, dirty_key);
    assert!(decrypted.is_ok(), "Expected successful decryption with sanitized 16-byte key, but got: {:?}", decrypted.err());
    assert_eq!(decrypted.unwrap(), plaintext);
}

#[test]
fn test_aes_decrypt_with_dirty_32_char_hex_key() {
    // 1. Raw 16-byte key, its 32-character hex representation, and corresponding ciphertext
    let raw_key = b"my_secret_key_16";
    let plaintext = "High Strength TDD Protection! 🚀";
    let encrypted_base64 = aes_encrypt(plaintext, raw_key);

    // 2. Wrap the 32-character hex key with whitespace and various zero-width characters (e.g. \u{200c}, \u{feff})
    // "6d795f7365637265745f6b65795f3136" is hex of b"my_secret_key_16"
    let dirty_hex_key = "  \u{200c}6d795f7365637265745f6b65795f3136 \n\u{feff}";

    // 3. Perform decryption using the dirty hex key. It MUST successfully sanitize, hex-decode to 16 bytes, decrypt, and match.
    let decrypted = connector_sdk::crypto::aes_decrypt(&encrypted_base64, dirty_hex_key);
    assert!(decrypted.is_ok(), "Expected successful decryption with sanitized 32-character hex key, but got: {:?}", decrypted.err());
    assert_eq!(decrypted.unwrap(), plaintext);
}




