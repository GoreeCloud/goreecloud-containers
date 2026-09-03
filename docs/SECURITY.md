# Security Model — Development Foundation

## Current scope

This repository is pre-production. The only process execution currently implemented is an explicit OCI runtime `--version` probe initiated by the local CLI. Planned OCI lifecycle commands are represented as data and are not executed.

## Baseline rules

- Rootless is the intended default wherever supported.
- Rootful operation must become an explicit elevated path, never an implicit fallback.
- Runtime choice must be explicit; a selected runtime failure must not silently switch engines or elevate privileges.
- Administrative sockets/APIs must not become unrestricted host-root equivalents.
- Container identifiers and critical paths must be validated before use.
- Bundle planning currently requires absolute paths to remove working-directory ambiguity.
- Runtime output must be treated as untrusted input when later parsed.
- Secrets must not be accepted into ordinary logs, exports, or source-controlled configuration.
- Missing authorization/security evidence must fail closed once privileged or remote operations exist.

## Not yet implemented

- Runtime lifecycle execution and timeout/output bounds.
- Rootless user-namespace/cgroup/network validation.
- Capability, seccomp, SELinux/AppArmor policy handling.
- Image digest/signature/attestation/SBOM verification.
- Secret storage and injection.
- GoreeCloud Identity authorization.
- Wardveil Security integration.
- Privacy Shield enforcement.
- Remote API transport security.
- Security event/audit integration.

No production security claim should be inferred from this document or the current source foundation.
