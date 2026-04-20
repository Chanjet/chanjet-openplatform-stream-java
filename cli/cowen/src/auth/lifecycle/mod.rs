use crate::auth::models::AuthSession;
use crate::auth::pool::TokenPool;
use crate::auth::provider::oauth2::Pkce;
use anyhow::{Result, anyhow, Context};
use chrono::{Utc, Duration};
use uuid::Uuid;

pub mod listener;

pub struct AuthSessionManager<'a> {
    pool: &'a (dyn TokenPool + Send + Sync),
}

impl<'a> AuthSessionManager<'a> {
    pub fn new(pool: &'a (dyn TokenPool + Send + Sync)) -> Self {
        Self { pool }
    }

    pub fn create_session(&self, profile: &str, redirect_port: u16) -> Result<AuthSession> {
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
        self.pool.as_vault().set(profile, "pending_auth_session", &session_json)?;
        
        Ok(session)
    }

    pub fn get_session(&self, profile: &str) -> Result<AuthSession> {
        let session_json = self.pool.as_vault().get(profile, "pending_auth_session")
            .context("No pending auth session found")?;
        let session: AuthSession = serde_json::from_str(&session_json)?;
        
        if Utc::now() > session.expires_at {
            let _ = self.pool.as_vault().delete(profile, "pending_auth_session");
            return Err(anyhow!("Auth session expired"));
        }
        
        Ok(session)
    }

    pub fn save_code(&self, profile: &str, code: &str, state: &str) -> Result<()> {
        let session = self.get_session(profile)?;
        if session.state != state {
            return Err(anyhow!("State mismatch"));
        }

        self.pool.as_vault().set(profile, "captured_auth_code", code)?;
        Ok(())
    }

    pub fn get_captured_code(&self, profile: &str) -> Result<String> {
        self.pool.as_vault().get(profile, "captured_auth_code")
            .context("No captured auth code found")
    }

    pub fn clear(&self, profile: &str) -> Result<()> {
        let _ = self.pool.as_vault().delete(profile, "pending_auth_session");
        let _ = self.pool.as_vault().delete(profile, "captured_auth_code");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::collections::HashMap;
    use crate::auth::models::{Token, Ticket};
    use crate::core::vault::Vault;

    struct MockVault {
        data: Mutex<HashMap<String, String>>,
    }

    impl Vault for MockVault {
        fn get(&self, _profile: &str, key: &str) -> Result<String> {
            self.data.lock().unwrap().get(key).cloned().ok_or_else(|| anyhow!("Not found"))
        }
        fn set(&self, _profile: &str, key: &str, value: &str) -> Result<()> {
            self.data.lock().unwrap().insert(key.to_string(), value.to_string());
            Ok(())
        }
        fn delete(&self, _profile: &str, key: &str) -> Result<()> {
            self.data.lock().unwrap().remove(key);
            Ok(())
        }
        fn clear_profile(&self, _profile: &str) -> Result<()> {
            self.data.lock().unwrap().clear();
            Ok(())
        }
    }

    struct MockPool {
        vault: Arc<dyn Vault>,
    }

    impl TokenPool for MockPool {
        fn get_app_ticket(&self, _p: &str) -> Result<Ticket> { Err(anyhow!("")) }
        fn set_app_ticket(&self, _p: &str, _t: &Ticket) -> Result<()> { Ok(()) }
        fn get_access_token(&self, _p: &str) -> Result<Token> { Err(anyhow!("")) }
        fn set_access_token(&self, _p: &str, _t: &Token) -> Result<()> { Ok(()) }
        fn delete_access_token(&self, _p: &str) -> Result<()> { Ok(()) }
        fn clear_cache(&self, _p: &str) {}
        fn as_vault(&self) -> Arc<dyn Vault> { self.vault.clone() }
    }

    #[test]
    fn test_session_lifecycle() {
        let vault = Arc::new(MockVault { data: Mutex::new(HashMap::new()) });
        let pool = MockPool { vault };
        let manager = AuthSessionManager::new(&pool);

        let session = manager.create_session("test", 1234).unwrap();
        assert_eq!(session.redirect_port, 1234);
        assert!(!session.code_verifier.is_empty());

        let retrieved = manager.get_session("test").unwrap();
        assert_eq!(retrieved.state, session.state);

        manager.save_code("test", "captured_code", &session.state).unwrap();
        assert_eq!(manager.get_captured_code("test").unwrap(), "captured_code");

        manager.clear("test").unwrap();
        assert!(manager.get_session("test").is_err());
    }
}
