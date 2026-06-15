// PRD v0.5.0 — Gateway Auth Routing Engine
//
// Implements the route classification and request-type detection logic
// for the Identity-Aware Gateway's zero-trust auth enforcement.

use axum::http::HeaderMap;
use cowen_common::config::{AuthRoutingConfig, AuthRoutingMode};

/// The outcome of routing a request through the auth routing engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteDecision {
    /// The request path requires a valid session (auth enforced).
    RequiresAuth,
    /// The request path is exempt from authentication (bypass).
    BypassAuth,
}

/// Detected request type based on Accept headers.
/// Used to determine the appropriate unauthorized response strategy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestType {
    /// API/data request (Accept: application/json, X-Requested-With, etc.)
    Api,
    /// Page/browser navigation (Accept: text/html)
    Page,
}

/// Route matcher evaluates incoming request paths against configured rules.
#[derive(Clone)]
pub struct RouteMatcher {
    config: AuthRoutingConfig,
}

impl RouteMatcher {
    /// Create a new RouteMatcher from the auth routing config.
    pub fn new(config: AuthRoutingConfig) -> Self {
        Self { config }
    }

    /// Classify a request path as requiring auth or bypassing it.
    ///
    /// - **STRICT mode**: Default-deny. All paths require auth unless matched by `bypass_rules`.
    /// - **PERMISSIVE mode**: Default-allow. Only paths matching `require_rules` require auth.
    pub fn classify(&self, path: &str) -> RouteDecision {
        match self.config.mode {
            AuthRoutingMode::Strict => {
                if self.matches_any(path, &self.config.bypass_rules) {
                    RouteDecision::BypassAuth
                } else {
                    RouteDecision::RequiresAuth
                }
            }
            AuthRoutingMode::Permissive => {
                if self.matches_any(path, &self.config.require_rules) {
                    RouteDecision::RequiresAuth
                } else {
                    RouteDecision::BypassAuth
                }
            }
        }
    }

    /// Detect the request type from HTTP headers.
    ///
    /// - `Accept: application/json` or `X-Requested-With` → API
    /// - `Accept: text/html` → Page
    /// - Default → Page (browser navigation assumed)
    pub fn detect_request_type(headers: &HeaderMap) -> RequestType {
        // Check X-Requested-With (common for Ajax/XHR)
        if headers.contains_key("x-requested-with") {
            return RequestType::Api;
        }

        // Check Accept header
        if let Some(accept) = headers.get("accept").and_then(|v| v.to_str().ok()) {
            let accept_lower = accept.to_lowercase();
            if accept_lower.contains("application/json") {
                return RequestType::Api;
            }
            if accept_lower.contains("text/html") {
                return RequestType::Page;
            }
        }

        // Default: assume page navigation
        RequestType::Page
    }

    /// Match a path against a list of glob patterns.
    ///
    /// Supports:
    /// - `*` — matches any single path segment
    /// - `**` — matches zero or more path segments (recursive wildcard)
    /// - Exact match
    fn matches_any(&self, path: &str, patterns: &[String]) -> bool {
        patterns.iter().any(|pattern| glob_match(pattern, path))
    }
}

/// Simple glob pattern matcher for URL paths.
///
/// Supports:
/// - `**` matches zero or more path segments (e.g., `/api/**` matches `/api`, `/api/foo`, `/api/foo/bar`)
/// - `*` matches exactly one path segment (e.g., `/api/*/list` matches `/api/v1/list`)
/// - Exact string match
fn glob_match(pattern: &str, path: &str) -> bool {
    let pat = pattern.trim_end_matches('/');
    let pth = path.trim_end_matches('/');

    // Handle empty cases
    if pat.is_empty() && pth.is_empty() {
        return true;
    }

    // Split into segments
    let pat_segs: Vec<&str> = pat.split('/').filter(|s| !s.is_empty()).collect();
    let path_segs: Vec<&str> = pth.split('/').filter(|s| !s.is_empty()).collect();

    glob_match_segments(&pat_segs, &path_segs)
}

