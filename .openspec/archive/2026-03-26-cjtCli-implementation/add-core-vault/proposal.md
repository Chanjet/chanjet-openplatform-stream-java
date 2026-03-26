# Proposal: Physical Secret Vault Implementation (core/vault)

## Why
PRD v0.1.1 mandates that `appSecret` and other sensitive data must NOT be stored in cleartext. We need a dual-layer security approach: OS Keyring for desktop/GUI environments and AES-GCM-256 encrypted local files for headless/server environments.

## What Changes
- Implement `core/vault` package.
- Integrate `github.com/zalando/go-keyring`.
- Implement a fallback mechanism using AES-GCM-256 for systems where Keyring is unavailable.
- Provide a `Vault` interface for storing and retrieving secrets (e.g., `AppSecret`).
- Support a master key for the fallback seal file (e.g., derived from machine ID or a user-provided env var).

## Impact
- **Specs**: Defines the secret storage hierarchy and fallback rules.
- **Code**: New `core/vault` package in `cli/cjtCli`.
- **Users**: Enhanced security for credentials.
- **Security**: Mitigates risk of cleartext secret exposure.
