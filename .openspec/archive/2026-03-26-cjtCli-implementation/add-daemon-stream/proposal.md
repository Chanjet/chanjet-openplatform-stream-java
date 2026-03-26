# Proposal: Daemon Stream & Long-lived Connection Implementation

## Why
PRD v0.1.1 requires `cjtCli` to run as a daemon, maintaining a long-lived connection with the Connector Server. This connection is used to receive real-time events (webhooks) and `appTicket` updates. It must support robust automatic reconnection and heartbeat management.

## What Changes
- Implement `internal/daemon/stream` package.
- Integrate `nhooyr.io/websocket` for high-performance WebSocket communication.
- Implement `Dialer` with exponential backoff and connection state management.
- Implement `StreamHandler` to process incoming frames (EventFrame, AppTicket).
- Support HMAC-SHA256 signature for the WebSocket handshake as required by v0.1.0 security specs.
- Integrate with `TokenPool` to store received `appTicket` updates.

## Impact
- **Specs**: Defines the streaming connection lifecycle and message handling rules.
- **Code**: New `internal/daemon/stream` package in `cli/cjtCli`.
- **Availability**: Enables real-time event delivery and automated ticket management.
- **Reliability**: Self-healing connection through automated reconnection logic.
