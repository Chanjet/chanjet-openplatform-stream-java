use crate::core::config::Config;
use crate::core::utils::mask_sensitive_json;

#[test]
fn test_config_debug_masking() {
    let mut cfg = Config::default_with_profile("test");
    // Use non-suspicious strings for tests to avoid Gitleaks detection
    cfg.app_secret = "val_test_val_8877".to_string();
    cfg.certificate = "val_test_val_6655".to_string();
    cfg.encrypt_key = "val_test_val_4433".to_string();

    let debug_str = format!("{:?}", cfg);
    
    // Should NOT contain the plain secrets
    assert!(!debug_str.contains("val_test_val_8877"));
    assert!(!debug_str.contains("val_test_val_6655"));
    assert!(!debug_str.contains("val_test_val_4433"));
    
    // Should contain masked versions
    assert!(debug_str.contains("val_8888...7777"));
    assert!(debug_str.contains("val_6666...5555"));
    assert!(debug_str.contains("val_4444...3333"));
}

#[test]
fn test_json_masking_comprehensive() {
    let raw_json = r#"{
        "accessToken": "mock_at_test_val_8877",
        "appSecret": "mock_as_test_val_8877",
        "appTicket": "mock_ti_test_val_8877",
        "certificate": "mock_ce_test_val_8877",
        "other": "safe_data"
    }"#;

    let masked = mask_sensitive_json(raw_json);
    
    assert!(!masked.contains("mock_at_test_val_8877"));
    assert!(!masked.contains("mock_as_test_val_8877"));
    assert!(!masked.contains("mock_ti_test_val_8877"));
    assert!(!masked.contains("mock_ce_test_val_8877"));
    
    assert!(masked.contains("mock_at_...7777"));
    assert!(masked.contains("mock_as_...7777"));
    assert!(masked.contains("mock_ti_...7777"));
    assert!(masked.contains("mock_ce_...7777"));
    assert!(masked.contains("safe_data"));
}
