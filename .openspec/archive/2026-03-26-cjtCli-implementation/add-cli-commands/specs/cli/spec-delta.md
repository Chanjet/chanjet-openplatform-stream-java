# Specification Delta: CLI Command Tree & Integration

## ADDED Requirements

### Requirement: Global Command Parameters
The cjtCli SHALL support the following global parameters for all commands:
- `--profile <name>`: Specify the configuration profile (defaults to `default`).
- `--format <json|yaml|text>`: Specify the output format (defaults to `text`).
- `--log-level <debug|info|warn|error>`: Override the log level.

### Requirement: Initialization Command (`init`)
The cjtCli SHALL provide an `init` command to bootstrap the application configuration.
- Must support non-interactive mode via `--app-key`, `--app-secret`, `--encrypt-code`.

### Requirement: Daemon Life Cycle (`daemon`)
The cjtCli SHALL provide a `daemon` command tree to manage background services.
- `daemon start`: Starts the stream, proxy, and forwarder.
- `daemon stop`: Gracefully shuts down services.

### Requirement: Dynamic API Client (`api`)
The cjtCli SHALL provide a generic API client that uses the `{METHOD} {PATH}` paradigm.
- Automatically handles `AppAccessToken` injection and signing.
- Returns the platform response in the requested `--format`.

### Requirement: DLQ Management (`dlq`)
The cjtCli SHALL provide a `dlq` command tree to inspect and manage the Dead-Letter Queue.
- `dlq list`: Shows failed events and reasons.
- `dlq retry <id>`: Manually retries a failed event delivery.
