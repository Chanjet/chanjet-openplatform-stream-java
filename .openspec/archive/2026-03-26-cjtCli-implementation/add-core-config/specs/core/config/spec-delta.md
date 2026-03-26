# Specification Delta: Core Configuration Implementation

## ADDED Requirements

### Requirement: Configuration Profile Management
The cjtCli SHALL support multiple configuration profiles to isolate credentials for different applications/environments.

#### Scenario: Default Profile Initialization
GIVEN no `--profile` flag is provided
WHEN the CLI starts
THEN it SHALL load the configuration from `~/.cjtCli/default.yaml`.

#### Scenario: Specific Profile Loading
GIVEN a profile name `shop-A` is provided via `--profile` flag
WHEN the CLI starts
THEN it SHALL load the configuration from `~/.cjtCli/shop-A.yaml`.

### Requirement: Dynamic Configuration Reloading
The cjtCli SHALL monitor the active configuration file for changes during runtime (Daemon mode).

#### Scenario: Log Level Hot-Update
GIVEN the CLI is running in `daemon` mode
WHEN the `log_level` field in the configuration file is updated from `info` to `debug`
THEN the system SHALL automatically reload the new log level without restart.

### Requirement: Configuration Model Schema
The configuration file SHALL adhere to the following minimal schema:
- `app_key` (string)
- `encrypt_code` (string)
- `app_mode` (string, defaults to `self-built`)
- `log_level` (string, one of: `debug`, `info`, `warn`, `error`)
- `auth_url` (string, defaults to `https://open.chanjet.com`)
