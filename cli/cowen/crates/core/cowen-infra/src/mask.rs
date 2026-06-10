pub fn mask_string(val: &str) -> String {
    if val.is_empty() {
        return "********".to_string();
    }
    if val.len() <= 12 {
        return "********".to_string();
    }
    format!("{}...{}", &val[..8], &val[val.len() - 4..])
}

pub fn mask_sensitive_json(input: &str) -> String {
    let mut output = input.to_string();

    let keys = [
        "\"accessToken\"",
        "\"access_token\"",
        "\"orgAccessToken\"",
        "\"userAccessToken\"",
        "\"refreshToken\"",
        "\"refresh_token\"",
        "\"appSecret\"",
        "\"app_secret\"",
        "\"certificate\"",
        "\"appTicket\"",
        "\"app_ticket\"",
        "\"encryptKey\"",
        "\"encrypt_key\"",
        "\"permanentAuthCode\"",
        "\"userPermanentCode\"",
        "\"user_auth_permanent_code\"",
        "\"tempAuthCode\"",
    ];

    for key in keys {
        let lower_key = key.to_lowercase();
        let mut start_idx = 0;

        while let Some(idx) = output[start_idx..].to_lowercase().find(&lower_key) {
            let actual_idx = start_idx + idx;
            let search_start = actual_idx + key.len();

            let mut replaced = false;
            if let Some(colon_idx) = output[search_start..].find(':') {
                let val_start_search = search_start + colon_idx + 1;
                if let Some(quote1_idx) = output[val_start_search..].find('"') {
                    let val_start = val_start_search + quote1_idx + 1;
                    if let Some(quote2_idx) = output[val_start..].find('"') {
                        let val_end = val_start + quote2_idx;
                        let secret = &output[val_start..val_end];
                        let masked = mask_string(secret);

                        let mut new_output = String::with_capacity(output.len());
                        new_output.push_str(&output[..val_start]);
                        new_output.push_str(&masked);
                        new_output.push_str(&output[val_end..]);
                        output = new_output;

                        start_idx = val_start + masked.len() + 1;
                        replaced = true;
                    }
                }
            }
            if !replaced {
                start_idx = actual_idx + key.len();
            }
        }
    }
    output
}

pub fn mask_url_query(url: &str) -> String {
    let mut output = url.to_string();
    let keys = [
        "accessToken=",
        "access_token=",
        "orgAccessToken=",
        "userAccessToken=",
        "refreshToken=",
        "refresh_token=",
        "token=",
        "openToken=",
        "appSecret=",
        "app_secret=",
        "appTicket=",
        "app_ticket=",
        "encryptKey=",
        "encrypt_key=",
        "permanentAuthCode=",
        "userPermanentCode=",
        "tempAuthCode=",
    ];

    for key in keys {
        let qs = format!("?{}", key);
        let am = format!("&{}", key);
        let qs_lower = qs.to_lowercase();
        let am_lower = am.to_lowercase();

        for search_key_lower in &[qs_lower, am_lower] {
            let mut start_idx = 0;
            while let Some(idx) = output[start_idx..].to_lowercase().find(search_key_lower) {
                let actual_idx = start_idx + idx;
                let val_start = actual_idx + search_key_lower.len();
                let val_end = output[val_start..]
                    .find('&')
                    .map(|i| val_start + i)
                    .unwrap_or(output.len());
                let secret = &output[val_start..val_end];
                let masked = mask_string(secret);

                let mut new_output = String::new();
                new_output.push_str(&output[..val_start]);
                new_output.push_str(&masked);
                new_output.push_str(&output[val_end..]);
                output = new_output;

                start_idx = val_start + masked.len();
            }
        }
    }
    output
}

pub fn mask_tail(val: &str, show_len: usize) -> String {
    if val.len() <= show_len {
        return val.to_string();
    }
    let masked_len = val.len() - show_len;
    let mut result = "*".repeat(masked_len);
    result.push_str(&val[masked_len..]);
    result
}

pub fn mask_url(url: &str) -> String {
    if let Some(scheme_idx) = url.find("://") {
        let after_scheme = scheme_idx + 3;
        if let Some(at_idx) = url[after_scheme..].find('@') {
            let userinfo_end = after_scheme + at_idx;
            if !url[after_scheme..userinfo_end].contains('/') {
                let userinfo = &url[after_scheme..userinfo_end];
                let scheme_part = &url[..after_scheme];
                let rest = &url[userinfo_end + 1..];

                if let Some(colon_idx) = userinfo.find(':') {
                    return format!("{}{}:***@{}", scheme_part, &userinfo[..colon_idx], rest);
                } else {
                    return format!("{}***@{}", scheme_part, rest);
                }
            }
        }
    }
    url.to_string()
}
