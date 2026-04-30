pub(crate) fn get_user_token_key(app_key: &str, org_id: &str, user_id: &str) -> String {
    format!("oauth2_token_pair_user_{}_{}_{}", app_key, org_id, user_id)
}

pub(crate) fn get_org_token_key(app_key: &str, org_id: &str) -> String {
    format!("oauth2_token_pair_org_{}_{}", app_key, org_id)
}

pub(crate) fn get_user_upc_key(app_key: &str, org_id: &str, user_id: &str) -> String {
    format!("user_permanent_code_{}_{}_{}", app_key, org_id, user_id)
}

pub(crate) fn get_org_opc_key(app_key: &str, org_id: &str) -> String {
    format!("org_permanent_code_{}_{}", app_key, org_id)
}

pub(crate) fn get_custom_profile(base_profile: &str, app_key: &str, org_id: &str, user_id: Option<&str>) -> String {
    if let Some(uid) = user_id {
        format!("{}:{}:{}:{}", base_profile, app_key, org_id, uid)
    } else {
        format!("{}:{}:{}", base_profile, app_key, org_id)
    }
}
