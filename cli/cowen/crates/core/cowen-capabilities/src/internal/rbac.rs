use cowen_common::jwt::{IpcClaims, IpcRole};

/// Verifies whether the provided claims contain the required permissions and target profile.
/// Returns Ok(()) if authorized, or Err(String) with a detailed forbidden message if not.
pub fn verify_permission(
    claims: Option<&IpcClaims>,
    target_profile: Option<&str>,
    all_scopes: &[&str],
    any_scopes: &[&str],
) -> Result<(), String> {
    if let Some(claims) = claims {
        if claims.role == IpcRole::Plugin {
            if let Some(p) = target_profile {
                if p != claims.sub {
                    return Err(format!(
                        "Forbidden: Plugin '{}' is not authorized to access profile '{}'",
                        claims.sub, p
                    ));
                }
            } else if all_scopes.is_empty() && any_scopes.is_empty() {
                // If there's no target profile AND no specific scope requested, we reject by default for plugins.
                return Err(format!(
                    "Forbidden: Plugin '{}' is not authorized for this action",
                    claims.sub
                ));
            }

            if !claims.scopes.contains(&"*".to_string()) {
                if !all_scopes.is_empty() {
                    let missing: Vec<&str> = all_scopes
                        .iter()
                        .filter(|&s| !claims.scopes.contains(&s.to_string()))
                        .copied()
                        .collect();
                    if !missing.is_empty() {
                        return Err(format!(
                            "Forbidden: Plugin '{}' lacks required permissions {:?}",
                            claims.sub, missing
                        ));
                    }
                }

                if !any_scopes.is_empty() {
                    let has_any = any_scopes
                        .iter()
                        .any(|&s| claims.scopes.contains(&s.to_string()));
                    if !has_any {
                        return Err(format!(
                            "Forbidden: Plugin '{}' lacks any of the permissions {:?}",
                            claims.sub, any_scopes
                        ));
                    }
                }
            }
        }
        Ok(())
    } else {
        // Unauthenticated or not using IPC auth (e.g. standard local usage or unauth).
        // Since RBAC is only strictly applied to Plugin role via IPC tokens,
        // if there are no claims, it's either an error or handled upstream.
        // Currently, we just return Ok(()) for non-IPC calls or rely on upstream token validation.
        // The original `check_rbac` ignored missing claims for non-plugin roles.
        Ok(())
    }
}

/// The Global Policy Matrix.
/// Defines the required scopes for a given (domain, action) pair.
/// Returns (all_scopes, any_scopes).
pub fn get_policy(domain: &str, action: &str) -> (Vec<&'static str>, Vec<&'static str>) {
    match (domain, action) {
        ("native.api.registry", "execute") => (vec!["native.api.registry:execute"], vec![]),
        ("native.api.registry", "search") => (vec!["native.api.registry:search"], vec![]),
        ("native.api.registry", "read") => (vec!["native.api.registry:read"], vec![]),
        ("native.auth", "filter") => (vec![], vec!["native.auth:filter", "native.config:read"]),
        ("native.dlq", "read") => (vec!["native.dlq:read"], vec![]),
        ("native.dlq", "execute") => (vec!["native.dlq:execute"], vec![]),
        ("native.system", "read") => (vec!["native.system:read"], vec![]),
        ("native.system", "execute") => (vec!["native.system:execute"], vec![]),
        ("native.worker", "read") => (vec!["native.worker:read"], vec![]),
        ("native.worker", "execute") => (vec!["native.worker:execute"], vec![]),
        ("native.auth", "read") => (vec!["native.auth:read"], vec![]),
        ("native.auth", "execute") => (vec!["native.auth:execute"], vec![]),
        ("native.config", "read") => (vec!["native.config:read"], vec![]),
        ("native.config", "write") => (vec!["native.config:write"], vec![]),
        ("native.audit", "read") => (vec!["native.audit:read"], vec![]),
        _ => (vec![], vec![]),
    }
}

/// Helper for Wasm Extism host functions to check permissions from manifest strings.
pub fn has_policy_permission(permissions: &[String], domain: &str, action: &str) -> bool {
    let (all_scopes, any_scopes) = get_policy(domain, action);

    // If there are no required scopes, allow
    if all_scopes.is_empty() && any_scopes.is_empty() {
        return true; // Or should we default to false if not found? Let's assume some actions don't require scopes.
                     // Wait, for Wasm, we only call this for actions that DO require scopes.
    }

    if permissions.contains(&"*".to_string()) {
        return true;
    }

    for scope in &all_scopes {
        if !permissions.contains(&scope.to_string()) {
            return false;
        }
    }

    if !any_scopes.is_empty() {
        let has_any = any_scopes
            .iter()
            .any(|s| permissions.contains(&s.to_string()));
        if !has_any {
            return false;
        }
    }

    true
}
