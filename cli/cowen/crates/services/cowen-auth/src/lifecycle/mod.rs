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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::VaultTokenPool;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_auth_session_manager_lifecycle() {
        let temp_dir = tempfile::tempdir().unwrap();
        let app_cfg = cowen_common::config::AppConfig::default();
        let vault = cowen_store::create_vault(&app_cfg, temp_dir.path(), "test_fingerprint")
            .await
            .unwrap();

        let pool = Arc::new(VaultTokenPool::new(vault.clone()));
        let manager = AuthSessionManager::new(pool.as_ref());
        let profile = "test_profile_session";
        let state_val = "test_state_123";

        // 1. Initially getting non-existent session should fail
        let res = manager.get_session(state_val).await;
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("No pending auth session found"));

        // 2. Create a session
        let port = 8080;
        let session = manager.create_session(profile, port).await.unwrap();
        assert_eq!(session.profile, profile);
        assert_eq!(session.redirect_port, port);
        assert!(session.redirect_uri.contains("8080"));

        // 3. Get the session we just created
        let retrieved = manager.get_session(&session.state).await.unwrap();
        assert_eq!(retrieved.profile, profile);
        assert_eq!(retrieved.code_verifier, session.code_verifier);

        // 4. Save Captured code
        manager
            .save_code(profile, "captured_code_val", &session.state)
            .await
            .unwrap();

        // 5. Retrieve captured code
        let code = manager.get_captured_code(profile).await.unwrap();
        assert_eq!(code, "captured_code_val");

        // 6. Clear session
        manager.clear_session(&session.state).await.unwrap();

        // Getting deleted session should fail
        let res = manager.get_session(&session.state).await;
        assert!(res.is_err());

        // The captured code remains on the main profile; retrieve it
        let code = manager.get_captured_code(profile).await.unwrap();
        assert_eq!(code, "captured_code_val");

        // Manually delete the config to test the error path
        vault
            .delete_config(profile, "captured_auth_code")
            .await
            .unwrap();
        let res = manager.get_captured_code(profile).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_auth_session_expired() {
        let temp_dir = tempfile::tempdir().unwrap();
        let app_cfg = cowen_common::config::AppConfig::default();
        let vault = cowen_store::create_vault(&app_cfg, temp_dir.path(), "test_fingerprint")
            .await
            .unwrap();

        let pool = Arc::new(VaultTokenPool::new(vault.clone()));
        let manager = AuthSessionManager::new(pool.as_ref());

        // Save an expired session manually
        let expired_session = AuthSession {
            profile: "test_expired".to_string(),
            code_verifier: "verifier".to_string(),
            state: "state_exp".to_string(),
            redirect_uri: "http://127.0.0.1:8080/callback".to_string(),
            redirect_port: 8080,
            expires_at: Utc::now() - Duration::minutes(1),
        };
        vault.save_session(expired_session).await.unwrap();

        // Get expired session should fail with expired error
        let res = manager.get_session("state_exp").await;
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("Auth session expired"));
    }
}
