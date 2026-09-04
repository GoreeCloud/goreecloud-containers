# GoreeCloud Containers — Features

## Current Development features

The following capabilities exist in the current source and are Development-only:

- Validated GoreeCloud container identifiers.
- Explicit container lifecycle states and guarded state transitions.
- Deterministic in-memory state for development and tests.
- Typed minimal Linux OCI configuration generation.
- Controlled OCI bundle initialization with no-overwrite behavior.
- `crun` and `runc` runtime identities.
- Explicit runtime version probing.
- Low-level `create`, `start`, `state`, and `delete` runtime execution path.
- Absolute/canonical runtime-path and bundle/config endpoint validation.
- Direct child-process invocation without a command shell.
- Bounded stdout/stderr retention and runtime timeout handling.
- Development CLI for the implemented low-level operations.
- Fake-runtime lifecycle tests and error/timeout/output-bound tests.
- Rust CI and GoreeCloud Platform Contract declaration.

## Planned Phase 1 features

These remain planned rather than implemented:

- OCI registry manifest/config/layer retrieval.
- Digest verification and content-addressed storage.
- Safe image layer unpacking and root-filesystem construction.
- Real `crun`/`runc` lifecycle acceptance.
- Rootless execution acceptance and resource-boundary validation.
- Persistent engine metadata and recovery.
- Logs and complete high-level run/stop/inspect/remove workflows.

## Later planned features

Networking, volumes, health checks, restart policies, Compose compatibility, image builds, remote management, Docker-compatible interoperability endpoints, native management clients, web administration, and GoreeCloud platform-system integrations remain later work.

No planned item in this document should be interpreted as current product availability.
