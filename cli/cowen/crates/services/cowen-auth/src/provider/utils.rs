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
