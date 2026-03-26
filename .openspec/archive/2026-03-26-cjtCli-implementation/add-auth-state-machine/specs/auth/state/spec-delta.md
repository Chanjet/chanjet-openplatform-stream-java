# Specification Delta: Auth State Machine & Token Management

## ADDED Requirements

### Requirement: Single-Flight Token Synchronization
The cjtCli SHALL ensure that only one authentication request is in flight for the same resource (e.g., refreshing an `accessToken`) even if multiple threads/commands trigger it simultaneously.

#### Scenario: Thundering Herd Mitigation
GIVEN an `accessToken` is expired
WHEN 100 concurrent requests trigger `GetToken`
THEN only ONE network call SHALL be made to the Chanjet platform
AND all 100 requests SHALL receive the SAME new `accessToken` after the call returns.

### Requirement: Proactive AppTicket Fetching
The cjtCli SHALL NOT wait for the next periodic `appTicket` push if it discovers the current ticket is missing or invalid.

#### Scenario: Immediate Ticket Trigger
GIVEN the cjtCli starts or restarts
WHEN it detects no valid `appTicket` in the local `Vault`
THEN it SHALL immediately invoke the "Trigger Active Push" API on the Chanjet platform
AND it SHALL WAIT (with timeout) for the `daemon` mode to receive and store the new ticket before proceeding.

### Requirement: Token Persistence and In-Memory Cache
The cjtCli SHALL maintain an in-memory cache of tokens for speed, but MUST persist them to the `Vault` (Keyring or Seal File) to survive restarts.

### Requirement: Auth Application Mode
The cjtCli SHALL currently only support the `self-built` (自建应用) application mode for v0.1.1.
- `AppKey`, `AppSecret`, and `EncryptCode` are required.
