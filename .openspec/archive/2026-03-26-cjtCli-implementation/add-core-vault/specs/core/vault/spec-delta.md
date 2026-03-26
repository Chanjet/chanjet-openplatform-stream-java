# Specification Delta: Physical Secret Vault Implementation

## ADDED Requirements

### Requirement: Secret Encryption Hierarchy
The cjtCli SHALL use a multi-layered approach to secret storage.

#### Scenario: Keyring Usage
GIVEN the operating system supports a standard Keyring/Keychain (macOS, Windows, or Linux with DBus)
WHEN a secret needs to be stored (e.g., `app_secret`)
THEN the cjtCli SHALL store it in the OS Keyring.

#### Scenario: Fallback to AES-GCM
GIVEN the operating system does NOT support a standard Keyring OR the Keyring is inaccessible
WHEN a secret needs to be stored
THEN the cjtCli SHALL store it in an AES-GCM-256 encrypted file named `.seal` in the configuration directory (`~/.cjtCli/.seal`).

### Requirement: Fallback Master Key Derivation
The master key used for the `.seal` file SHALL be derived from a stable system identifier OR a user-provided environment variable `CJT_MASTER_KEY`.

#### Scenario: Environment Variable Master Key
GIVEN the `CJT_MASTER_KEY` environment variable is set
WHEN the `.seal` file needs to be decrypted
THEN the system SHALL use this environment variable as the source for the AES-GCM key.

### Requirement: Secret Retrieval Interface
The system SHALL provide a unified `Vault` interface for managing secrets, abstracting the underlying storage mechanism (Keyring or Seal File).
- `Set(profile, key, secret)`
- `Get(profile, key)`
- `Delete(profile, key)`
