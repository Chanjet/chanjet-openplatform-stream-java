use cowen_common::{CowenError, CowenResult};
use serde_json::Value;

pub trait ConfigStrategy: Send + Sync {
    /// Identifies which keys this strategy can handle (e.g. by prefix or exact match).
    fn matches(&self, key: &str) -> bool;

    /// Indicates whether this config belongs to the global `AppConfig` or profile `Config`.
    fn is_global(&self) -> bool;

    /// Indicates whether the given key is read-only and should be locked from manual modification.
    fn is_readonly(&self, _key: &str) -> bool {
        false
    }

    fn handle_get(&self, key: &str, current_json: &Value) -> CowenResult<Value> {
        crate::path_parser::get_by_path(current_json, key)
            .ok_or_else(|| CowenError::Config(format!("Key not found: {}", key)))
    }

    fn handle_set(&self, key: &str, val: &str, current_json: &mut Value) -> CowenResult<()> {
        crate::path_parser::set_by_path(current_json, key, val)
    }

    fn handle_unset(&self, key: &str, current_json: &mut Value) -> CowenResult<()> {
        crate::path_parser::unset_by_path(current_json, key)
    }
}

pub struct GlobalAppConfigStrategy;
impl ConfigStrategy for GlobalAppConfigStrategy {
    fn matches(&self, key: &str) -> bool {
        let global_fields = [
            "monitor_port",
            "openapi_url",
            "stream_url",
            "telemetry_enabled",
        ];
        global_fields.contains(&key)
            || key.starts_with("storage.")
            || key.starts_with("log.")
            || key.starts_with("security.")
            || key.starts_with("search.")
    }

    fn is_global(&self) -> bool {
        true
    }

    fn is_readonly(&self, key: &str) -> bool {
        // Search fields are managed by plugins command, not direct set
        key.starts_with("search.") || key == "version" || key == "exclusive"
    }
}

pub struct ProfileLockedStrategy;
impl ConfigStrategy for ProfileLockedStrategy {
    fn matches(&self, key: &str) -> bool {
        let locked_fields = ["app_key", "app_mode"];
        locked_fields.contains(&key)
    }

    fn is_global(&self) -> bool {
        false
    }

    fn is_readonly(&self, _key: &str) -> bool {
        true
    }

    fn handle_set(&self, key: &str, _val: &str, _current_json: &mut Value) -> CowenResult<()> {
        Err(CowenError::Config(format!(
            "Field '{}' is locked for safety",
            key
        )))
    }

    fn handle_unset(&self, key: &str, _current_json: &mut Value) -> CowenResult<()> {
        Err(CowenError::Config(format!(
            "Field '{}' is mandatory and cannot be unset",
            key
        )))
    }
}

pub struct ProfileDefaultStrategy;
impl ConfigStrategy for ProfileDefaultStrategy {
    fn matches(&self, _key: &str) -> bool {
        // Matches everything else, acts as fallback
        true
    }

    fn is_global(&self) -> bool {
        false
    }

    fn is_readonly(&self, key: &str) -> bool {
        key == "version" || key == "exclusive"
    }
}
