use anyhow::Result;
use std::sync::Arc;
use cowen_common::vault::Vault;

pub async fn tail(profile: &str, lines: usize, vault: Arc<dyn Vault>) -> Result<()> {
    // Audit tail is basically log view for the audit domain with follow=true
    crate::cmd::log::view(profile, "audit", true, lines, vault).await
}
