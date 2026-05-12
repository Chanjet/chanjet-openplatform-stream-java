use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StoreAppTokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
    #[serde(alias = "refresh_token_expires_in")]
    pub refresh_expires_in: Option<i64>,
    #[serde(alias = "user_auth_permanent_code")]
    pub user_permanent_code: Option<String>,
    #[serde(alias = "permanent_auth_code")]
    pub org_permanent_code: Option<String>,
}
