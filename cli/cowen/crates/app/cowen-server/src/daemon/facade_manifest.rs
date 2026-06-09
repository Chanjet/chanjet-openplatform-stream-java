use std::collections::HashMap;

/// The Global Facade Manifest aggregates and aligns the capability versions 
/// provided by both the Wasm and gRPC facades.
pub struct FacadeManifest;

impl FacadeManifest {
    /// Validates the alignment between Wasm and gRPC capability versions at boot time.
    /// Panics if the contracts diverge, ensuring "Single Source of Truth" governance.
    pub fn get_global_manifest() -> HashMap<&'static str, Vec<&'static str>> {
        let wasm_caps = crate::daemon::wasm_capabilities::registry_supported_versions();
        let grpc_caps = crate::daemon::grpc_capabilities::registry_supported_versions();

        // Check if both doors declare the exact same capabilities and versions.
        if wasm_caps != grpc_caps {
            tracing::error!("Wasm Capabilities: {:?}", wasm_caps);
            tracing::error!("gRPC Capabilities: {:?}", grpc_caps);
            panic!(
                "FATAL: Capability Facade Version Mismatch! \
                The Wasm facade and gRPC facade must support the exact same capability contracts. \
                Please ensure both `wasm_capabilities` and `grpc_capabilities` are aligned."
            );
        }

        wasm_caps
    }

    /// Checks if a plugin's required capabilities are fully satisfied by this host's unified facade manifest using SemVer rules.
    pub fn check_plugin_compatibility(required: &std::collections::HashMap<String, String>) -> anyhow::Result<()> {
        let supported = Self::get_global_manifest();
        
        for (domain, req_ver_str) in required {
            // Ignore Wasm-internal ABI declarations
            if domain == "extism.pdk" {
                continue;
            }

            let req_ver = semver::VersionReq::parse(req_ver_str).map_err(|e| {
                anyhow::anyhow!("Invalid semantic version requirement '{}' for capability '{}': {}", req_ver_str, domain, e)
            })?;

            if let Some(host_versions_strs) = supported.get(domain.as_str()) {
                let mut matched = false;
                for host_ver_str in host_versions_strs {
                    if let Ok(host_ver) = semver::Version::parse(host_ver_str) {
                        if req_ver.matches(&host_ver) {
                            matched = true;
                            break;
                        }
                    }
                }

                if !matched {
                    return Err(anyhow::anyhow!(
                        "Capability version mismatch for '{}': plugin requires '{}', but host provides {:?}",
                        domain, req_ver_str, host_versions_strs
                    ));
                }
            } else {
                return Err(anyhow::anyhow!(
                    "Required capability '{}' is not supported by this host",
                    domain
                ));
            }
        }
        Ok(())
    }
}
