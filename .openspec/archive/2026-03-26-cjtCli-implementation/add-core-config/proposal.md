# Proposal: Config Orchestration Implementation (core/config)

## Why
As per the PRD v0.1.1, `cjtCli` needs a robust configuration management system that supports multi-profile isolation, dynamic loading, and seamless integration with Viper. This is the foundation for all subsequent commands and services.

## What Changes
- Implement `core/config` package using Viper.
- Support multi-profile loading via `--profile` flag or default.
- Support `Watch` mechanism for hot-reloading (specifically for log levels).
- Provide a `Manager` interface to abstract configuration access.
- Ensure all configuration is stored in `~/.cjtCli/` by default.

## Impact
- **Specs**: Defines the configuration structure and profile management rules.
- **Code**: New `core/config` package in `cli/cjtCli`.
- **Users**: Users can manage multiple app configurations (profiles).
- **Security**: No secrets stored in cleartext in the config file (handled by Task 1.2, but the config structure must support it).
