# Specification Delta: Daemon Stream & WebSocket Connection

## ADDED Requirements

### Requirement: Secure WebSocket Handshake
The cjtCli daemon SHALL perform a secure, two-step handshake with the Stream Gateway.

#### Scenario: Handshake Signature Calculation
GIVEN a `nonce` is received from the `/challenge` endpoint
WHEN calculating the WebSocket connection URL
THEN the cjtCli SHALL include `sign = HMAC_SHA256(app_key + "&" + nonce, app_secret).hex().lower()`.

### Requirement: Self-Healing Connection
The cjtCli daemon SHALL automatically recover from connection drops using an exponential backoff strategy.

#### Scenario: Exponential Backoff Reconnect
GIVEN the WebSocket connection is lost
WHEN the `daemon` is still running
THEN the cjtCli SHALL attempt to reconnect with a delay of `2^n` seconds (where `n` is retry count), capped at 60 seconds.

### Requirement: Real-time Event Handling
The cjtCli daemon SHALL handle incoming message frames from the Gateway.

#### Scenario: AppTicket Auto-Update
GIVEN a message with `msg_type: "APP_TICKET"` is received
WHEN the message is successfully parsed
THEN the cjtCli SHALL update the `TokenPool` with the new ticket value immediately.

#### Scenario: Webhook Event Routing
GIVEN a message with `msg_type: "EVENT"` is received
WHEN the message is received
THEN the cjtCli SHALL route it to the internal `EventHub` for delivery to the local target (Proxy).
