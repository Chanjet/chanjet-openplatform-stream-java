use tonic::{Request, Response, Status};
use std::sync::Arc;
use cowen_common::vault::Vault;
use cowen_config::ConfigManager;
use cowen_common::grpc::proto::api_registry_service_server::ApiRegistryService;
use cowen_common::grpc::proto::{ApiListRequest, ApiListResponse, ApiSpecRequest, ApiSpecResponse, CallApiRequest, CallApiResponse};
use cowen_macros::{rbac, rbac_controller};
use cowen_auth::client::Client;

pub struct ApiRegistryController {
    vault: Arc<dyn Vault>,
    cfg_mgr: ConfigManager,
    search_orchestrator: crate::orchestrators::SearchOrchestrator,
    api_orchestrator: crate::orchestrators::ApiOrchestrator,
}

impl ApiRegistryController {
    pub fn new(vault: Arc<dyn Vault>, cfg_mgr: ConfigManager) -> Self {
        Self {
            vault: vault.clone(),
            cfg_mgr: cfg_mgr.clone(),
            search_orchestrator: crate::orchestrators::SearchOrchestrator::new(),
            api_orchestrator: crate::orchestrators::ApiOrchestrator::new(vault.clone(), cfg_mgr.clone()),
        }
    }
}

#[rbac_controller(domain = "native.api.registry")]
#[tonic::async_trait]
impl ApiRegistryService for ApiRegistryController {
    #[rbac(action = "execute")]
    async fn call_api(&self, request: Request<CallApiRequest>) -> Result<Response<CallApiResponse>, Status> {
        self.api_orchestrator.call_api(request.into_inner()).await
    }

    #[rbac(action = "search")]
    async fn api_list(&self, request: Request<ApiListRequest>) -> Result<Response<ApiListResponse>, Status> {
        let req = request.into_inner();
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Err(Status::not_found(e.to_string()))
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        match auth_cli.get_openapi_spec(&req.profile, &config, req.refresh).await {
            Ok(spec) => {
                let mut ops = crate::openapi_parser::OpenApiParser::parse_operations(&spec);
                
                let (filtered_ops, used_plugin_name) = self.search_orchestrator.search_if_needed(
                    &req.profile, 
                    ops, 
                    &req.search
                ).await;
                
                ops = filtered_ops;
                
                let total = ops.len() as u32;
                
                let page = req.page.max(1) as usize;
                let page_size = req.page_size.max(1) as usize;
                let start = (page - 1) * page_size;
                let end = (start + page_size).min(ops.len());
                
                let paged_ops = if start < ops.len() {
                    ops[start..end].to_vec()
                } else {
                    Vec::new()
                };

                let json = serde_json::to_string(&paged_ops).unwrap_or_default();
                Ok(Response::new(ApiListResponse { total, json, plugin_used: used_plugin_name, error_message: None }))
            }
            Err(e) => Ok(Response::new(ApiListResponse { total: 0, json: "".to_string(), plugin_used: None, error_message: Some(format!("{}", e)) }))
        }
    }

    #[rbac(action = "read")]
    async fn api_spec(&self, request: Request<ApiSpecRequest>) -> Result<Response<ApiSpecResponse>, Status> {
        self.api_orchestrator.api_spec(request.into_inner()).await
    }
}
