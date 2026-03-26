# Proposal: Telemetry and Structured Logging Implementation (core/telemetry)

## Why
PRD v0.1.1 requires structured, domain-separated logging (System, Audit, Stream, DLQ) and strictly structured output (JSON/YAML) for AI Agent friendliness. We need a robust logging core that supports rolling logs and a mechanism to intercept panics to prevent raw stack traces in standard output.

## What Changes
- Implement `core/telemetry` package.
- Integrate `uber-go/zap` for high-performance structured logging.
- Integrate `natefinch/lumberjack` for log rotation (rolling files).
- Implement four specialized loggers: `System`, `Audit`, `Stream`, `DLQ`.
- Implement an global output interceptor/middleware for Cobra commands to support `--format json/yaml`.
- Implement a `Recover` hook to capture panics and output them as structured JSON/YAML.

## Impact
- **Specs**: Defines the logging domains and the structured output protocol.
- **Code**: New `core/telemetry` package and Cobra integration in `cli/cjtCli`.
- **Users**: Machine-friendly output and comprehensive audit logs.
- **Observability**: Multi-domain logs for easier debugging and monitoring.
