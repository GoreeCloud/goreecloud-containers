# Security Model — Development Foundation

## Current scope

The repository is pre-production. It now contains a controlled source path capable of invoking a selected OCI runtime for `create`, `start`, `state`, and `delete`, plus an OCI bundle/config generator. Automated lifecycle tests use a fake runtime; real `crun`/`runc` and rootless acceptance remain pending.

## Implemented safety boundaries

- Rootless operation remains the intended default, but is not yet accepted.
- Runtime lifecycle execution requires an explicit runtime kind and absolute executable path.
- The executable path is canonicalized and must resolve to a regular executable file with executable permission on Unix.
- Runtime selection does not silently fall back or elevate.
- `create` requires an absolute bundle, validates/canonicalizes it, rejects a symbolic-link bundle endpoint, and requires a regular non-symlink `config.json` endpoint.
- Bundle initialization rejects symbolic-link bundle/rootfs endpoints and refuses to overwrite `config.json`.
- Generated Development config enables `noNewPrivileges` and defaults the in-container process to UID/GID 65534.
- The runtime is spawned directly without a command shell and with null stdin.
- stdout/stderr are drained and retained with a default 1 MiB-per-stream bound.
- A default 30-second timeout attempts to terminate the directly invoked runtime process.
- Non-zero runtime exits are errors rather than silent success.
- Runtime output remains untrusted input; current `state` output is not parsed into trusted durable state.

## Important limitations

- UID/GID 65534 inside the generated container config does **not** prove host-level rootless execution.
- No user-namespace mapping, cgroup policy, capability policy, seccomp profile, SELinux/AppArmor integration, or network configuration is accepted yet.
- Timeout handling targets the directly invoked process; complete process-tree supervision/recovery is not established.
- No image digest/signature/attestation/SBOM verification exists yet.
- No secret store/injection, GoreeCloud Identity authorization, Wardveil Security integration, Privacy Shield enforcement, remote API security, or security-event integration exists.

No production security claim should be inferred from the current source foundation.
