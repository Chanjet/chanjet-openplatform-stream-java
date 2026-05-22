use crate::pool::TokenPool;
use crate::provider::oauth2::Pkce;
use chrono::{Duration, Utc};
use cowen_common::models::AuthSession;
use cowen_common::{CowenError, CowenResult};
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

    pub async fn create_session(
        &self,
        profile: &str,
        redirect_port: u16,
    ) -> CowenResult<AuthSession> {
        let pkce = Pkce::new();
        let state = Uuid::new_v4().to_string();

        let session = AuthSession {
            profile: profile.to_string(),
            code_verifier: pkce.verifier.clone(),
            state: state.clone(),
            redirect_uri: format!("http://127.0.0.1:{}/callback", redirect_port),
            redirect_port,
            expires_at: Utc::now() + Duration::minutes(5),
        };

        self.pool.as_vault().save_session(session.clone()).await?;
        Ok(session)
    }

    pub async fn get_session(&self, state: &str) -> CowenResult<AuthSession> {
        let session = self
            .pool
            .as_vault()
            .get_session(state)
            .await
            .map_err(|_| CowenError::Auth("No pending auth session found".to_string()))?;

        if Utc::now() > session.expires_at {
            let _ = self.pool.as_vault().delete_session(state).await;
            return Err(CowenError::Auth("Auth session expired".to_string()));
        }

        Ok(session)
    }

    pub async fn save_code(&self, _profile: &str, code: &str, state: &str) -> CowenResult<()> {
        let session = self.get_session(state).await?;
        // Store the code in the original profile's config so it can be retrieved by the orchestrator/provider
        self.pool
            .as_vault()
            .set_config(&session.profile, "captured_auth_code", code)
            .await?;
        Ok(())
    }

    pub async fn get_captured_code(&self, profile: &str) -> CowenResult<String> {
        self.pool
            .as_vault()
            .get_config(profile, "captured_auth_code")
            .await
            .map_err(|_| CowenError::Auth("No captured auth code found".to_string()))
    }

    pub async fn clear(&self, _profile: &str) -> CowenResult<()> {
        Ok(())
    }

    pub async fn clear_session(&self, state: &str) -> CowenResult<()> {
        let _ = self.pool.as_vault().delete_session(state).await;
        let profile = format!("session:{}", state);
        let _ = self
            .pool
            .as_vault()
            .delete_config(&profile, "captured_auth_code")
            .await;
        Ok(())
    }
}
