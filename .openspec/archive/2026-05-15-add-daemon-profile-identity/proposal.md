# Proposal: Use Profile Name as Daemon Process Identity

## Why
Currently, all Cowen daemon processes are named `cowen` in the system process list. When multiple profiles are running simultaneously (e.g., `default`, `inte`, `prod`), it is impossible to distinguish between them using standard tools like `ps`, `top`, or `htop`. This leads to operational confusion and difficulty in troubleshooting specific instances.

## What Changes
1.  **cowen-common**: Implement a cross-platform utility `set_process_name(name: &str)` that uses `prctl` on Linux and `pthread_setname_np` on macOS to update the process title.
2.  **cowen-server**: Modify the daemon startup sequence to automatically set the process name to `cowen:<profile>` after the background process is spawned and initialized.
3.  **TDD Validation**: Add unit tests in `cowen-common` to verify the process naming utility and integration tests to ensure the daemon displays the correct name.

## Impact
-   **Observability**: Developers and operators can clearly identify which profile each `cowen` process belongs to.
-   **Stability**: No breaking changes to existing CLI arguments or configuration files.
-   **Platform Support**: Initially supports Linux and macOS. Windows support will fallback to default behavior (as setting process titles in Windows is significantly more complex and usually involves changing the executable name).
