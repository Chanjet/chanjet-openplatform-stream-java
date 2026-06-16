use cowen_common::CowenError;

pub fn parse_port_from_bind_addr(addr: &str) -> Option<u16> {
    addr.split(':').next_back()?.parse::<u16>().ok()
}

pub async fn validate_port_conflicts(
    cfg_mgr: &cowen_config::ConfigManager,
    current_profile: &str,
    new_proxy_port: u16,
    new_bind_address: Option<&str>,
) -> Result<(), CowenError> {
    let profiles = cfg_mgr
        .list_profiles()
        .await
        .map_err(|e| CowenError::config(e.to_string()))?;
    for p in profiles {
        if p == current_profile {
            continue;
        }
        if let Ok(other_config) = cfg_mgr.load(&p).await {
            if new_proxy_port > 0 && other_config.proxy_port == new_proxy_port {
                return Err(CowenError::config(format!(
                    "Port conflict: proxy_port {} is already used by profile '{}'",
                    new_proxy_port, p
                )));
            }
            let other_gateway_port = other_config
                .gateway
                .as_ref()
                .and_then(|g| parse_port_from_bind_addr(&g.bind_address));
            let new_gateway_port = new_bind_address.and_then(parse_port_from_bind_addr);
            if let Some(new_gp) = new_gateway_port {
                if new_gp > 0 {
                    if let Some(other_gp) = other_gateway_port {
                        if other_gp == new_gp {
                            return Err(CowenError::config(format!(
                                "Port conflict: gateway bind_address port {} conflicts with profile '{}'",
                                new_gp, p
                            )));
                        }
                    }
                    if other_config.proxy_port == new_gp {
                        return Err(CowenError::config(format!(
                            "Port conflict: gateway bind_address port {} conflicts with proxy_port of profile '{}'",
                            new_gp, p
                        )));
                    }
                }
            }
            if new_proxy_port > 0 {
                if let Some(other_gp) = other_gateway_port {
                    if other_gp == new_proxy_port {
                        return Err(CowenError::config(format!(
                            "Port conflict: proxy_port {} conflicts with gateway bind_address port of profile '{}'",
                            new_proxy_port, p
                        )));
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn deep_merge(target: &mut serde_json::Value, source: &serde_json::Value) {
    match (target, source) {
        (serde_json::Value::Object(target_map), serde_json::Value::Object(source_map)) => {
            for (key, val) in source_map {
                if val.is_null() {
                    target_map.remove(key);
                } else {
                    deep_merge(
                        target_map
                            .entry(key.clone())
                            .or_insert(serde_json::Value::Null),
                        val,
                    );
                }
            }
        }
        (target, source) => {
            *target = source.clone();
        }
    }
}

fn merge_storage(
    app_config: &mut cowen_common::config::AppConfig,
    val: &serde_json::Value,
    changed: &mut bool,
) {
    if let Some(storage_val) = val.get("storage") {
        if let Ok(storage_cfg) =
            serde_json::from_value::<cowen_common::config::StorageConfig>(storage_val.clone())
        {
            if app_config.storage != storage_cfg {
                app_config.storage = storage_cfg;
                *changed = true;
            }
        }
    }
}

fn merge_log(
    app_config: &mut cowen_common::config::AppConfig,
    val: &serde_json::Value,
    changed: &mut bool,
) {
    if let Some(log_val) = val.get("log") {
        if let Ok(log_cfg) =
            serde_json::from_value::<cowen_common::config::LogConfig>(log_val.clone())
        {
            if app_config.log != log_cfg {
                app_config.log = log_cfg;
                *changed = true;
            }
        }
    }
}

fn merge_other_fields(
    app_config: &mut cowen_common::config::AppConfig,
    val: &serde_json::Value,
    changed: &mut bool,
) {
    if let Some(monitor_port_val) = val.get("monitor_port") {
        if let Some(mp) = monitor_port_val.as_u64() {
            let mp_u16 = mp as u16;
            if app_config.monitor_port != mp_u16 {
                app_config.monitor_port = mp_u16;
                *changed = true;
            }
        }
    }
    if let Some(openapi_url_val) = val.get("openapi_url") {
        if let Some(url) = openapi_url_val.as_str() {
            if app_config.openapi_url != url {
                app_config.openapi_url = url.to_string();
                *changed = true;
            }
        }
    }
    if let Some(stream_url_val) = val.get("stream_url") {
        if let Some(url) = stream_url_val.as_str() {
            if app_config.stream_url != url {
                app_config.stream_url = url.to_string();
                *changed = true;
            }
        }
    }
}

pub async fn merge_and_save_global_config(
    cfg_mgr: &cowen_config::ConfigManager,
    json_val: &Option<serde_json::Value>,
    req_openapi_url: Option<&str>,
    req_stream_url: Option<&str>,
) -> Result<cowen_common::config::AppConfig, CowenError> {
    let mut app_config = cfg_mgr
        .load_app_config()
        .await
        .map_err(|e| CowenError::config(format!("Failed to load global config: {}", e)))?;
    let mut changed = false;

    if let Some(ref val) = json_val {
        merge_storage(&mut app_config, val, &mut changed);
        merge_log(&mut app_config, val, &mut changed);
        merge_other_fields(&mut app_config, val, &mut changed);
    }

    if let Some(url) = req_openapi_url {
        if app_config.openapi_url != url {
            app_config.openapi_url = url.to_string();
            changed = true;
        }
    }
    if let Some(url) = req_stream_url {
        if app_config.stream_url != url {
            app_config.stream_url = url.to_string();
            changed = true;
        }
    }

    if changed {
        cfg_mgr
            .save_app_config(&app_config)
            .await
            .map_err(|e| CowenError::config(format!("Failed to save global config: {}", e)))?;
    }
    Ok(app_config)
}
