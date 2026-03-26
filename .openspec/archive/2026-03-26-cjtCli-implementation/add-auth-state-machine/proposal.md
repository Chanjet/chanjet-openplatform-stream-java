# Proposal: Auth State Machine & Token Management Implementation

## Why
PRD v0.1.1 mandates a high-reliability auth state machine. We need to handle `appTicket` lifecycle, `accessToken` refreshes, and prevent "thundering herd" (multiple concurrent refreshes) using a single-flight barrier. This is critical for both the `api` command and the `daemon` mode.

## What Changes
- Implement `internal/auth` package for authentication logic.
- Implement `Barrier` (Single-Flight) to synchronize token refresh operations.
- Implement `TokenPool` to manage `accessToken` and `appTicket` in memory with persistence to `Vault`.
- Implement `TicketRequester` to proactively request new `appTicket` from Chanjet platform when missing or expired (PRD: "立刻触发发生").
- Support both `self-built` (Self-Built App) and future extensibility.

## Impact
- **Specs**: Defines the token lifecycle, retry policies, and atomic refresh rules.
- **Code**: New `internal/auth` package in `cli/cjtCli`.
- **Performance**: Prevents redundant network calls during authentication.
- **Availability**: Ensures the tool always has a valid `appTicket` before declaring `Ready`.
