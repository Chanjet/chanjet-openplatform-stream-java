use cowen_common::status::StatusLevel;

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
        let key_len = if decrypt_key.len() == 32 {
            if decrypt_key.len().is_multiple_of(2)
                && decrypt_key.chars().all(|c| c.is_ascii_hexdigit())
            {
                16
            } else {
                32
            }
        } else {
            decrypt_key.len()
        };

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
