use crate::auth::models::{Ticket, Token, AuthSession};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait TicketDomain: Send + Sync {
    /// 获取应用的 AppTicket
    async fn get_app_ticket(&self, app_key: &str) -> Result<Ticket>;
    /// 保存应用的 AppTicket
    async fn save_app_ticket(&self, app_key: &str, ticket: Ticket) -> Result<()>;
    /// 删除应用的 AppTicket (通常由于平台判定过期)
    async fn delete_app_ticket(&self, app_key: &str) -> Result<()>;
}

#[async_trait]
pub trait TokenDomain: Send + Sync {
    /// 获取 Profile 级别的 AccessToken
    async fn get_access_token(&self, profile: &str) -> Result<Token>;
    /// 保存 Profile 级别的 AccessToken
    async fn save_access_token(&self, profile: &str, token: Token) -> Result<()>;
    /// 删除 Profile 级别的 AccessToken
    async fn delete_access_token(&self, profile: &str) -> Result<()>;

    /// 获取应用级别的 AccessToken (AppAccessToken)
    async fn get_app_access_token(&self, app_key: &str) -> Result<Token>;
    /// 保存应用级别的 AccessToken
    async fn save_app_access_token(&self, app_key: &str, token: Token) -> Result<()>;
}

#[async_trait]
pub trait PermanentCodeDomain: Send + Sync {
    /// 获取组织级永久授权码 (OPC)
    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> Result<String>;
    /// 保存组织级永久授权码
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> Result<()>;

    /// 获取用户级永久授权码 (UPC)
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> Result<String>;
    /// 保存用户级永久授权码
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> Result<()>;
}

#[async_trait]
pub trait SessionDomain: Send + Sync {
    /// 获取交互式认证会话
    async fn get_session(&self, state: &str) -> Result<AuthSession>;
    /// 保存交互式认证会话
    async fn save_session(&self, session: AuthSession) -> Result<()>;
    /// 删除交互式认证会话
    async fn delete_session(&self, state: &str) -> Result<()>;
}

#[async_trait]
pub trait SecretDomain: Send + Sync {
    /// 获取敏感凭据 (如 appSecret, certificate)
    async fn get_secret(&self, profile: &str, key: &str) -> Result<String>;
    /// 保存敏感凭据
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()>;
    /// 删除敏感凭据
    async fn delete_secret(&self, profile: &str, key: &str) -> Result<()>;
}

#[async_trait]
pub trait ConfigDomain: Send + Sync {
    /// 获取非敏感配置
    async fn get_config(&self, profile: &str, key: &str) -> Result<String>;
    /// 获取配置详情（含版本、时间戳）
    async fn get_config_full(&self, profile: &str, key: &str) -> Result<crate::core::store::Item>;
    /// 设置配置
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()>;
    /// 条件更新配置（CAS）
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> Result<()>;
    /// 列出 Profile 下所有配置键
    async fn list_configs(&self, profile: &str) -> Result<Vec<String>>;
    /// 删除配置
    async fn delete_config(&self, profile: &str, key: &str) -> Result<()>;
}

#[async_trait]
pub trait AuditDomain: Send + Sync {
    /// 保存审计日志
    async fn save_audit(&self, entry: &crate::core::store::AuditEntry) -> Result<()>;
    /// 列出审计日志
    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<crate::core::store::AuditEntry>>;
}

#[async_trait]
pub trait DlqDomain: Send + Sync {
    /// 压入死信队列
    async fn push_dlq(&self, msg: &crate::core::store::DlqMessage) -> Result<()>;
    /// 弹出死信消息
    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<crate::core::store::DlqMessage>>;
    /// 列出死信消息
    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<crate::core::store::DlqMessage>>;
    /// 获取 Profile 下所有死信消息
    async fn list_all_dlq(&self, profile: &str) -> Result<Vec<crate::core::store::DlqMessage>>;
}

#[async_trait]
pub trait ManagementDomain: Send + Sync {
    /// 清理 Profile 所有数据
    async fn clear_profile(&self, profile: &str) -> Result<()>;
    /// 重命名 Profile
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<()>;
    /// 列出所有 Profile
    async fn list_all_profiles(&self) -> Result<Vec<String>>;
}
