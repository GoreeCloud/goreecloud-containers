# GoreeCloud Containers — Change Log

## 0.1.0-dev.0 — Native engine development foundation

Status: Development only.

Added:

- Initial Rust workspace and pinned Rust toolchain.
- Core container identity and lifecycle-state model.
- Deterministic in-memory development state store.
- OCI runtime abstraction for `crun` and `runc`.
- Runtime version probing and non-executing OCI lifecycle command planning.
- Initial `goree` development CLI.
- Unit-test, formatting, lint, and build CI.
- GoreeCloud Platform Contract v0.2 manifest and validation workflow.
- Architecture, dependency, security, recovery, and platform-conformance documentation.

Release boundary:

This foundation does not implement OCI image pulling, root filesystem unpacking, container execution, networking, persistent volumes, durable engine metadata, daemon/API behavior, Compose, image builds, remote management, GoreeCloud Integral Platform System runtime integrations, production deployment, or Docker replacement.
