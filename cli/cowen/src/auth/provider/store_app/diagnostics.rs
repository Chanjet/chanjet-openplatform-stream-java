use crate::auth::models::Token;
use crate::core::config::Config;
use anyhow::Result;
use chrono::{Utc, Local};
use crate::auth::pool::TokenPool;

pub(crate) async fn get_status_entries(
    pool: &(dyn TokenPool + Send + Sync),
    profile: &str,
    config: &Config
) -> Result<Vec<crate::core::status::StatusEntry>> {
    use crate::core::status::{StatusEntry, StatusLevel};
    let mut entries = Vec::new();
    let vault = pool.as_vault();

    // 1. Security Check (Sidecar Mode)
    let mut missing = Vec::new();
    if vault.get(profile, "app_secret").await.is_err() { missing.push("app_secret".to_string()); }
    if vault.get(profile, "encrypt_key").await.is_err() { missing.push("encrypt_key".to_string()); }

    let (sec_level, sec_msg) = if missing.is_empty() {
        (StatusLevel::OK, "Sidecar credentials are securely stored.".to_string())
    } else {
        (StatusLevel::WARN, format!("Missing: {}", missing.join(", ")))
    };

    entries.push(StatusEntry {
        name: "Security (Sidecar)".to_string(),
        icon: "🛡️".to_string(),
        level: sec_level,
        message: sec_msg,
        reason: if sec_level == StatusLevel::WARN { Some("缺少必要凭据，可能导致解密失败或令牌刷新失败。".to_string()) } else { None },
        details: vec![],
        children: vec![],
    });

    // 2. Token Check (SuiteAccessToken and Org/User Tokens)
    let refresh_error = vault.get(profile, "last_refresh_error").await.ok();
    let ref_revoked = vault.get(profile, "oauth2_revoked").await.is_ok();

    // 2.1 Org/User Tokens
    if let Ok(pair_raw) = vault.get(profile, "oauth2_token_pair").await {
        let pair: crate::auth::models::OAuth2TokenPair = serde_json::from_str(&pair_raw)?;
        let is_expired = Utc::now() > pair.expires_at;
        let ref_expired = Utc::now() > pair.refresh_expires_at;

        let children = vec![
            StatusEntry {
                name: "AccessToken".to_string(),
                icon: "🔑".to_string(),
                level: if is_expired || ref_revoked { StatusLevel::ERROR } else { StatusLevel::OK },
                message: format!("[{}] (Expires: {})", 
                    if is_expired || ref_revoked { "EXPIRED" } else { "VALID" },
                    pair.expires_at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S")),
                reason: if ref_revoked {
                    Some("关联的 RefreshToken 已失效，AccessToken 无法继续自动续约。".to_string())
                } else if is_expired { 
                    refresh_error.map(|e| format!("自动续约失败: {}", e))
                        .or(Some("AccessToken 已过期，正在等待后台续约进程处理...".to_string()))
                } else { None },
                details: vec![],
                children: vec![],
            },
            StatusEntry {
                name: "RefreshToken".to_string(),
                icon: "🔄".to_string(),
                level: if ref_expired || ref_revoked { StatusLevel::ERROR } else { StatusLevel::OK },
                message: format!("[{}] (Expires: {})", 
                    if ref_revoked { "REVOKED" } else if ref_expired { "EXPIRED" } else { "VALID" },
                    pair.refresh_expires_at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S")),
                reason: if ref_revoked {
                    Some("令牌已于服务端吊销或失效，必须重新执行 `cowen auth login`。".to_string())
                } else if ref_expired { 
                    Some("RefreshToken 已失效，必须重新运行 'cowen auth login' 或 'init'。".to_string()) 
                } else { None },
                details: vec![],
                children: vec![],
            }
        ];

        let mut details = vec![];
        let token_inner = Token {
            value: pair.access_token.clone(),
            expires_at: pair.expires_at,
            created_at: pair.created_at,
        };
        
        if let Some(identity) = token_inner.extract_identity() {
            details.push(format!("User ID: {}", identity.user_id));
            details.push(format!("Org ID:  {}", identity.org_id));
            details.push(format!("App ID:  {}", identity.app_id));
        }

        entries.push(StatusEntry {
            name: "Authentication (Org)".to_string(),
            icon: "🔐".to_string(),
            level: if ref_revoked { StatusLevel::ERROR } else if is_expired { StatusLevel::WARN } else { StatusLevel::OK },
            message: "OAuth2 tokens are locally managed via sidecar.".to_string(),
            reason: if ref_revoked { Some("会话已失效 (Revoked)".to_string()) } else { None },
            details,
            children,
        });
    }

    // 2.2 SuiteAccessToken Status
    if let Ok(sat) = pool.get_app_access_token(config.app_key.trim()).await {
            let is_expired = sat.is_expired();
            entries.push(StatusEntry {
            name: "SuiteAccessToken".to_string(),
            icon: "🎫".to_string(),
            level: if is_expired { StatusLevel::ERROR } else { StatusLevel::OK },
            message: format!("[{}] (Expires: {})", 
                if is_expired { "EXPIRED" } else { "VALID" },
                sat.expires_at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S")),
            reason: if is_expired { Some("SuiteAccessToken 已过期，正在尝试通过 AppTicket 续约。".to_string()) } else { None },
            details: vec![],
            children: vec![],
        });
    }

    // 2.3 AppTicket Status
    if let Ok(ts_str) = vault.get(profile, "app_ticket_created").await {
        let created_at = chrono::DateTime::parse_from_rfc3339(&ts_str).map(|dt| dt.with_timezone(&Utc)).unwrap_or(Utc::now());
        entries.push(StatusEntry {
            name: "AppTicket".to_string(),
            icon: "🎫".to_string(),
            level: StatusLevel::OK,
            message: format!("[CACHED] (Received: {})", created_at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S")),
            reason: None,
            details: vec![],
            children: vec![],
        });
    } else {
        entries.push(StatusEntry {
            name: "AppTicket".to_string(),
            icon: "🎫".to_string(),
            level: StatusLevel::NONE,
            message: "[NONE] (等待 Daemon 接收推送)".to_string(),
            reason: None,
            details: vec![],
            children: vec![],
        });
    }

    Ok(entries)
}
