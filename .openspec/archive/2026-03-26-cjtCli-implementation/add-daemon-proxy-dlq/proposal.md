# Proposal: Local Proxy & Dead-Letter Queue (DLQ) Implementation

## Why
PRD v0.1.1 requires `cjtCli` to act as a local gateway, forwarding events from the stream to local webhook targets and providing a local proxy for outgoing API calls. It also mandates a robust Dead-Letter Queue (DLQ) using a pure Go SQLite implementation to ensure no events are lost when the target is offline.

## What Changes
- Implement `internal/daemon/proxy` package for HTTP event forwarding and local API proxying.
- Implement `internal/daemon/dlq` package for persistent event storage.
- Integrate `modernc.org/sqlite` for a zero-dependency local database.
- Implement a three-stage retry mechanism: Immediate -> Exponential Backoff -> DLQ.
- Provide a mechanism to manually retry events from the DLQ.
- Ensure the local proxy is restricted to `127.0.0.1`.

## Impact
- **Specs**: Defines the event delivery lifecycle and the dead-letter persistence rules.
- **Code**: New `internal/daemon/proxy` and `internal/daemon/dlq` packages.
- **Reliability**: Guarantees event persistence and provides a fallback for offline targets.
- **Security**: Local-only proxy ensures no unauthorized access from the network.
