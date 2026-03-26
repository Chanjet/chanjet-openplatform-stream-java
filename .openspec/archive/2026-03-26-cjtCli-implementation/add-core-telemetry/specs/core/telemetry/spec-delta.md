# Specification Delta: Telemetry & Structured Logging

## ADDED Requirements

### Requirement: Four-Layer Log Separation
The cjtCli SHALL separate logs into four distinct domains, each stored in a dedicated file in `~/.cjtCli/log/`.

#### Scenario: Log File Generation
GIVEN the cjtCli is running
WHEN logs are generated
THEN they SHALL be distributed as follows:
- `sys.log`: System lifecycle and operational errors.
- `audit.log`: Audit trail of all requests and proxy operations.
- `stream.log`: Incoming events from the open platform.
- `dlq.log`: Dead-letter queue events and failure stacks.

### Requirement: Log Rotation
The cjtCli SHALL automatically rotate log files to prevent excessive disk usage.
- `max_size`: 500MB per file.

### Requirement: Machine-Friendly Structured Output
The cjtCli SHALL support `--format json` and `--format yaml` flags for all commands.

#### Scenario: JSON Output Format
GIVEN the `--format json` flag is used
WHEN a command is executed
THEN the `STDOUT` SHALL only contain a pure JSON object, with all human-readable UI elements suppressed.

#### Scenario: Panic Recovery and Reporting
GIVEN the cjtCli encounters a critical error (panic)
WHEN a structured format is requested
THEN the system SHALL intercept the panic and output it as a structured error object in the requested format, including a `suggestion` field for remediation.
