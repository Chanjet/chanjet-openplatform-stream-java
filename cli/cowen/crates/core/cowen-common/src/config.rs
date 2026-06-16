use serde::{Deserialize, Serialize};

pub const BUILTIN_CLIENT_ID: &str = env!("BUILTIN_CLIENT_ID");
pub const DEF_MARKET_URL: &str = env!("DEF_MARKET_URL");

// ============================================================================
// Gateway Configuration (PRD v0.5.0 Identity-Aware Gateway)
// ============================================================================

/// Auth routing mode for the Identity-Aware Gateway.
///
/// - `STRICT`: Default-deny. All requests require auth unless matched by `bypass_rules`.
/// - `PERMISSIVE`: Default-allow. Only requests matching `require_rules` require auth.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AuthRoutingMode {
    #[serde(rename = "STRICT")]
    #[default]
    Strict,
    #[serde(rename = "PERMISSIVE")]
    Permissive,
}

impl std::fmt::Display for AuthRoutingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthRoutingMode::Strict => write!(f, "STRICT"),
            AuthRoutingMode::Permissive => write!(f, "PERMISSIVE"),
        }
    }
}

impl std::str::FromStr for AuthRoutingMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "STRICT" => Ok(AuthRoutingMode::Strict),
            "PERMISSIVE" => Ok(AuthRoutingMode::Permissive),
            _ => Err(format!(
                "Invalid auth routing mode: '{}'. Supported: STRICT, PERMISSIVE",
                s
            )),
        }
    }
}

/// Routing rules for the Identity-Aware Gateway's auth enforcement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AuthRoutingConfig {
    /// The interception mode: STRICT (default-deny) or PERMISSIVE (default-allow).
    #[serde(default)]
    pub mode: AuthRoutingMode,

    /// Glob patterns for paths that REQUIRE authentication (used in PERMISSIVE mode).
    /// Example: `["/api/**", "/user/invoice/**"]`
    #[serde(default)]
    pub require_rules: Vec<String>,

    /// Glob patterns for paths that BYPASS authentication (used in STRICT mode).
    /// Example: `["/health", "/static/**"]`
    #[serde(default)]
    pub bypass_rules: Vec<String>,
}

/// Configuration for the Identity-Aware Gateway (Ingress reverse proxy).
///
/// This configuration block, when present on a `store-app` profile, enables
/// the Cowen Sidecar to act as an Identity-Aware Proxy that intercepts
/// browser traffic, handles OAuth code exchange, manages encrypted JWE
/// sessions, and reverse-proxies to the ISV backend with identity headers.
///
/// **Constraint**: Gateway is ONLY valid for `store-app` mode. If present
/// on a non-store-app profile, the daemon will refuse to start.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayConfig {
    /// The address the gateway listens on. Default: "127.0.0.1:8080".
    /// For cloud-native sidecar mode, bind to loopback.
    /// For VM/centralized gateway mode, can bind to "0.0.0.0:<port>".
    #[serde(default = "default_gateway_bind_address")]
    pub bind_address: String,

    /// The ISV backend URL to reverse-proxy authenticated requests to.
    /// Example: "http://127.0.0.1:3000" or "https://remote-isv.com"
    pub upstream_url: String,

    /// Optional synchronous webhook URL called during code exchange.
    /// When set, Cowen blocks the 302 redirect until this hook returns 200.
    /// The ISV can return Set-Cookie headers that are merged into the 302 response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_sync_hook: Option<String>,

    /// Auth routing rules controlling which paths require authentication.
    #[serde(default)]
    pub auth_routing: AuthRoutingConfig,

    /// Custom routing rules for multiple upstreams or direct OpenAPI proxying.
    #[serde(default)]
    pub routes: Vec<GatewayRouteRule>,
}

/// Custom routing rule for Gateway forwarding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct GatewayRouteRule {
    /// Glob pattern to match against request path.
    pub path: String,
    /// Destination URL or special keyword "openapi".
    pub upstream: String,
    /// Optional prefix to strip from path before forwarding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strip_prefix: Option<String>,
}

fn default_gateway_bind_address() -> String {
    "127.0.0.1:8080".to_string()
}

// ============================================================================
// App-level Configuration (app.yaml)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default = "default_zero")]
    pub monitor_port: u16,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default = "default_log")]
    pub log: LogConfig,
    #[serde(default = "default_openapi_url")]
    pub openapi_url: String,
    #[serde(default = "default_stream_url")]
    pub stream_url: String,
    #[serde(default)]
    pub plugins: Vec<String>,
    #[serde(default = "default_true")]
    pub telemetry_enabled: bool,
}

fn default_openapi_url() -> String {
    env!("DEF_OPENAPI_URL").to_string()
}

fn default_stream_url() -> String {
    env!("DEF_STREAM_URL").to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            storage: StorageConfig::default(),
            monitor_port: 0,
            security: SecurityConfig::default(),
            log: default_log(),
            openapi_url: default_openapi_url(),
            stream_url: default_stream_url(),
            plugins: vec![],
            telemetry_enabled: true,
        }
    }
}

