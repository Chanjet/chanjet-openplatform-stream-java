# Cowen v0.2.x Configuration & Vault Snapshot (Verified) {#cowen-v02-snapshot}

> **Snapshot Date**: 2026-04-29
> **Source**: `cli/cowen/src/core/config.rs`, `cli/cowen/src/core/vault.rs`

## 1. Config (.yaml) Structure
Current `Config` struct (v0.2.1):
```rust
pub struct Config {
    pub app_key: String,
    pub openapi_url: String,
    pub stream_url: String,
    pub webhook_target: String,
    pub log: LogConfig,
    pub telemetry_enabled: bool,
    pub ai_enabled: bool,
    pub proxy_port: u16,
    pub proxy_enabled: bool,
    pub app_mode: AuthMode,
}
```

## 2. Vault (.vault) Structure
- **Storage**: Encrypted JSON using AES-GCM.
- **Key Derivation**: Based on machine fingerprint.
- **Data Model**: `HashMap<ProfileName, HashMap<Key, Value>>`.
- **Key Fields**: `app_secret`, `certificate`, `encrypt_key`, `oauth2_token_pair`.

## 3. Directory Layout
- `~/.cowen/current_profile`: Stores active profile name.
- `~/.cowen/<profile>.yaml`: Profile configuration.
- `~/.cowen/cowen.vault`: Encrypted secrets.
- `~/.cowen/logs/`: Log files.

---
*Verified by Master Orchestrator against source code.*
