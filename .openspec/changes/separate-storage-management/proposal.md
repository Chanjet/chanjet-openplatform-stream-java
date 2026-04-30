# Proposal: Separate Storage Management from Initialization

## Why
Currently, storage configuration (store type, database URL, cache type, cache URL) is handled as part of the `cowen init` command. This creates several issues:
1. **Conceptual Misalignment**: Storage settings are global to the application instance (shared across all profiles), while `init` is primarily focused on profile-specific setup (app keys, secrets, URLs).
2. **Confusing UX**: Users might think they can set different storage backends for different profiles, which is explicitly forbidden by `RULE_STORAGE_MUTEX`.
3. **Redundancy**: To change storage settings, users currently have to run `init` even if they don't want to create or reset a profile.

Separating storage into a dedicated `store` command makes the lifecycle clearer: configure the environment's storage once, then initialize profiles as needed.

## What Changes
1.  **New Command: `cowen store`**:
    - `cowen store set`: Configures the global storage backend and cache.
    - `cowen store status`: Shows current storage configuration and health status.
2.  **Modify Command: `cowen init`**:
    - Remove `--store`, `--db-url`, `--cache`, and `--cache-url` flags.
    - `init` will now strictly use the storage backend already configured in `app.yaml`.
3.  **Refactor Implementation**:
    - Move storage configuration logic from `cmd/init.rs` to a new `cmd/store.rs`.
    - Update `main.rs` to include the new command and subcommands.

## Impact
- **Breaking Change**: The `init` command will no longer accept storage-related flags. Automation scripts using these flags will need to be updated.
- **User Experience**: Improved clarity on the global nature of storage settings.
- **Documentation**: PRD, help text, and user guides must be updated to reflect the new command structure.
- **Health Check**: Implements `[Feature-10] 存储状态自检 (Health Check)` more formally within the `store status` command.
