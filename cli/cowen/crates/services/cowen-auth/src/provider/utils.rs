use cowen_common::status::StatusLevel;

fn get_decrypt_key_len(decrypt_key: &str) -> usize {
    if decrypt_key.len() == 32 && decrypt_key.chars().all(|c| c.is_ascii_hexdigit()) {
        16
    } else {
        decrypt_key.len()
    }
}

pub fn check_decryption_key_format(
    encrypt_key_val: &str,
    app_secret_val: &str,
) -> (StatusLevel, String) {
    let decrypt_key_raw = if !encrypt_key_val.is_empty() {
        encrypt_key_val
    } else {
        app_secret_val
    };
    let decrypt_key = cowen_common::utils::sanitize_credential(decrypt_key_raw);

    if decrypt_key.is_empty() {
        (
            StatusLevel::ERROR,
            "Decryption key is missing (both encrypt_key and app_secret are empty)".to_string(),
        )
    } else {
        let key_len = get_decrypt_key_len(&decrypt_key);

        if key_len != 16 {
            (
                StatusLevel::ERROR,
                format!(
                    "Decryption key trimmed length {} is invalid. Must be 16 bytes or 32-character hex",
                    decrypt_key.len()
                ),
            )
        } else {
            (
                StatusLevel::OK,
                "Decryption key format is valid (16 bytes or 32-character hex)".to_string(),
            )
        }
    }
}

pub fn validate_decrypt_key_config(
    config: &cowen_common::config::Config,
) -> cowen_common::CowenResult<()> {
    let decrypt_key_raw = if !config.encrypt_key.is_empty() {
        &config.encrypt_key
    } else {
        &config.app_secret
    };
    let decrypt_key = cowen_common::utils::sanitize_credential(decrypt_key_raw);

    if decrypt_key.is_empty() {
        return Err(cowen_common::CowenError::Config(
            "Decryption key (encrypt_key or fallback app_secret) is required and cannot be empty for SelfBuilt or StoreApp modes".to_string(),
        ));
    }

    let key_len = get_decrypt_key_len(&decrypt_key);

    if key_len != 16 {
        return Err(cowen_common::CowenError::Config(format!(
            "Decryption key (encrypt_key or fallback app_secret) must be exactly 16 bytes (or 32-character hex) for SelfBuilt or StoreApp modes, got {} bytes",
            decrypt_key.len()
        )));
    }
    Ok(())
}

pub async fn check_decrypt_key_credentials(
    vault: &dyn cowen_common::vault::Vault,
    profile: &str,
) -> Result<cowen_doctor::DiagnosticStatus, String> {
    let app_secret = vault
        .get_secret(profile, "app_secret")
        .await
        .unwrap_or_default();
    let encrypt_key = vault
        .get_secret(profile, "encrypt_key")
        .await
        .unwrap_or_default();

    let decrypt_key_raw = if !encrypt_key.is_empty() {
        &encrypt_key
    } else {
        &app_secret
    };
    let decrypt_key = decrypt_key_raw.trim();

    if decrypt_key.is_empty() {
        Ok(cowen_doctor::DiagnosticStatus::Error(
            "缺少解密密钥 (App Secret 或 Encrypt Key 均为空)".to_string(),
        ))
    } else {
        let key_len = get_decrypt_key_len(decrypt_key);

        if key_len != 16 {
            Ok(cowen_doctor::DiagnosticStatus::Error(format!(
                "解密密钥不合规：必须为16字节或32字符Hex，当前 trimmed 长度为 {}",
                decrypt_key.len()
            )))
        } else {
            Ok(cowen_doctor::DiagnosticStatus::Ok)
        }
    }
}

use cowen_common::status::{CommonTemplate, StatusEntry};

pub fn wrap_auth_entries(auth_entries: Vec<StatusEntry>) -> Option<StatusEntry> {
    if auth_entries.is_empty() {
        return None;
    }

    let max_level = auth_entries
        .iter()
        .map(|e| e.level)
        .max_by_key(|l| match l {
            StatusLevel::ERROR => 3,
            StatusLevel::WARN => 2,
            StatusLevel::OK => 1,
            _ => 0,
        })
        .unwrap_or(StatusLevel::OK);

    Some(
        StatusEntry::new(
            CommonTemplate::ProviderSummary("Authentication Status".to_string(), "🔐".to_string()),
            max_level,
            format!("Collected {} status indicators", auth_entries.len()),
        )
        .with_children(auth_entries),
    )
}

