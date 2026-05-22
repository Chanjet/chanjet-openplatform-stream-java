use async_trait::async_trait;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use cowen_auth::pool::TokenPool;
use cowen_auth::provider::oauth2::{OAuth2Provider, Pkce};
use cowen_auth::provider::AuthProvider;
use cowen_common::domain::*;
use cowen_common::models::{AuthSession, Ticket, Token};
use cowen_common::{CowenError, CowenResult};
use cowen_store::Item;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[test]
fn test_pkce_generation() {
    let pkce = Pkce::new();
    assert_eq!(pkce.verifier.len(), 64);

    // Verify challenge can be computed from verifier
    let challenge = Pkce::generate_challenge(&pkce.verifier);
    assert!(!challenge.is_empty());

    // Manual verification of challenge
    let mut hasher = Sha256::new();
    hasher.update(pkce.verifier.as_bytes());
    let expected_challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
    assert_eq!(challenge, expected_challenge);
}

#[test]
fn test_oauth2_capabilities() {
    struct MockVault {}
    #[async_trait]
    impl cowen_common::vault::Vault for MockVault {
        fn primary_store(&self) -> Arc<dyn cowen_store::Store> {
            unimplemented!()
        }
    }

    #[async_trait]
    impl PermanentCodeDomain for MockVault {
        async fn get_org_permanent_code(&self, _: &str, _: &str) -> CowenResult<String> {
            Err(CowenError::Auth(format!("not found")))
        }
        async fn save_org_permanent_code(&self, _: &str, _: &str, _: &str) -> CowenResult<()> {
            Ok(())
        }
        async fn get_user_permanent_code(&self, _: &str, _: &str, _: &str) -> CowenResult<String> {
            Err(CowenError::Auth(format!("not found")))
        }
        async fn save_user_permanent_code(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> CowenResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl TicketDomain for MockVault {
        async fn get_app_ticket(&self, _: &str) -> CowenResult<Ticket> {
            Err(CowenError::Auth(format!("not found")))
        }
        async fn save_app_ticket(&self, _: &str, _: Ticket) -> CowenResult<()> {
            Ok(())
        }
        async fn delete_app_ticket(&self, _: &str) -> CowenResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl TokenDomain for MockVault {
        async fn get_access_token(&self, _: &str) -> CowenResult<Token> {
            Err(CowenError::Auth(format!("not found")))
        }
        async fn save_access_token(&self, _: &str, _: Token) -> CowenResult<()> {
            Ok(())
        }
        async fn delete_access_token(&self, _: &str) -> CowenResult<()> {
            Ok(())
        }
        async fn get_refresh_token(&self, _: &str) -> CowenResult<Token> {
            Err(CowenError::Auth(format!("not found")))
        }
        async fn save_refresh_token(&self, _: &str, _: Token) -> CowenResult<()> {
            Ok(())
        }
        async fn delete_refresh_token(&self, _: &str) -> CowenResult<()> {
            Ok(())
        }
        async fn get_app_access_token(&self, _: &str) -> CowenResult<Token> {
            Err(CowenError::Auth(format!("not found")))
        }
        async fn save_app_access_token(&self, _: &str, _: Token) -> CowenResult<()> {
            Ok(())
        }
        async fn delete_app_access_token(&self, _: &str) -> CowenResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl SessionDomain for MockVault {
        async fn get_session(&self, _: &str) -> CowenResult<AuthSession> {
            Err(CowenError::Auth(format!("not found")))
        }
        async fn save_session(&self, _: AuthSession) -> CowenResult<()> {
            Ok(())
        }
        async fn delete_session(&self, _: &str) -> CowenResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl SecretDomain for MockVault {
        async fn get_secret(&self, _: &str, _: &str) -> CowenResult<String> {
            Ok("".to_string())
        }
        async fn set_secret(&self, _: &str, _: &str, _: &str) -> CowenResult<()> {
            Ok(())
        }
        async fn delete_secret(&self, _: &str, _: &str) -> CowenResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl ConfigDomain for MockVault {
        async fn get_config(&self, _: &str, _: &str) -> CowenResult<String> {
            Ok("".to_string())
        }
        async fn get_config_metadata(&self, _: &str, _: &str) -> CowenResult<(u64, i64)> {
            Ok((0, 0))
        }
        async fn get_config_full(&self, _: &str, _: &str) -> CowenResult<Item> {
            Err(CowenError::Auth(format!("not found")))
        }
        async fn set_config(&self, _: &str, _: &str, _: &str) -> CowenResult<()> {
            Ok(())
        }
        async fn set_config_conditional(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: u64,
        ) -> CowenResult<()> {
            Ok(())
        }
        async fn list_configs(&self, _: &str) -> CowenResult<Vec<String>> {
            Ok(vec![])
        }
        async fn delete_config(&self, _: &str, _: &str) -> CowenResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl AuditDomain for MockVault {
        async fn save_audit(&self, _: &cowen_common::models::AuditEntry) -> CowenResult<()> {
            Ok(())
        }
        async fn list_audit(
            &self,
            _: &str,
            _: usize,
        ) -> CowenResult<Vec<cowen_common::models::AuditEntry>> {
            Ok(vec![])
        }
    }

    #[async_trait]
    impl DlqDomain for MockVault {
        async fn push_dlq(&self, _: &cowen_common::models::DlqMessage) -> CowenResult<()> {
            Ok(())
        }
        async fn pop_dlq(
            &self,
            _: &str,
            _: &str,
        ) -> CowenResult<Option<cowen_common::models::DlqMessage>> {
            Ok(None)
        }
        async fn list_dlq(
            &self,
            _: &str,
            _: usize,
        ) -> CowenResult<Vec<cowen_common::models::DlqMessage>> {
            Ok(vec![])
        }
        async fn list_all_dlq(
            &self,
            _: &str,
        ) -> CowenResult<Vec<cowen_common::models::DlqMessage>> {
            Ok(vec![])
        }
        async fn get_dlq_by_id(&self, _: i64) -> CowenResult<Option<cowen_common::models::DlqMessage>> {
            Ok(None)
        }
        async fn list_dlq_paged(&self, _: &str, _: usize, _: usize) -> CowenResult<Vec<cowen_common::models::DlqMessage>> {
            Ok(vec![])
        }
        async fn delete_dlq_by_id(&self, _: i64) -> CowenResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl ManagementDomain for MockVault {
        async fn clear_profile(&self, _: &str) -> CowenResult<()> {
            Ok(())
        }
        async fn rename_profile(&self, _: &str, _: &str) -> CowenResult<()> {
            Ok(())
        }
        async fn list_all_profiles(&self) -> CowenResult<Vec<String>> {
            Ok(vec![])
        }
    }

    struct MockHttpSender {}
    #[async_trait]
    impl cowen_auth::client::HttpSender for MockHttpSender {
        async fn post(
            &self,
            _: &str,
            _: reqwest::header::HeaderMap,
            _: serde_json::Value,
        ) -> CowenResult<cowen_auth::client::SimpleResponse> {
            Ok(cowen_auth::client::SimpleResponse {
                status: 200,
                body: "{}".to_string(),
            })
        }
        async fn post_form(
            &self,
            _: &str,
            _: reqwest::header::HeaderMap,
            _: serde_json::Value,
        ) -> CowenResult<cowen_auth::client::SimpleResponse> {
            Ok(cowen_auth::client::SimpleResponse {
                status: 200,
                body: "{}".to_string(),
            })
        }
        async fn get(
            &self,
            _: &str,
            _: reqwest::header::HeaderMap,
        ) -> CowenResult<cowen_auth::client::SimpleResponse> {
            Ok(cowen_auth::client::SimpleResponse {
                status: 200,
                body: "{}".to_string(),
            })
        }
    }

    let vault = Arc::new(MockVault {});
    let pool: Arc<dyn TokenPool> = Arc::new(cowen_auth::VaultTokenPool::new(vault));
    let sender = Arc::new(MockHttpSender {});
    let provider = OAuth2Provider::new(pool, sender);
    assert!(!provider.supports_webhooks());
}

#[test]
fn test_verifier_charset() {
    let verifier = Pkce::generate_verifier(1000);
    let allowed = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    for c in verifier.chars() {
        assert!(allowed.contains(c));
    }
}
