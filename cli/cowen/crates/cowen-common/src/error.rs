use thiserror::Error;

#[derive(Error, Debug)]
pub enum CowenError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Storage error: {0}")]
    Store(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API error: {0}")]
    Api(String),

    #[error("Security/Crypto error: {0}")]
    Security(String),

    #[error("Profile not found: {0}")]
    ProfileNotFound(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type CowenResult<T> = std::result::Result<T, CowenError>;

impl From<serde_json::Error> for CowenError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl From<serde_yaml::Error> for CowenError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl From<base64::DecodeError> for CowenError {
    fn from(err: base64::DecodeError) -> Self {
        Self::Security(err.to_string())
    }
}

impl From<reqwest::header::InvalidHeaderValue> for CowenError {
    fn from(err: reqwest::header::InvalidHeaderValue) -> Self {
        Self::Config(format!("Invalid header value: {}", err))
    }
}

#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql", feature = "mssql"))]
impl From<sqlx::Error> for CowenError {
    fn from(err: sqlx::Error) -> Self {
        Self::Store(err.to_string())
    }
}

#[cfg(feature = "redis")]
impl From<redis::RedisError> for CowenError {
    fn from(err: redis::RedisError) -> Self {
        Self::Store(err.to_string())
    }
}

impl CowenError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn auth(msg: impl Into<String>) -> Self {
        Self::Auth(msg.into())
    }

    pub fn store(msg: impl Into<String>) -> Self {
        Self::Store(msg.into())
    }

    pub fn api(msg: impl Into<String>) -> Self {
        Self::Api(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}
