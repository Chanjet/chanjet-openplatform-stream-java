use crate::core::config::Config;
use crate::auth::pool::TokenPool;
use crate::auth::models::Token;
use anyhow::{Result, anyhow, Context};
use serde::Deserialize;
use chrono::{Utc, Duration};
use reqwest::Client as HttpClient;

#[async_trait::async_trait]
pub trait Client {
    async fn get_app_access_token(&self, profile: &str, cfg: &Config) -> Result<Token>;
    async fn trigger_push(&self, profile: &str, cfg: &Config) -> Result<()>;
    async fn get_openapi_spec(&self, profile: &str, cfg: &Config) -> Result<serde_json::Value>;
}

pub struct AuthClient<'a> {
    pool: &'a (dyn TokenPool + Send + Sync),
    http: HttpClient,
}

#[derive(Debug, Deserialize)]
struct PlatformTokenResponse {
    result: bool,
    error: Option<serde_json::Value>,
    value: Option<TokenValue>,
}

#[derive(Debug, Deserialize)]
struct TokenValue {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "expiresIn")]
    expires_in: i64,
}

impl<'a> AuthClient<'a> {
    pub fn new(pool: &'a (dyn TokenPool + Send + Sync)) -> Self {
        Self {
            pool,
            http: HttpClient::new(),
        }
    }
}

#[async_trait::async_trait]
impl<'a> Client for AuthClient<'a> {
    async fn get_app_access_token(&self, profile: &str, cfg: &Config) -> Result<Token> {
        // 1. Check pool
        if let Ok(token) = self.pool.get_access_token(profile) {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        // 2. Perform network refresh
        let ticket = self.pool.get_app_ticket(profile)
            .context("Missing app_ticket, please ensure daemon is running and app_ticket is received.")?;

        let url = format!("{}/v1/common/auth/selfBuiltApp/generateToken", cfg.openapi_url);
        
        let body = serde_json::json!({
            "appTicket": ticket.value,
            "certificate": cfg.certificate,
        });

        let resp = self.http.post(&url)
            .header("appKey", &cfg.app_key)
            .header("appSecret", &cfg.app_secret)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await?;
            return Err(anyhow!("Platform auth failed (HTTP {}): {}", status, err_text));
        }

        let token_resp: PlatformTokenResponse = resp.json().await?;
        
        if !token_resp.result {
            return Err(anyhow!("Platform error: {:?}", token_resp.error));
        }

        let val = token_resp.value.context("Platform returned success but value is empty")?;
        
        let new_token = Token {
            value: val.access_token,
            expires_at: Utc::now() + Duration::seconds(val.expires_in),
        };

        // 3. Save to pool
        self.pool.set_access_token(profile, &new_token)?;

        Ok(new_token)
    }

    async fn trigger_push(&self, _profile: &str, cfg: &Config) -> Result<()> {
        let url = format!("{}/auth/appTicket/resend", cfg.openapi_url);
        
        let body = serde_json::json!({});

        let resp = self.http.post(&url)
            .header("appKey", &cfg.app_key)
            .header("appSecret", &cfg.app_secret)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await?;
            return Err(anyhow!("Failed to trigger push (HTTP {}): {}", status, err_text));
        }

        #[derive(Deserialize)]
        struct ResendResp {
            code: String,
            message: Option<String>,
        }

        let resend_resp: ResendResp = resp.json().await?;
        if resend_resp.code != "200" {
            return Err(anyhow!("Platform error: {} - {:?}", resend_resp.code, resend_resp.message));
        }

        Ok(())
    }

    async fn get_openapi_spec(&self, profile: &str, _cfg: &Config) -> Result<serde_json::Value> {
        let home = directories::UserDirs::new()
            .context("Could not find home directory")?
            .home_dir()
            .to_path_buf();
        let cache_path = home.join(".cjtc").join(format!("{}_openapi.json", profile));

        // 1. Try Cache with Staleness Check (1 hour TTL)
        if cache_path.exists() {
            let metadata = std::fs::metadata(&cache_path)?;
            let is_stale = metadata.modified()
                .map(|m| m.elapsed().map(|e| e.as_secs() > 3600).unwrap_or(true))
                .unwrap_or(true);

            if !is_stale {
                let data = std::fs::read_to_string(&cache_path)?;
                if let Ok(spec) = serde_json::from_str(&data) {
                    return Ok(spec);
                }
            }
        }

        // 2. Generate Mock Spec (Same as Go version for parity)
        let spec = self.generate_mock_spec();

        // 3. Save Cache
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json_data = serde_json::to_string_pretty(&spec)?;
        let _ = std::fs::write(cache_path, json_data);

        Ok(spec)
    }
}

