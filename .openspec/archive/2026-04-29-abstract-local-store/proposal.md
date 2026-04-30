# Proposal: Abstract Local Storage as a Store Implementation

## Why
Currently, Cowen has two parallel storage architectures:
1.  **`MultiVault`**: Handles local encrypted file storage.
2.  **`DistributedVault`**: Wraps a `Store` trait for remote backends (SQL, Redis).

This leads to code duplication and feature inconsistency (e.g., `rename_profile` only works on local storage). By abstracting local file storage as a `FileStore` implementation of the `Store` trait, we can unify the `Vault` logic, simplify `main.rs` initialization, and ensure all storage backends support the same management operations.

## What Changes
1.  **Enhance `Store` Trait**:
    - Add `clear_profile(profile)` and `rename_profile(old, new)` to the `Store` trait.
2.  **Implement `FileStore`**:
    - Create `src/core/store/file.rs`.
    - Move encryption and file locking logic from `MultiVault` to `FileStore`.
3.  **Implement New Methods in Existing Stores**:
    - Add SQL implementations for `clear_profile` (DELETE WHERE profile=?) and `rename_profile` (UPDATE profile=?).
    - Add Redis implementations (pattern-based deletion/renaming).
4.  **Unify `Vault`**:
    - Refactor `src/core/vault.rs` to have a single `VaultImpl` (or keep `DistributedVault` as the sole implementation and rename it).
    - Remove `MultiVault`.
5.  **Refactor Initialization**:
    - Simplify `create_vault` in `main.rs` to always return the unified `Vault` wrapping the appropriate `Store`.

## Impact
- **Architecture**: Cleaner, unified storage abstraction.
- **Features**: Remote storage backends will now support profile management (clear/rename).
- **Maintenance**: Reduced code duplication in storage and vault logic.
- **Testing**: Easier to switch backends in integration tests.
