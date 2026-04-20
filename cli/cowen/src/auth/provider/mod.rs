use crate::auth::models::Token;
use crate::core::config::Config;
use anyhow::Result;
use async_trait::async_trait;

pub mod self_built;
pub mod oauth2;

#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// 获取当前可用令牌。若过期则触发刷新或网络重整。
    async fn get_token(&self, profile: &str, config: &Config) -> Result<Token>;
    
    /// 强制执行网络刷新逻辑（忽略内存或本地缓存）。
    async fn refresh(&self, profile: &str, config: &Config) -> Result<Token>;
}
