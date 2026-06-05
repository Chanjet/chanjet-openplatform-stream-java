use anyhow::Result;

pub async fn tail(profile: &str, lines: usize) -> Result<()> {
    // Audit tail is basically log view for the audit domain with follow=true
    crate::cmd::log::view(profile, "audit", true, lines).await
}
