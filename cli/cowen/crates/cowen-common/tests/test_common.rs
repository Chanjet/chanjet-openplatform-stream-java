use cowen_common::utils;
use cowen_common::security;
use cowen_common::network;
use std::net::{SocketAddr, IpAddr};

#[test]
fn test_mask_string() {
    assert_eq!(utils::mask_string(""), "********");
    assert_eq!(utils::mask_string("short"), "********");
    assert_eq!(utils::mask_string("123456789012"), "********");
    assert_eq!(utils::mask_string("1234567890123"), "12345678...0123");
    assert_eq!(utils::mask_string("my-long-test-string"), "my-long-...ring");
}

#[test]
fn test_mask_sensitive_json() {
    let json = r#"{"accessToken": "mock_at_1234567890", "other": "public"}"#;
    let masked = utils::mask_sensitive_json(json);
    assert!(masked.contains(r#""accessToken": "mock_at_...7890""#));
    assert!(masked.contains(r#""other": "public""#));

    let json2 = r#"{"app_secret": "too-short"}"#;
    let masked2 = utils::mask_sensitive_json(json2);
    assert!(masked2.contains(r#""app_secret": "********""#));
}

#[test]
fn test_mask_url_query() {
    let url = "https://example.com/api?accessToken=secret1234567890&other=public";
    let masked = utils::mask_url_query(url);
    assert_eq!(masked, "https://example.com/api?accessToken=secret12...7890&other=public");

    let url2 = "https://example.com/api?token=short";
    let masked2 = utils::mask_url_query(url2);
    assert_eq!(masked2, "https://example.com/api?token=********");
}

#[test]
fn test_mask_tail() {
    assert_eq!(utils::mask_tail("12345", 2), "***45");
    assert_eq!(utils::mask_tail("12345", 5), "12345");
    assert_eq!(utils::mask_tail("12345", 6), "12345");
    assert_eq!(utils::mask_tail("abc", 0), "***");
}

#[test]
fn test_derive_key() {
    let key1 = security::derive_key("fingerprint1");
    let key2 = security::derive_key("fingerprint1");
    let key3 = security::derive_key("fingerprint2");

    assert_eq!(key1, key2);
    assert_ne!(key1, key3);
    assert_eq!(key1.len(), 32);
}

#[test]
fn test_encrypt_decrypt() {
    let key = security::derive_key("test_fingerprint");
    let data = b"hello world secret data";
    
    let encrypted = security::encrypt(data, &key).expect("Encryption failed");
    assert!(encrypted.len() > 12);
    
    let decrypted = security::decrypt(&encrypted, &key).expect("Decryption failed");
    assert_eq!(decrypted, data);
}

#[test]
fn test_decrypt_invalid_data() {
    let key = security::derive_key("test_fingerprint");
    let result = security::decrypt(b"short", &key);
    assert!(result.is_err());
    
    let invalid_data = vec![0u8; 20];
    let result2 = security::decrypt(&invalid_data, &key);
    assert!(result2.is_err());
}

#[test]
fn test_user_agent_format() {
    let ua = network::get_user_agent();
    
    // 验证格式: Cowen/x.y.z (os; arch)
    assert!(ua.starts_with("Cowen/"));
    assert!(ua.contains("("));
    assert!(ua.contains(";"));
    assert!(ua.ends_with(")"));
    
    // 验证包含版本号 (至少包含一个数字)
    assert!(ua.chars().any(|c| c.is_numeric()));
}

#[test]
fn test_validate_loopback_addr() {
    // 1. Success cases
    let localhost_v4 = SocketAddr::new(IpAddr::V4("127.0.0.1".parse().unwrap()), 8080);
    assert!(network::validate_loopback_addr(&localhost_v4).is_ok());

    let localhost_v6 = SocketAddr::new(IpAddr::V6("::1".parse().unwrap()), 8080);
    assert!(network::validate_loopback_addr(&localhost_v6).is_ok());

    // 2. Failure cases
    let any_v4 = SocketAddr::new(IpAddr::V4("0.0.0.0".parse().unwrap()), 8080);
    let err = network::validate_loopback_addr(&any_v4).unwrap_err();
    assert!(err.to_string().contains("0.0.0.0"));

    let lan_ip = SocketAddr::new(IpAddr::V4("192.168.1.1".parse().unwrap()), 8080);
    assert!(network::validate_loopback_addr(&lan_ip).is_err());
}
