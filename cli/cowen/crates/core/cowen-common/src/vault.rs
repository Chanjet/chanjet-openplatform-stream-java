use crate::domain::*;
use crate::CowenResult;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait Vault:
    TicketDomain
    + TokenDomain
    + PermanentCodeDomain
    + SessionDomain
    + SecretDomain
    + ConfigDomain
    + AuditDomain
    + DlqDomain
    + ManagementDomain
    + Send
    + Sync
{
    fn primary_store(&self) -> Arc<dyn crate::store::Store>;

    async fn shutdown(&self) -> CowenResult<()> {
        self.primary_store().shutdown().await
    }

    async fn migrate(&self) -> CowenResult<()> {
        self.primary_store().migrate().await
    }
}