use cowen_common::{CowenError, CowenResult};

pub async fn send_token_form_request(
    http_sender: &dyn crate::client::HttpSender,
    url: &str,
    body: serde_json::Value,
    app_key: &str,
) -> CowenResult<crate::client::SimpleResponse> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "appKey",
        app_key
            .parse()
            .unwrap_or(reqwest::header::HeaderValue::from_static("")),
    );
    http_sender.post_form(url, headers, body).await
}

pub async fn handle_common_token_errors(
    vault: &dyn cowen_common::vault::Vault,
    profile: &str,
    err_text: &str,
    status: reqwest::StatusCode,
) -> Option<CowenResult<cowen_common::models::Token>> {
    if err_text.contains("4007") || err_text.contains("invalid_grant") {
        let _ = vault.set_config(profile, "oauth2_revoked", "true").await;
        return Some(Err(CowenError::Auth(format!(
            "令牌已失效（可能已被吊销），请执行 `owenc auth login` 重新授权。 (Error: {})",
            status
        ))));
    }
    if err_text.contains("4006") {
        return Some(Err(CowenError::Auth(format!(
            "ClientID 与令牌颁发者不一致，请检查配置。 (Error: {})",
            status
        ))));
    }
    if err_text.contains("4001") {
        return Some(Err(CowenError::Auth(format!(
            "授权校验失败 (PKCE)，请重新执行 `owenc init`。 (Error: {})",
            status
        ))));
    }
    None
}

pub fn decorate_proxy_headers(
    headers: &mut reqwest::header::HeaderMap,
    spec: &serde_json::Value,
    path: &str,
    method: &str,
    app_key: &str,
    app_secret: &str,
    token_value: &str,
) {
    let auth_headers = crate::RequestDecorator::get_auth_headers(
        spec,
        path,
        method,
        app_key,
        app_secret,
        token_value,
    );

    for (name, value) in auth_headers {
        if let Ok(name) = reqwest::header::HeaderName::from_bytes(name.as_bytes()) {
            if let Ok(val) = reqwest::header::HeaderValue::from_bytes(value.as_bytes()) {
                headers.insert(name, val);
            }
        }
    }
}

pub async fn push_daemon_diagnostic(
    results: &mut Vec<cowen_common::status::StatusEntry>,
    ctx: &cowen_common::status::StatusContext<'_>,
    display_name: &str,
    efficiency_tip: &str,
    supports_webhooks: bool,
) -> CowenResult<()> {
    let daemon_info = cowen_common::status::get_active_daemon_info(&ctx.profile);
    results.push(
        cowen_common::status::collect_daemon_status(
            ctx,
            display_name,
            efficiency_tip,
            supports_webhooks,
            daemon_info,
        )
        .await?,
    );
    Ok(())
}

pub async fn perform_logout_cleanup(
    vault: &dyn cowen_common::vault::Vault,
    profile: &str,
    app_key: &str,
) -> CowenResult<()> {
    let _ = vault.delete_access_token(profile).await;
    let _ = vault.delete_refresh_token(profile).await;

    let app_key = app_key.trim();
    if !app_key.is_empty() {
        let _ = vault.delete_app_access_token(app_key).await;
        let _ = vault.delete_app_ticket(app_key).await;
    }

    // Cleanup legacy keys if any
    let _ = vault.delete_config(profile, "oauth2_token_pair").await;
    let _ = vault.delete_secret(profile, "oauth2_token_pair").await;

    let _ = vault.delete_config(profile, "oauth2_revoked").await;
    let _ = vault.delete_config(profile, "last_refresh_error").await;
    Ok(())
}

pub fn insert_openapi_headers(
    headers: &mut reqwest::header::HeaderMap,
    token_value: &str,
    app_key: &str,
) {
    headers.insert(
        "openToken",
        token_value
            .parse()
            .unwrap_or(reqwest::header::HeaderValue::from_static("")),
    );
    headers.insert(
        "appKey",
        app_key
            .parse()
            .unwrap_or(reqwest::header::HeaderValue::from_static("")),
    );
}
