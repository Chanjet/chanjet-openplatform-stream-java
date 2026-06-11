use arc_swap::ArcSwap;
use extism::{Manifest, Plugin, Wasm};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cowen_wasm_facade::{
    native_auth::NativeAuthProvider, native_config::NativeConfigProvider, CapabilityContext,
    HostCapabilityProvider, SysBaseProvider,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPluginManifest {
    pub name: String,
    pub path: String, // Path to the .wasm file
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePolicy {
    pub path_prefix: String,
    pub pre_auth_plugins: Vec<String>,
    pub request_filter_plugins: Vec<String>,
    pub response_filter_plugins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub plugins: Vec<WasmPluginManifest>,
    pub routes: Vec<RoutePolicy>,
}

pub struct WasmPipelineManager {
    plugins: ArcSwap<HashMap<String, Arc<Mutex<Plugin>>>>,
    plugin_manifests: ArcSwap<HashMap<String, cowen_common::plugin::PluginManifest>>,
    lazy_system_plugins: ArcSwap<HashMap<String, std::path::PathBuf>>,
    routes: ArcSwap<Vec<RoutePolicy>>,
    profile: String,
    config: cowen_common::config::Config,
    capabilities: Arc<cowen_capabilities::CapabilityRegistry>,
}

impl WasmPipelineManager {
    pub fn new(
        profile: String,
        config: cowen_common::config::Config,
        capabilities: Arc<cowen_capabilities::CapabilityRegistry>,
    ) -> Self {
        Self {
            plugins: ArcSwap::from_pointee(HashMap::new()),
            plugin_manifests: ArcSwap::from_pointee(HashMap::new()),
            lazy_system_plugins: ArcSwap::from_pointee(HashMap::new()),
            routes: ArcSwap::from_pointee(Vec::new()),
            profile,
            config,
            capabilities,
        }
    }

    /// Load pipeline configuration (Manifest + Routes)
    pub fn load_pipeline(&self, config: PipelineConfig) -> anyhow::Result<()> {
        let mut loaded_plugins = HashMap::new();
        let mut loaded_manifests = HashMap::new();
        let mut lazy_plugins = HashMap::new();

        for plugin_info in &config.plugins {
            // For now, load from filesystem path
            let wasm_bytes = std::fs::read(&plugin_info.path).map_err(|e| {
                anyhow::anyhow!("Failed to read Wasm file {}: {}", plugin_info.path, e)
            })?;

            let wasm = Wasm::data(wasm_bytes);
            let manifest = Manifest::new([wasm]);
            let plugin_manifest = cowen_common::plugin::PluginManifest::load(&plugin_info.name)
                .unwrap_or_else(|_| {
                    cowen_common::plugin::PluginManifest::new_empty(&plugin_info.name)
                });

            if !plugin_manifest.required_capabilities.is_empty() {
                crate::daemon::facade_manifest::FacadeManifest::check_plugin_compatibility(
                    &plugin_manifest.required_capabilities,
                )
                .map_err(|e| {
                    anyhow::anyhow!("Plugin {} capability check failed: {}", plugin_info.name, e)
                })?;
            }

            let host_functions = self.create_host_functions(
                &plugin_manifest.requested_permissions,
                &plugin_manifest.required_capabilities,
            );
            let plugin = Plugin::new(&manifest, host_functions, true)?;
            loaded_plugins.insert(plugin_info.name.clone(), Arc::new(Mutex::new(plugin)));
            loaded_manifests.insert(plugin_info.name.clone(), plugin_manifest);
        }

        self.load_system_plugins(
            &mut loaded_plugins,
            &mut loaded_manifests,
            &mut lazy_plugins,
        );

        let total_loaded = loaded_plugins.len();
        self.plugins.store(Arc::new(loaded_plugins));
        self.plugin_manifests.store(Arc::new(loaded_manifests));
        self.lazy_system_plugins.store(Arc::new(lazy_plugins));
        self.routes.store(Arc::new(config.routes));

        tracing::info!(
            "Wasm pipeline reloaded successfully (Total plugins loaded: {})",
            total_loaded
        );
        Ok(())
    }

    fn get_system_plugin_search_paths() -> Vec<std::path::PathBuf> {
        let mut paths = vec![cowen_common::config::get_app_dir().join("system_plugins")];
        if cfg!(unix) {
            paths.push(std::path::PathBuf::from(
                "/usr/local/share/cowen/system_plugins",
            ));
        } else if cfg!(windows) {
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(parent) = exe_path.parent() {
                    paths.push(parent.join("system_plugins"));
                }
            }
        }
        paths
    }

    fn load_system_plugin_manifest(
        name: &str,
        path: &std::path::Path,
    ) -> cowen_common::plugin::PluginManifest {
        let bundle_path = path.with_extension("bundle");
        if bundle_path.exists() {
            cowen_common::plugin::PluginManifest::load_from_bundle(name, &bundle_path)
                .unwrap_or_else(|_| cowen_common::plugin::PluginManifest::new_empty(name))
        } else {
            let json_path = path.with_extension("json");
            cowen_common::plugin::PluginManifest::load_from_json(name, &json_path)
                .unwrap_or_else(|_| cowen_common::plugin::PluginManifest::new_empty(name))
        }
    }

    fn load_system_plugins(
        &self,
        loaded_plugins: &mut HashMap<String, Arc<Mutex<Plugin>>>,
        loaded_manifests: &mut HashMap<String, cowen_common::plugin::PluginManifest>,
        lazy_plugins: &mut HashMap<String, std::path::PathBuf>,
    ) {
        let paths_to_try = Self::get_system_plugin_search_paths();

        for base_dir in paths_to_try {
            if !base_dir.exists() {
                continue;
            }

            if let Ok(entries) = std::fs::read_dir(&base_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                        let name = path.file_stem().unwrap().to_string_lossy().to_string();

                        if loaded_plugins.contains_key(&name) || lazy_plugins.contains_key(&name) {
                            continue; // Avoid overriding if loaded previously or via pipeline.yaml
                        }

                        if let Err(e) = cowen_infra::pki::verify_plugin_bundle(&path) {
                            tracing::error!(
                                "System Wasm plugin {} failed signature verification: {}",
                                name,
                                e
                            );
                            continue;
                        }

                        let plugin_manifest = Self::load_system_plugin_manifest(&name, &path);

                        if !plugin_manifest.required_capabilities.is_empty() {
                            if let Err(e) = crate::daemon::facade_manifest::FacadeManifest::check_plugin_compatibility(&plugin_manifest.required_capabilities) {
                                tracing::error!("Local plugin {} failed capability check: {}", name, e);
                                continue;
                            }
                        }

                        lazy_plugins.insert(name.clone(), path.clone());
                        loaded_manifests.insert(name.clone(), plugin_manifest);
                        tracing::debug!("Discovered system Wasm plugin for lazy loading: {}", name);
                    }
                }
            }
        }
    }

    /// Start watching the given directory for changes to pipeline.yaml or .wasm files
    pub fn start_watch(self: Arc<Self>, plugin_dir: std::path::PathBuf) -> anyhow::Result<()> {
        // Trigger an initial load immediately
        self.load_pipeline_from_file(&plugin_dir);

        use notify::{RecursiveMode, Watcher};
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);

        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            })?;

        watcher.watch(&plugin_dir, RecursiveMode::Recursive)?;

        tokio::spawn(async move {
            let _watcher = watcher; // Keep it alive
            tracing::info!(
                "Watching Wasm plugin directory for changes: {:?}",
                plugin_dir
            );

            while let Some(event) = rx.recv().await {
                if event.kind.is_modify() || event.kind.is_create() {
                    let should_reload = event.paths.iter().any(|p| {
                        if let Some(ext) = p.extension() {
                            ext == "yaml" || ext == "yml" || ext == "wasm"
                        } else {
                            false
                        }
                    });

                    if should_reload {
                        tracing::info!("Plugin directory changed, reloading pipeline...");
                        self.load_pipeline_from_file(&plugin_dir);
                    }
                }
            }
        });

        Ok(())
    }

    fn load_pipeline_from_file(&self, plugin_dir: &std::path::Path) {
        let pipeline_yaml = plugin_dir.join("pipeline.yaml");
        if pipeline_yaml.exists() {
            if let Ok(yaml_str) = std::fs::read_to_string(&pipeline_yaml) {
                if let Ok(mut config) = serde_yaml::from_str::<PipelineConfig>(&yaml_str) {
                    // Make paths absolute based on plugin_dir if they are relative
                    for plugin in &mut config.plugins {
                        let path = std::path::Path::new(&plugin.path);
                        if !path.is_absolute() {
                            plugin.path = plugin_dir.join(path).to_string_lossy().to_string();
                        }
                    }

                    if let Err(e) = self.load_pipeline(config) {
                        tracing::error!("Failed to reload pipeline: {}", e);
                    }
                } else {
                    tracing::error!("Failed to parse pipeline.yaml");
                }
            }
        }
    }

    fn create_host_functions(
        &self,
        scopes: &[String],
        required_capabilities: &HashMap<String, String>,
    ) -> Vec<extism::Function> {
        let mut funcs = vec![];

        let context = CapabilityContext {
            profile: self.profile.clone(),
            config: self.config.clone(),
            capabilities: self.capabilities.clone(),
        };

        // Initialize Providers Registry
        let providers: Vec<Box<dyn HostCapabilityProvider>> = vec![
            Box::new(SysBaseProvider),
            Box::new(NativeConfigProvider),
            Box::new(NativeAuthProvider),
        ];

        // We run all providers. Each provider checks if the required permissions/domains
        // match what it provides, and looks up the version from `required_capabilities`.
        for provider in providers {
            let domain = provider.domain();

            let req_version = required_capabilities.get(domain).map(|s| s.as_str());

            // If it's sys.base, we always provide it using the highest supported version for compatibility if not explicitly requested
            let version_to_use = if let Some(v) = req_version {
                v
            } else if domain == "sys.base" {
                // Default to the first supported version
                provider
                    .supported_versions()
                    .first()
                    .copied()
                    .unwrap_or("1.0.0")
            } else {
                // Capability not explicitly requested, do not mount its host functions
                continue;
            };

            match provider.create_functions(version_to_use, scopes, &context) {
                Ok(mut provider_funcs) => {
                    funcs.append(&mut provider_funcs);
                }
                Err(e) => {
                    // Fail hard if we fail to mount requested host functions due to version mismatch or other error
                    tracing::error!("Failed to create host functions for {}: {}", domain, e);
                    // Depending on policy, we might want to panic or return empty. For now we just skip but log error.
                    // Returning here would require changing `create_host_functions` to return Result.
                }
            }
        }

        funcs
    }

    /// Compatibility with tests that just load a single wasm
    #[cfg(test)]
    pub fn load_single_wasm_for_test(
        &self,
        name: &str,
        wasm_bytes: &[u8],
        scopes: &[String],
    ) -> anyhow::Result<()> {
        let wasm = Wasm::data(wasm_bytes.to_vec());
        let manifest = Manifest::new([wasm]);
        let empty_caps = HashMap::new();
        let host_functions = self.create_host_functions(scopes, &empty_caps);
        let plugin = Plugin::new(&manifest, host_functions, true)?;

        let mut plugins = (**self.plugins.load()).clone();
        plugins.insert(name.to_string(), Arc::new(Mutex::new(plugin)));
        self.plugins.store(Arc::new(plugins));

        let mut manifests = (**self.plugin_manifests.load()).clone();
        manifests.insert(
            name.to_string(),
            cowen_common::plugin::PluginManifest {
                name: name.to_string(),
                requested_permissions: scopes.to_vec(),
                required_capabilities: HashMap::new(),
                allowed_commands: std::collections::HashSet::new(),
                wasm_interceptors: vec![cowen_common::plugin::WasmInterceptorContribution {
                    name: "auth_interceptor".to_string(),
                    app_modes: vec!["self-built".to_string(), "oauth2".to_string()],
                    priority: 100,
                }],
            },
        );
        self.plugin_manifests.store(Arc::new(manifests));

        // Setup a catch-all route for test
        let mut routes = (**self.routes.load()).clone();
        if routes.is_empty() {
            routes.push(RoutePolicy {
                path_prefix: "/".to_string(),
                pre_auth_plugins: vec![name.to_string()],
                request_filter_plugins: vec![name.to_string()],
                response_filter_plugins: vec![name.to_string()],
            });
            self.routes.store(Arc::new(routes));
        }
        Ok(())
    }

    fn find_route(&self, uri: &str) -> Option<RoutePolicy> {
        let routes = self.routes.load();
        // find longest prefix match
        routes
            .iter()
            .filter(|r| uri.starts_with(&r.path_prefix))
            .max_by_key(|r| r.path_prefix.len())
            .cloned()
    }

    fn get_or_load_plugin(&self, target_plugin_name: &str) -> Option<Arc<Mutex<Plugin>>> {
        if let Some(plugin) = self.plugins.load().get(target_plugin_name) {
            return Some(plugin.clone());
        }

        let lazy_paths = self.lazy_system_plugins.load();
        if let Some(path) = lazy_paths.get(target_plugin_name) {
            let manifests = self.plugin_manifests.load();
            if let Some(plugin_manifest) = manifests.get(target_plugin_name) {
                if let Ok(wasm_bytes) = std::fs::read(path) {
                    let wasm = Wasm::data(wasm_bytes);
                    let manifest = Manifest::new([wasm]);
                    let host_functions = self.create_host_functions(
                        &plugin_manifest.requested_permissions,
                        &plugin_manifest.required_capabilities,
                    );
                    if let Ok(plugin) = Plugin::new(&manifest, host_functions, true) {
                        let plugin_arc = Arc::new(Mutex::new(plugin));
                        self.plugins.rcu(|old| {
                            if old.contains_key(target_plugin_name) {
                                return old.clone();
                            }
                            let mut new_map = HashMap::clone(old);
                            new_map.insert(target_plugin_name.to_string(), plugin_arc.clone());
                            Arc::new(new_map)
                        });
                        tracing::info!("Lazy loaded system plugin: {}", target_plugin_name);
                        return Some(self.plugins.load().get(target_plugin_name).unwrap().clone());
                    } else {
                        tracing::error!(
                            "Failed to instantiate lazy system plugin: {}",
                            target_plugin_name
                        );
                    }
                }
            }
        }
        None
    }

    fn find_matched_plugins(
        &self,
        interceptor_name: &str,
        app_mode_str: &str,
    ) -> Vec<(String, i32)> {
        let manifests_map = self.plugin_manifests.load();
        let mut matched_plugins = Vec::new();

        for (actual_name, manifest) in manifests_map.iter() {
            if let Some(interceptor) = manifest
                .wasm_interceptors
                .iter()
                .find(|i| i.name == interceptor_name)
            {
                let has_match = interceptor.app_modes.is_empty()
                    || interceptor
                        .app_modes
                        .iter()
                        .any(|m| m == app_mode_str || m == &app_mode_str.replace("-", "_"));

                if has_match {
                    matched_plugins.push((actual_name.clone(), interceptor.priority));
                }
            } else if actual_name == interceptor_name {
                // Fallback: direct exact name match
                matched_plugins.push((actual_name.clone(), 0));
            }
        }

        matched_plugins.sort_by(|a, b| b.1.cmp(&a.1));
        matched_plugins
    }

    pub fn filter_headers(
        &self,
        uri: &str,
        method: &str,
        has_body: bool,
        mut headers: HashMap<String, String>,
    ) -> HashMap<String, String> {
        let route = match self.find_route(uri) {
            Some(r) => r,
            None => return headers,
        };

        let app_mode_str = match self.config.app_mode {
            cowen_common::models::AuthMode::Oauth2 => "oauth2",
            cowen_common::models::AuthMode::SelfBuilt => "self-built",
            cowen_common::models::AuthMode::StoreApp => "store-app",
        };

        for interceptor_name in route.request_filter_plugins.clone() {
            let matched_plugins = self.find_matched_plugins(&interceptor_name, app_mode_str);

            for (target_plugin_name, _) in matched_plugins {
                if let Some(plugin_mutex) = self.get_or_load_plugin(&target_plugin_name) {
                    let mut plugin = match plugin_mutex.lock() {
                        Ok(p) => p,
                        Err(_) => continue,
                    };

                    let req = serde_json::json!({
                        "path": uri,
                        "method": method,
                        "has_body": has_body,
                        "headers": headers
                    });
                    let req_bytes = serde_json::to_vec(&req).unwrap_or_default();

                    match plugin.call::<&[u8], &[u8]>("auth_interceptor", &req_bytes) {
                        Ok(res) => {
                            if let Ok(new_headers) =
                                serde_json::from_slice::<HashMap<String, String>>(res)
                            {
                                headers = new_headers;
                            }
                        }
                        Err(_) => {
                            // fallback to filter_headers if auth_interceptor is missing for backward compatibility
                            match plugin.call::<&[u8], &[u8]>("filter_headers", &req_bytes) {
                                Ok(res) => {
                                    if let Ok(new_headers) =
                                        serde_json::from_slice::<HashMap<String, String>>(res)
                                    {
                                        headers = new_headers;
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Wasm plugin {} failed to execute interceptor hooks: {}",
                                        target_plugin_name,
                                        e
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        headers
    }

    pub fn filter_request_body(&self, uri: &str, method: &str, mut body: Vec<u8>) -> Vec<u8> {
        let route = match self.find_route(uri) {
            Some(r) => r,
            None => return body,
        };

        for plugin_name in route.request_filter_plugins.clone() {
            if let Some(plugin_mutex) = self.get_or_load_plugin(&plugin_name) {
                let mut plugin = match plugin_mutex.lock() {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                let req = serde_json::json!({
                    "method": method,
                    "uri": uri,
                    "body": body, // Send as byte array
                });
                let req_bytes = serde_json::to_vec(&req).unwrap_or_default();

                match plugin.call::<&[u8], &[u8]>("filter_request_body", &req_bytes) {
                    Ok(res) => {
                        body = res.to_vec();
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Wasm plugin {} failed to execute filter_request_body hook: {}",
                            plugin_name,
                            e
                        );
                    }
                }
            }
        }
        body
    }

    pub fn filter_response_body(
        &self,
        uri: &str,
        method: &str,
        status: u16,
        mut body: Vec<u8>,
    ) -> Vec<u8> {
        let route = match self.find_route(uri) {
            Some(r) => r,
            None => return body,
        };

        let app_mode_str = match self.config.app_mode {
            cowen_common::models::AuthMode::Oauth2 => "oauth2",
            cowen_common::models::AuthMode::SelfBuilt => "self-built",
            cowen_common::models::AuthMode::StoreApp => "store-app",
        };

        for interceptor_name in route.response_filter_plugins.clone() {
            let matched_plugins = self.find_matched_plugins(&interceptor_name, app_mode_str);

            for (target_plugin_name, _) in matched_plugins {
                if let Some(plugin_mutex) = self.get_or_load_plugin(&target_plugin_name) {
                    let mut plugin = match plugin_mutex.lock() {
                        Ok(p) => p,
                        Err(_) => continue,
                    };

                    let req = serde_json::json!({
                        "method": method,
                        "uri": uri,
                        "status": status,
                        "body": body, // Send as byte array
                    });
                    let req_bytes = serde_json::to_vec(&req).unwrap_or_default();

                    match plugin.call::<&[u8], &[u8]>("filter_response_body", &req_bytes) {
                        Ok(res) => {
                            body = res.to_vec();
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Wasm plugin {} failed to execute filter_response_body hook: {}",
                                target_plugin_name,
                                e
                            );
                        }
                    }
                }
            }
        }
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_wasm_pipeline_full() {
        let temp_dir = std::env::temp_dir().join("cowen_test_vault");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let store =
            std::sync::Arc::new(cowen_store::FileStore::new(temp_dir.clone(), None).unwrap());
        let vault = std::sync::Arc::new(cowen_store::StoreVault::new(store.clone(), store.clone()));

        let mut config = cowen_common::config::Config::default_with_profile("test_profile");
        config.app_key = "my-test-app".to_string();
        config.app_mode = cowen_common::models::AuthMode::SelfBuilt;

        let config_manager = cowen_config::ConfigManager::new_with_dir(temp_dir.clone()).unwrap();
        let caps = std::sync::Arc::new(cowen_capabilities::CapabilityRegistry::new(
            std::sync::Arc::new(cowen_common::daemon::DummyDaemonService),
            vault.clone(),
            config_manager,
            0,
            cowen_wasm_facade::registry_supported_versions()
                .into_iter()
                .map(|(k, v)| (k.to_string(), v[0].to_string()))
                .collect(),
        ));
        let manager = WasmPipelineManager::new("test_profile".to_string(), config, caps);

        let wasm_path =
            "../../../target/wasm32-unknown-unknown/debug/cowen_wasm_auth_selfbuilt.wasm";
        let wasm_bytes = std::fs::read(wasm_path);

        // Skip test if plugin wasn't built
        if wasm_bytes.is_err() {
            println!("Skipping test, Wasm plugin not found at {}", wasm_path);
            return;
        }
        let wasm_bytes = wasm_bytes.unwrap();

        // Load the plugin matching the smart dispatch name
        manager
            .load_single_wasm_for_test(
                "cowen-wasm-auth-selfbuilt",
                &wasm_bytes,
                &["native.config:read".to_string()],
            )
            .unwrap();

        use cowen_common::domain::TokenDomain;
        vault
            .save_access_token(
                "test_profile",
                cowen_common::models::Token {
                    value: "mocked_access_token".to_string(),
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(2),
                    created_at: chrono::Utc::now(),
                },
            )
            .await
            .unwrap();

        vault
            .save_app_access_token(
                "my-test-app",
                cowen_common::models::Token {
                    value: "mocked_access_token".to_string(),
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(2),
                    created_at: chrono::Utc::now(),
                },
            )
            .await
            .unwrap();

        // Emulate the route config naming the generic 'auth_interceptor'
        let route = RoutePolicy {
            path_prefix: "/api".to_string(),
            pre_auth_plugins: vec![],
            request_filter_plugins: vec!["auth_interceptor".to_string()],
            response_filter_plugins: vec![],
        };
        manager.routes.store(std::sync::Arc::new(vec![route]));

        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("Host".to_string(), "localhost".to_string());
        headers.insert("x-app-key".to_string(), "my-test-app".to_string());

        let new_headers = manager.filter_headers("/api", "GET", false, headers);

        assert_eq!(
            new_headers.get("appKey").map(|s| s.as_str()),
            Some("my-test-app")
        );
        assert_eq!(
            new_headers.get("openToken").map(|s| s.as_str()),
            Some("mocked_access_token")
        );

        let body = b"{\"phone\":\"13812345678\"}".to_vec();
        let new_body = manager.filter_request_body("/api/sensitive", "POST", body.clone());
        assert_eq!(body, new_body); // Transparent now

        // Test filter_response_body
        let resp_body = b"{\"id_card\":\"110105199001011234\"}".to_vec();
        let new_resp_body =
            manager.filter_response_body("/api/sensitive", "GET", 200, resp_body.clone());
        assert_eq!(resp_body, new_resp_body); // Transparent now
    }
}
