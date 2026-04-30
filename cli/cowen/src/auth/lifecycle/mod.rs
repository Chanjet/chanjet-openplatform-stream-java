use crate::auth::models::AuthSession;
use crate::auth::pool::TokenPool;
use crate::auth::provider::oauth2::Pkce;
use anyhow::{Result, anyhow, Context};
use chrono::{Utc, Duration};
use uuid::Uuid;

pub mod listener;
pub mod orchestrator;

pub struct AuthSessionManager<'a> {
    pool: &'a (dyn TokenPool + Send + Sync),
}

impl<'a> AuthSessionManager<'a> {
    pub fn new(pool: &'a (dyn TokenPool + Send + Sync)) -> Self {
        Self { pool }
    }

    pub async fn create_session(&self, profile: &str, redirect_port: u16) -> Result<AuthSession> {
        let pkce = Pkce::new();
        let state = Uuid::new_v4().to_string();
        
        let session = AuthSession {
            profile: profile.to_string(),
            code_verifier: pkce.verifier.clone(), // This is the secret for later
            state: state.clone(),
            redirect_uri: format!("http://127.0.0.1:{}/callback", redirect_port),
            redirect_port,
            expires_at: Utc::now() + Duration::minutes(5),
        };

        let session_json = serde_json::to_string(&session)?;
        self.pool.as_vault().set(profile, "pending_auth_session", &session_json).await?;
        
        Ok(session)
    }

    pub async fn get_session(&self, profile: &str) -> Result<AuthSession> {
        let session_json = self.pool.as_vault().get(profile, "pending_auth_session").await
            .context("No pending auth session found")?;
        let session: AuthSession = serde_json::from_str(&session_json)?;
        
        if Utc::now() > session.expires_at {
            let _ = self.pool.as_vault().delete(profile, "pending_auth_session").await;
            return Err(anyhow!("Auth session expired"));
        }
        
        Ok(session)
    }

    pub async fn save_code(&self, profile: &str, code: &str, state: &str) -> Result<()> {
        let session = self.get_session(profile).await?;
        if session.state != state {
            return Err(anyhow!("State mismatch"));
        }

        self.pool.as_vault().set(profile, "captured_auth_code", code).await?;
        Ok(())
    }

    pub async fn get_captured_code(&self, profile: &str) -> Result<String> {
        self.pool.as_vault().get(profile, "captured_auth_code").await
            .context("No captured auth code found")
    }

    pub async fn clear(&self, profile: &str) -> Result<()> {
        let _ = self.pool.as_vault().delete(profile, "pending_auth_session").await;
        let _ = self.pool.as_vault().delete(profile, "captured_auth_code").await;
        Ok(())
    }
}