fn glob_match_segments(pattern: &[&str], path: &[&str]) -> bool {
    let mut pi = 0; // pattern index
    let mut si = 0; // path (segment) index

    while pi < pattern.len() && si < path.len() {
        match pattern[pi] {
            "**" => {
                // ** at end: matches everything remaining
                if pi == pattern.len() - 1 {
                    return true;
                }
                // Try matching ** against 0, 1, 2, ... segments
                for skip in 0..=(path.len() - si) {
                    if glob_match_segments(&pattern[pi + 1..], &path[si + skip..]) {
                        return true;
                    }
                }
                return false;
            }
            "*" => {
                // * matches exactly one segment
                pi += 1;
                si += 1;
            }
            seg => {
                if seg != path[si] {
                    return false;
                }
                pi += 1;
                si += 1;
            }
        }
    }

    // Handle trailing ** in pattern
    while pi < pattern.len() && pattern[pi] == "**" {
        pi += 1;
    }

    pi == pattern.len() && si == path.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    // ---- Glob matching tests ----

    #[test]
    fn test_glob_exact_match() {
        assert!(glob_match("/health", "/health"));
        assert!(!glob_match("/health", "/healthz"));
        assert!(!glob_match("/health", "/api/health"));
    }

    #[test]
    fn test_glob_double_star_suffix() {
        assert!(glob_match("/api/**", "/api"));
        assert!(glob_match("/api/**", "/api/foo"));
        assert!(glob_match("/api/**", "/api/foo/bar"));
        assert!(glob_match("/api/**", "/api/foo/bar/baz"));
        assert!(!glob_match("/api/**", "/other/foo"));
    }

    #[test]
    fn test_glob_double_star_prefix() {
        assert!(glob_match("/**/health", "/health"));
        assert!(glob_match("/**/health", "/api/health"));
        assert!(glob_match("/**/health", "/api/v1/health"));
        assert!(!glob_match("/**/health", "/api/healthz"));
    }

    #[test]
    fn test_glob_single_star() {
        assert!(glob_match("/api/*/list", "/api/v1/list"));
        assert!(glob_match("/api/*/list", "/api/v2/list"));
        assert!(!glob_match("/api/*/list", "/api/v1/v2/list"));
        assert!(!glob_match("/api/*/list", "/api/list"));
    }

    #[test]
    fn test_glob_trailing_slash_normalization() {
        assert!(glob_match("/api/**", "/api/"));
        assert!(glob_match("/api/", "/api"));
        assert!(glob_match("/api", "/api/"));
    }

    #[test]
    fn test_glob_empty_pattern_and_path() {
        assert!(glob_match("", ""));
        assert!(glob_match("/", "/"));
    }

    // ---- RouteMatcher classification tests ----

    #[test]
    fn test_strict_mode_default_requires_auth() {
        let matcher = RouteMatcher::new(AuthRoutingConfig {
            mode: AuthRoutingMode::Strict,
            bypass_rules: vec!["/health".to_string(), "/static/**".to_string()],
            require_rules: vec![],
        });

        assert_eq!(matcher.classify("/api/data"), RouteDecision::RequiresAuth);
        assert_eq!(matcher.classify("/"), RouteDecision::RequiresAuth);
        assert_eq!(matcher.classify("/invoice"), RouteDecision::RequiresAuth);
    }

    #[test]
    fn test_strict_mode_bypass_matched() {
        let matcher = RouteMatcher::new(AuthRoutingConfig {
            mode: AuthRoutingMode::Strict,
            bypass_rules: vec!["/health".to_string(), "/static/**".to_string()],
            require_rules: vec![],
        });

        assert_eq!(matcher.classify("/health"), RouteDecision::BypassAuth);
        assert_eq!(
            matcher.classify("/static/js/app.js"),
            RouteDecision::BypassAuth
        );
        assert_eq!(
            matcher.classify("/static/css/main.css"),
            RouteDecision::BypassAuth
        );
    }

    #[test]
    fn test_permissive_mode_default_bypass() {
        let matcher = RouteMatcher::new(AuthRoutingConfig {
            mode: AuthRoutingMode::Permissive,
            require_rules: vec!["/api/**".to_string(), "/user/invoice/**".to_string()],
            bypass_rules: vec![],
        });

        assert_eq!(matcher.classify("/"), RouteDecision::BypassAuth);
        assert_eq!(matcher.classify("/home"), RouteDecision::BypassAuth);
        assert_eq!(matcher.classify("/public/page"), RouteDecision::BypassAuth);
    }

    #[test]
    fn test_permissive_mode_require_matched() {
        let matcher = RouteMatcher::new(AuthRoutingConfig {
            mode: AuthRoutingMode::Permissive,
            require_rules: vec!["/api/**".to_string(), "/user/invoice/**".to_string()],
            bypass_rules: vec![],
        });

        assert_eq!(matcher.classify("/api/data"), RouteDecision::RequiresAuth);
        assert_eq!(
            matcher.classify("/api/v1/users"),
            RouteDecision::RequiresAuth
        );
        assert_eq!(
            matcher.classify("/user/invoice/create"),
            RouteDecision::RequiresAuth
        );
    }

    #[test]
    fn test_permissive_empty_rules_bypass_all() {
        let matcher = RouteMatcher::new(AuthRoutingConfig {
            mode: AuthRoutingMode::Permissive,
            require_rules: vec![],
            bypass_rules: vec![],
        });

        assert_eq!(matcher.classify("/anything"), RouteDecision::BypassAuth);
        assert_eq!(matcher.classify("/api/data"), RouteDecision::BypassAuth);
    }

    #[test]
    fn test_strict_empty_bypass_requires_all() {
        let matcher = RouteMatcher::new(AuthRoutingConfig {
            mode: AuthRoutingMode::Strict,
            bypass_rules: vec![],
            require_rules: vec![],
        });

        assert_eq!(matcher.classify("/anything"), RouteDecision::RequiresAuth);
        assert_eq!(matcher.classify("/health"), RouteDecision::RequiresAuth);
    }

    // ---- Request type detection tests ----

    #[test]
    fn test_detect_api_json_accept() {
        let mut headers = HeaderMap::new();
        headers.insert("accept", HeaderValue::from_static("application/json"));
        assert_eq!(
            RouteMatcher::detect_request_type(&headers),
            RequestType::Api
        );
    }

    #[test]
    fn test_detect_api_x_requested_with() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-requested-with",
            HeaderValue::from_static("XMLHttpRequest"),
        );
        assert_eq!(
            RouteMatcher::detect_request_type(&headers),
            RequestType::Api
        );
    }

    #[test]
    fn test_detect_page_html_accept() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "accept",
            HeaderValue::from_static("text/html,application/xhtml+xml"),
        );
        assert_eq!(
            RouteMatcher::detect_request_type(&headers),
            RequestType::Page
        );
    }

    #[test]
    fn test_detect_default_is_page() {
        let headers = HeaderMap::new();
        assert_eq!(
            RouteMatcher::detect_request_type(&headers),
            RequestType::Page
        );
    }

    #[test]
    fn test_detect_api_json_accept_with_charset() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "accept",
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
        assert_eq!(
            RouteMatcher::detect_request_type(&headers),
            RequestType::Api
        );
    }
}
