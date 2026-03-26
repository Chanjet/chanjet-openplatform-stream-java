# Proposal: CLI Command Tree & Integration Implementation

## Why
The final phase of the cjtCli implementation involves assembling all core components into a cohesive command-line interface. This includes implementing the `init`, `auth`, `daemon`, `api`, and `dlq` command trees, ensuring strict adherence to the PRD's structured output and Agent-friendly requirements.

## What Changes
- Implement the root command and global flags (`--profile`, `--format`, `--log-level`).
- Implement `init` command for first-time configuration and credential mounting.
- Implement `auth` command for status checking and credential reset.
- Implement `daemon` command to start the long-lived stream, local proxy, and DLQ.
- Implement `api` command for dynamic API calls (Method + Path).
- Implement `dlq` command for inspecting and retrying failed events.
- Implement `log` command for tailing domain-separated logs.
- Integrate the structured output interceptor and panic recovery into the root command.

## Impact
- **Specs**: Finalizes the user-facing CLI specification.
- **Code**: New `cmd/` package and main entry point.
- **Users**: Provides the final, ready-to-use tool for developers and Agents.
- **Interoperability**: Ensures 100% structured output for downstream Agent consumption.
