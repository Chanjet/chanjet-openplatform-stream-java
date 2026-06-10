use cowen_common::security;
use cowen_common::utils;
use cowen_infra as network;
use std::net::SocketAddr;

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
}

#[test]
fn test_mask_url_query() {
    let url = "https://api.example.com?token=secret_1234567890&other=public";
    let masked = utils::mask_url_query(url);
    assert!(masked.contains("token=secret_1...7890"));
    assert!(masked.contains("other=public"));
}

#[test]
fn test_machine_fingerprint() {
    let f1 = security::get_machine_fingerprint().unwrap();
    let f2 = security::get_machine_fingerprint().unwrap();
    assert_eq!(f1, f2);
    assert!(!f1.is_empty());
}

#[test]
fn test_obfs_macro() {
    let o = cowen_infra::obfs!("hello-world");
    assert_eq!(o, "hello-world");
    // Verify that the actual code in the binary would be obfuscated is hard here,
    // but we can verify the macro works and produces the correct string.
}

#[test]
fn test_crypto_roundtrip() {
    let data = b"secret-message";
    let key = [0u8; 32];

    let enc = security::encrypt(data, &key).unwrap();
    assert_ne!(data.to_vec(), enc);

    let dec = security::decrypt(&enc, &key).unwrap();
    assert_eq!(data.to_vec(), dec);
}

#[test]
fn test_derive_key() {
    let k1 = security::derive_key("fingerprint-1");
    let k2 = security::derive_key("fingerprint-1");
    let k3 = security::derive_key("fingerprint-2");

    assert_eq!(k1, k2);
    assert_ne!(k1, k3);
}

#[test]
fn test_user_agent_format() {
    let ua = cowen_infra::get_user_agent("1.0.0");

    // 验证格式: Cowen/x.y.z (os; arch)
    assert!(ua.starts_with("Cowen/"));
    assert!(ua.contains("("));
    assert!(ua.contains(";"));
    assert!(ua.ends_with(")"));

    // 验证包含版本号 (至少包含一个数字)
    assert!(ua.chars().any(|c: char| c.is_numeric()));
}

#[test]
fn test_validate_loopback_addr() {
    // 1. Success cases
    let addr1: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let addr2: SocketAddr = "[::1]:8080".parse().unwrap();

    assert!(network::validate_loopback_addr(&addr1).is_ok());
    assert!(network::validate_loopback_addr(&addr2).is_ok());

    // 2. Failure cases
    let addr3: SocketAddr = "192.168.1.1:8080".parse().unwrap();
    let addr4: SocketAddr = "0.0.0.0:8080".parse().unwrap();

    let result1 = network::validate_loopback_addr(&addr3);
    let result2 = network::validate_loopback_addr(&addr4);

    assert!(result1.is_err());
    assert!(result2.is_err());
}