impl<'a> AuthClient<'a> {
    fn generate_mock_spec(&self) -> serde_json::Value {
        let mut paths = serde_json::Map::new();

        let mut add = |path: &str, method: &str, summary: &str, desc: &str, req_header: bool| {
            let mut methods = paths
                .get(path)
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default();
            
            let mut parameters = serde_json::json!([]);
            if req_header {
                parameters = serde_json::json!([
                    {"name": "appKey", "in": "header", "required": true, "schema": {"type": "string"}},
                    {"name": "appSecret", "in": "header", "required": true, "schema": {"type": "string"}},
                    {"name": "openToken", "in": "header", "required": true, "schema": {"type": "string"}}
                ]);
            }

            let op = serde_json::json!({
                "summary": summary,
                "description": desc,
                "parameters": parameters,
                "responses": {
                    "200": {"description": "成功"},
                    "400": {"description": "请求参数错误"}
                }
            });
            methods.insert(method.to_string(), op);
            paths.insert(path.to_string(), serde_json::Value::Object(methods));
        };

        // Key business APIs (With expanded keywords for better search hits)
        add("/v1/user/profile", "get", "获取个人画像", "返回包含头像、部门及常用功能数据 (用户信息/权限)。", false);
        add("/v1/inventory/query", "get", "查询全渠道库存", "支持仓库或商品编码查询可用余量 (库存/盘点/调拨)。", true);
        add("/v1/orders/detail", "get", "获取订单详情", "包括商品、促销、物流状态 (订单/下单/退款)。", false);
        add("/v1/orders/{id}", "get", "获取单个订单", "查询指定ID订单", true);
        add("/v1/orders/{orderId}/status", "put", "更新订单状态", "路径加操作节点测试", false);
        add("/v1/users/{userId}/addresses/{addressId}", "get", "获取用户收货地址", "复杂路径变量测试", true);
        add("/v1/payment/batch-transfer", "post", "批量转账代发", "支持工资发放、佣金结算 (算账/报账/查钱/打款)。", true);
        add("/v1/crm/customer/register", "post", "录入客户档案", "记录客户名称、来源及跟进负责人 (客户/公海/CRM)。", false);
        add("/v1/hr/attendance/summary", "get", "导出月度考勤报表", "整合打卡、请假、出差数据 (考勤/工资/绩效)。", false);

        let categories = vec![
            ("fin", "财务结算", "处理企业日常财务流水。"),
            ("scm", "供应链管控", "优化采购流程。"),
            ("mkt", "营销推广", "策划优惠券发放。"),
            ("hr", "人事办公", "同步办理社保。"),
        ];

        for (prefix, summary, desc) in categories {
            for i in 1..=20 {
                let path = format!("/v1/{}/api/{}", prefix, i);
                add(&path, "get", &format!("{}-Mock-{}", summary, i), desc, i % 2 == 0);
            }
        }

        serde_json::json!({
            "openapi": "3.0.1",
            "info": {
                "title": "Chanjet Mock API",
                "version": "1.2.0",
                "description": "提供 100+ 个业务接口，用于发现与搜索。"
            },
            "paths": paths
        })
    }
}

pub fn find_matching_spec_path(req_path: &str, spec: &serde_json::Value) -> Option<String> {
    if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
        if paths.contains_key(req_path) {
            return Some(req_path.to_string());
        }
        let req_segments: Vec<&str> = req_path.split('/').filter(|s| !s.is_empty()).collect();
        for spec_path in paths.keys() {
            let spec_segments: Vec<&str> = spec_path.split('/').filter(|s| !s.is_empty()).collect();
            if req_segments.len() == spec_segments.len() {
                let mut match_ok = true;
                for (req_seg, spec_seg) in req_segments.iter().zip(spec_segments.iter()) {
                    if spec_seg.starts_with('{') && spec_seg.ends_with('}') {
                        continue; // matches path variable
                    }
                    if req_seg != spec_seg {
                        match_ok = false;
                        break;
                    }
                }
                if match_ok {
                    return Some(spec_path.clone());
                }
            }
        }
    }
    None
}

pub fn get_operation(spec: &serde_json::Value, path: &str, method: &str) -> Option<serde_json::Value> {
    if let Some(matched_path) = find_matching_spec_path(path, spec) {
        spec.get("paths")?
            .get(&matched_path)?
            .get(method.to_lowercase())
            .cloned()
    } else {
        None
    }
}

pub fn is_path_in_whitelist(req_path: &str, spec: &serde_json::Value) -> bool {
    find_matching_spec_path(req_path, spec).is_some()
}
