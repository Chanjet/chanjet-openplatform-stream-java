use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum McpFeature {
    OutputSchema,
}

#[derive(Debug, Clone)]
pub struct VersionRange {
    pub since: Option<String>,
    pub until: Option<String>,
}

impl VersionRange {
    pub fn is_supported(&self, version: &str) -> bool {
        if let Some(since) = &self.since {
            if version < since.as_str() {
                return false;
            }
        }
        if let Some(until) = &self.until {
            if version >= until.as_str() {
                return false;
            }
        }
        true
    }
}

pub struct CapabilityRegistry {
    features: HashMap<McpFeature, VersionRange>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        let mut features = HashMap::new();
        // `outputSchema` and `structuredContent` were formally introduced in the MCP v2025-06-18 specification update
        features.insert(
            McpFeature::OutputSchema,
            VersionRange {
                since: Some("2025-06-18".to_string()),
                until: None,
            },
        );

        Self { features }
    }

    pub fn supports(&self, feature: &McpFeature, version: Option<&str>) -> bool {
        let version = match version {
            Some(v) => v,
            None => return false, // If no protocolVersion is negotiated, assume highly legacy and no modern features supported
        };

        if let Some(range) = self.features.get(feature) {
            range.is_supported(version)
        } else {
            // Feature not registered, default to not supported
            false
        }
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

use std::sync::OnceLock;

pub fn get_global_registry() -> &'static CapabilityRegistry {
    static REGISTRY: OnceLock<CapabilityRegistry> = OnceLock::new();
    REGISTRY.get_or_init(CapabilityRegistry::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_range_since_only() {
        let range = VersionRange {
            since: Some("2024-11-05".to_string()),
            until: None,
        };
        assert!(!range.is_supported("2024-10-01"));
        assert!(range.is_supported("2024-11-05"));
        assert!(range.is_supported("2025-01-01"));
    }

    #[test]
    fn test_version_range_until_only() {
        let range = VersionRange {
            since: None,
            until: Some("2025-01-01".to_string()),
        };
        assert!(range.is_supported("2024-11-05"));
        assert!(!range.is_supported("2025-01-01"));
        assert!(!range.is_supported("2025-02-01"));
    }

    #[test]
    fn test_version_range_since_and_until() {
        let range = VersionRange {
            since: Some("2024-11-05".to_string()),
            until: Some("2025-01-01".to_string()),
        };
        assert!(!range.is_supported("2024-10-01"));
        assert!(range.is_supported("2024-11-05"));
        assert!(range.is_supported("2024-12-31"));
        assert!(!range.is_supported("2025-01-01"));
    }

    #[test]
    fn test_capability_registry() {
        let registry = CapabilityRegistry::new();
        // Legacy clients without version get false
        assert!(!registry.supports(&McpFeature::OutputSchema, None));
        // Older drafts get false
        assert!(!registry.supports(&McpFeature::OutputSchema, Some("2024-10-01")));
        // Original MCP standard release still didn't have outputSchema
        assert!(!registry.supports(&McpFeature::OutputSchema, Some("2024-11-05")));
        // Modern clients get true
        assert!(registry.supports(&McpFeature::OutputSchema, Some("2025-06-18")));
        assert!(registry.supports(&McpFeature::OutputSchema, Some("2026-01-01")));
    }
}
