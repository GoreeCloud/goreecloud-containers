# GoreeCloud Containers

GoreeCloud Containers is the planned first-party OCI-compatible container engine and workload platform for GoreeCloud.

## Development status

**Lifecycle:** Development  
**Version:** `0.1.0-dev.0`  
**Production replacement:** No

This repository now contains the first native Rust development foundation. It does **not** yet run containers, pull images, provide networking, persist engine state, expose a daemon/API, or replace Docker in GoreeCloud production.

Docker remains the current GoreeCloud production container runtime until a separately validated migration changes that operational state.

## Implemented in this foundation

- A Rust workspace owned by GoreeCloud.
- A core crate with validated container identifiers, explicit lifecycle states, guarded state transitions, and a deterministic in-memory state store for development/tests.
- An OCI runtime adapter crate with explicit `crun` and `runc` runtime identities.
- Runtime version probing through a narrowly scoped process invocation.
- Deterministic command planning for OCI `create`, `start`, `state`, and `delete` operations without executing container lifecycle operations yet.
- A `goree` development CLI with `version`, `runtime probe`, and `container validate-id` commands.
- Unit tests for identifier validation, state transitions, deterministic state ordering, runtime parsing, and OCI command planning.
- GitHub Actions CI for formatting, linting, tests, and build validation.
- GoreeCloud Platform Contract v0.2 declaration and reusable platform-contract validation.
- Development documentation for architecture, dependencies, security, recovery, and platform-conformance boundaries.

## Try the development CLI

```bash
cargo run -p goree -- version
cargo run -p goree -- container validate-id example-container
cargo run -p goree -- runtime probe crun
```

`runtime probe` requires the selected executable to be installed and reachable. A successful probe proves only that the runtime executable can be invoked and report a version; it does not prove container lifecycle support or production readiness.

## Architecture direction

GoreeCloud owns the engine, API, CLI, state model, policy, networking/storage orchestration, workload lifecycle, user experience, and GoreeCloud platform integrations. Mature OCI runtimes remain bounded low-level execution foundations.

The preferred initial runtime is `crun`; `runc` is supported as an alternative runtime target. OCI Image, Runtime, and Distribution interoperability, Docker Registry compatibility, Dockerfile compatibility where practical, and Compose compatibility remain strategic requirements.

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and the canonical GoreeCloud Project Specification for the full planned direction.

## Next implementation milestone

The next Phase 1 slice should add a controlled OCI bundle model and a real create/start/state/delete execution path behind explicit safety checks, followed by OCI image manifest/layer retrieval and unpacking. Durable engine metadata must be designed before any state is represented as recoverable.

## License

Original GoreeCloud Containers source is licensed under the Apache License 2.0. External runtimes and dependencies retain their own licenses and obligations.
