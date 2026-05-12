use async_trait::async_trait;
use std::sync::Arc;
use crate::domain::*;

#[async_trait]
pub trait Vault: 
    TicketDomain + TokenDomain + PermanentCodeDomain + SessionDomain + 
    SecretDomain + ConfigDomain + AuditDomain + DlqDomain + ManagementDomain + 
    Send + Sync 
{
    fn primary_store(&self) -> Arc<dyn crate::store::Store>;
}
