# Specification Delta: Local Proxy & Dead-Letter Queue

## ADDED Requirements

### Requirement: Dead-Letter Queue (DLQ) Persistence
The cjtCli daemon SHALL maintain a persistent Dead-Letter Queue (DLQ) using a local SQLite database for events that fail to be delivered after multiple retries.

#### Scenario: Event Capture into DLQ
GIVEN the cjtCli daemon is processing an `EventFrame`
WHEN the target webhook receiver is offline or returns a 5xx error
AND all retry attempts (exponential backoff) have been exhausted
THEN the cjtCli SHALL store the `EventFrame` and its failure metadata into the local `dlq.db`.

### Requirement: Local Loopback Proxy (Zero-Auth)
The cjtCli SHALL provide a local HTTP proxy that automatically injects authentication headers into outgoing requests to the Chanjet platform.

#### Scenario: Local Loopback Access
GIVEN a local legacy application is running on the same machine
WHEN the application makes a plain HTTP request to `http://127.0.0.1:PORT/v1/orders`
THEN the cjtCli proxy SHALL intercept the request
AND inject the current `AppAccessToken` and necessary signatures
AND forward the request to the Chanjet platform
AND return the response back to the application.

#### Scenario: Network Isolation
GIVEN the cjtCli local proxy is running
WHEN a request is made from a DIFFERENT machine on the network to the proxy port
THEN the cjtCli SHALL REJECT the connection immediately.

### Requirement: Three-Stage Reliability Algorithm
The system SHALL follow this delivery algorithm for incoming events:
1. **Immediate Delivery**: Try once immediately.
2. **Exponential Backoff**: If failed, retry up to 5 times with growing delays.
3. **DLQ Sink**: If still failing, persist to the local SQLite database.