impl AppConfig {
    pub fn apply_env_overrides(&mut self) {
        if let Ok(url) = std::env::var("COWEN_OPENAPI_URL") {
            self.openapi_url = url;
        }
        if let Ok(url) = std::env::var("COWEN_STREAM_URL") {
            self.stream_url = url;
        }
        if let Ok(val) = std::env::var("COWEN_TELEMETRY_ENABLED") {
            self.telemetry_enabled = val == "true" || val == "1";
        }
        if let Ok(val) = std::env::var("COWEN_MONITOR_PORT") {
            if let Ok(port) = val.parse::<u16>() {
                self.monitor_port = port;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageConfig {
    #[serde(default = "default_store")]
    pub store: String,
    pub db_url: Option<String>,
    #[serde(default = "default_cache")]
    pub cache: String,
    pub cache_url: Option<String>,
}

fn default_store() -> String {
    "innerdb".to_string()
}
fn default_cache() -> String {
    "none".to_string()
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            store: default_store(),
            db_url: None,
            cache: default_cache(),
            cache_url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum SecurityLevel {
    #[serde(rename = "strict")]
    #[default]
    Strict,
    #[serde(rename = "flexible")]
    Flexible,
    #[serde(rename = "disabled")]
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SecurityConfig {
    #[serde(default)]
    pub level: SecurityLevel,
    #[serde(default)]
    pub allow_cidr: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub app_key: String,
    pub webhook_target: String,
    #[serde(default = "default_zero")]
    pub proxy_port: u16,
    #[serde(default = "default_true")]
    pub proxy_enabled: bool,
    #[serde(default)]
    pub app_mode: crate::models::AuthMode,
    #[serde(skip)]
    pub app_secret: String,
    #[serde(skip)]
    pub certificate: String,
    #[serde(skip)]
    pub encrypt_key: String,
    #[serde(default)]
    pub version: u64,
    #[serde(default)]
    pub exclusive: Option<bool>,
    /// Identity-Aware Gateway configuration (PRD v0.5.0).
    /// When present, enables Ingress reverse proxy with OAuth code interception.
    /// **Only valid for `store-app` mode.**
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<GatewayConfig>,
}

fn default_true() -> bool {
    true
}
fn default_zero() -> u16 {
    0
}

fn default_log() -> LogConfig {
    LogConfig {
        level: "info".to_string(),
        rotation: default_rotation(),
        max_size_mb: default_max_size(),
        max_files: default_max_files(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_rotation")]
    pub rotation: String,
    #[serde(default = "default_max_size")]
    pub max_size_mb: u64,
    #[serde(default = "default_max_files")]
    pub max_files: usize,
}

fn default_rotation() -> String {
    "daily".to_string()
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_max_size() -> u64 {
    100
}
fn default_max_files() -> usize {
    7
}

impl Config {
    pub fn default_with_profile(_p: &str) -> Self {
        Self {
            app_key: "".to_string(),
            webhook_target: "http://localhost:8080".to_string(),
            proxy_port: 0,
            proxy_enabled: true,
            app_mode: crate::models::AuthMode::Oauth2,
            app_secret: "".to_string(),
            certificate: "".to_string(),
            encrypt_key: "".to_string(),
            version: 0,
            exclusive: None,
            gateway: None,
        }
    }

    pub fn apply_env_overrides(&mut self) {
        if let Ok(key) = std::env::var("COWEN_APP_KEY") {
            self.app_key = key;
        }
        if let Ok(secret) = std::env::var("COWEN_APP_SECRET") {
            self.app_secret = secret;
        }
        if let Ok(ek) = std::env::var("COWEN_ENCRYPT_KEY") {
            self.encrypt_key = ek;
        }
        if let Ok(target) = std::env::var("COWEN_WEBHOOK_TARGET") {
            self.webhook_target = target;
        }
        if let Ok(port) = std::env::var("COWEN_PROXY_PORT") {
            if let Ok(p) = port.parse::<u16>() {
                self.proxy_port = p;
            }
        }
        if let Ok(mode) = std::env::var("COWEN_APP_MODE") {
            self.app_mode = match mode.as_str() {
                "self-built" => crate::models::AuthMode::SelfBuilt,
                "store-app" => crate::models::AuthMode::StoreApp,
                _ => crate::models::AuthMode::Oauth2,
            };
        }
        self.apply_gateway_env_overrides();
    }

    /// Apply gateway-specific environment variable overrides (PRD v0.5.0).
    fn apply_gateway_env_overrides(&mut self) {
        if let Ok(val) = std::env::var("COWEN_GATEWAY_ENABLED") {
            let enabled = val == "true" || val == "1";
            if enabled && self.gateway.is_none() {
                self.gateway = Some(GatewayConfig {
                    bind_address: default_gateway_bind_address(),
                    upstream_url: String::new(),
                    auth_sync_hook: None,
                    auth_routing: AuthRoutingConfig::default(),
                    routes: vec![],
                });
            } else if !enabled {
                self.gateway = None;
            }
        }
        if let Ok(bind) = std::env::var("COWEN_GATEWAY_BIND") {
            if let Some(ref mut gw) = self.gateway {
                gw.bind_address = bind;
            }
        }
        if let Ok(upstream) = std::env::var("COWEN_GATEWAY_UPSTREAM") {
            if let Some(ref mut gw) = self.gateway {
                gw.upstream_url = upstream;
            }
        }
        if let Ok(mode) = std::env::var("COWEN_GATEWAY_MODE") {
            if let Some(ref mut gw) = self.gateway {
                if let Ok(m) = mode.parse::<AuthRoutingMode>() {
                    gw.auth_routing.mode = m;
                }
            }
        }
    }

    /// Validates that the gateway configuration is compatible with the app mode.
    /// Gateway is ONLY valid for `store-app` mode.
    pub fn validate_gateway_compatibility(&self) -> Result<(), String> {
        if self.gateway.is_some() && self.app_mode != crate::models::AuthMode::StoreApp {
            return Err(format!(
                "Gateway configuration is only supported in 'store-app' mode. \
                 Current mode: '{}'. Remove the gateway configuration or switch to store-app mode.",
                self.app_mode
            ));
        }
        Ok(())
    }
}

pub use cowen_infra::path::get_app_dir;

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("app_key", &self.app_key)
            .field("app_mode", &self.app_mode)
            .field("version", &self.version)
            .finish()
    }
}
