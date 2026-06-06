sed -i '' 's/if supports_webhooks && !is_fresh {/if supports_webhooks \&\& !is_fresh \&\& conn_state == "Connected" {/' crates/core/cowen-common/src/status.rs
