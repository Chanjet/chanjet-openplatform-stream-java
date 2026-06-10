#![allow(dead_code, unused_imports, unused_variables)]
// Config specific capability
use cowen_auth::client::Client;
use cowen_common::daemon::DaemonService;
use cowen_common::grpc::proto::*;
use cowen_common::{vault::Vault, CowenError};
use cowen_config::ConfigManager;
use cowen_macros::{rbac, rbac_controller};
use std::sync::Arc;
use tracing::info;

#[tonic::async_trait]
pub trait NativeConfigCapability: Send + Sync {
    async fn get_app_ticket(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        app_key: &str,
    ) -> Result<Option<cowen_common::models::Ticket>, CowenError>;
    async fn get_app_secret(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        profile: &str,
    ) -> Result<String, CowenError>;
    async fn get_global_config(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: GetGlobalConfigRequest,
    ) -> Result<GetGlobalConfigResponse, CowenError>;
    async fn set_global_config(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: SetGlobalConfigRequest,
    ) -> Result<SetGlobalConfigResponse, CowenError>;
    async fn get_config(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: GetConfigRequest,
    ) -> Result<GetConfigResponse, CowenError>;
    async fn list_config(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: ListConfigRequest,
    ) -> Result<ListConfigResponse, CowenError>;
    async fn set_config(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: SetConfigRequest,
    ) -> Result<SetConfigResponse, CowenError>;
    async fn rename_profile(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: RenameProfileRequest,
    ) -> Result<RenameProfileResponse, CowenError>;
}

pub struct DefaultConfigCapability {
    service: Arc<dyn DaemonService>,
    vault: Arc<dyn Vault>,
    cfg_mgr: ConfigManager,
}

impl DefaultConfigCapability {
    pub fn new(
        service: Arc<dyn DaemonService>,
        vault: Arc<dyn Vault>,
        cfg_mgr: ConfigManager,
    ) -> Self {
        Self {
            service,
            vault,
            cfg_mgr,
        }
    }
}

#[rbac_controller(domain = "native.config")]
#[tonic::async_trait]
impl NativeConfigCapability for DefaultConfigCapability {
    #[rbac(action = "read")]
    async fn get_app_ticket(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        app_key: &str,
    ) -> Result<Option<cowen_common::models::Ticket>, CowenError> {
        match self.vault.get_app_ticket(app_key).await {
            Ok(t) => Ok(Some(t)),
            Err(CowenError::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    #[rbac(action = "read")]
    async fn get_app_secret(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        profile: &str,
    ) -> Result<String, CowenError> {
        match self.cfg_mgr.load(profile).await {
            Ok(config) => Ok(config.app_secret.clone()),
            Err(e) => Err(CowenError::config(e.to_string())),
        }
    }
    #[rbac]
    // get_global_config has no rbac
    async fn get_global_config(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        _req: GetGlobalConfigRequest,
    ) -> Result<GetGlobalConfigResponse, CowenError> {
        match self.cfg_mgr.load_app_config().await {
            Ok(c) => Ok(GetGlobalConfigResponse {
                config_json: serde_json::to_string_pretty(&c).unwrap_or_default(),
                error_message: None,
            }),
            Err(e) => Ok(GetGlobalConfigResponse {
                config_json: "".to_string(),
                error_message: Some(e.to_string()),
            }),
        }
    }

    #[rbac]
    async fn set_global_config(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: SetGlobalConfigRequest,
    ) -> Result<SetGlobalConfigResponse, CowenError> {
        let value = req.value.trim();

        match self.cfg_mgr.set_value("", &req.key, value).await {
            Ok(_) => Ok(SetGlobalConfigResponse {
                success: true,
                error_message: None,
            }),
            Err(e) => Ok(SetGlobalConfigResponse {
                success: false,
                error_message: Some(e.to_string()),
            }),
        }
    }

    #[rbac(profile = "req.profile.as_str()")]
    async fn get_config(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: GetConfigRequest,
    ) -> Result<GetConfigResponse, CowenError> {
        match self.cfg_mgr.get_value(&req.profile, &req.key).await {
            Ok(v) => {
                let val = match v {
                    serde_json::Value::String(s) => s,
                    _ => v.to_string(),
                };
                Ok(GetConfigResponse {
                    config_json: val,
                    error_message: None,
                })
            }
            Err(e) => Ok(GetConfigResponse {
                config_json: "".to_string(),
                error_message: Some(e.to_string()),
            }),
        }
    }

    #[rbac(profile = "req.profile.as_str()")]
    async fn list_config(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: ListConfigRequest,
    ) -> Result<ListConfigResponse, CowenError> {
        let res = if req.all {
            self.cfg_mgr
                .list_all_values()
                .await
                .map(|v| serde_json::to_string(&v).unwrap_or_default())
        } else {
            self.cfg_mgr
                .list_values(&req.profile)
                .await
                .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
        };

        match res {
            Ok(json) => Ok(ListConfigResponse {
                config_json: json,
                error_message: None,
            }),
            Err(e) => Ok(ListConfigResponse {
                config_json: "".to_string(),
                error_message: Some(e.to_string()),
            }),
        }
    }

    #[rbac(profile = "req.profile.as_str()")]
    async fn set_config(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: SetConfigRequest,
    ) -> Result<SetConfigResponse, CowenError> {
        let value = req.value.trim();

        match self.cfg_mgr.set_value(&req.profile, &req.key, value).await {
            Ok(_) => Ok(SetConfigResponse {
                success: true,
                error_message: None,
            }),
            Err(e) => Ok(SetConfigResponse {
                success: false,
                error_message: Some(e.to_string()),
            }),
        }
    }

    #[rbac]
    async fn rename_profile(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: RenameProfileRequest,
    ) -> Result<RenameProfileResponse, CowenError> {
        let old_name = req.old_name;
        let new_name = req.new_name;

        let _ = self.service.stop_daemon(&old_name).await;

        match self.cfg_mgr.rename(&old_name, &new_name).await {
            Ok(_) => Ok(RenameProfileResponse {
                success: true,
                message: format!("Renamed to {}", new_name),
            }),
            Err(e) => Ok(RenameProfileResponse {
                success: false,
                message: e.to_string(),
            }),
        }
    }
}
