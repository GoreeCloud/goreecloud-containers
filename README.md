# GoreeCloud Containers

GoreeCloud Containers is the first-party GoreeCloud container-engine and workload-platform project. It is being developed as an OCI-compatible engine that preserves interoperability with mature OCI runtimes while keeping GoreeCloud ownership of the engine contract, CLI, state model, policy, lifecycle, and future platform integrations.

## Development status

**Lifecycle:** Development  
**Version:** `0.1.0-dev.1`  
**Supported development platform:** Linux  
**Production replacement:** No

The repository now contains a source-level OCI bundle/config model and a controlled lifecycle execution path for `create`, `start`, `state`, and `delete`. The execution path is validated with deterministic fake-runtime tests. **Real `crun`/`runc` lifecycle acceptance, OCI conformance, rootless acceptance, image pulling/unpacking, production deployment, and Docker replacement are not established.**

Docker remains the current GoreeCloud production container runtime until a separately validated migration changes that operational state.

## Implemented Development foundation

- Rust 2024 workspace pinned to Rust 1.85.0.
- Validated container identifiers and an explicit lifecycle-state model.
- Deterministic in-memory Development state store.
- Typed minimal Linux OCI `config.json` generation targeting OCI Runtime Specification 1.3.0.
- Fail-closed bundle initialization that requires an absolute existing bundle directory and existing non-symlink `rootfs/`, and refuses to overwrite `config.json`.
- OCI runtime identities for `crun` and `runc`.
- Runtime version probing.
- Controlled runtime execution for `create`, `start`, `state`, and `delete` using an explicit absolute runtime executable path.
- Runtime executable/bundle/config validation, direct process spawning without a shell, bounded stdout/stderr capture, timeout handling, and non-zero-exit propagation.
- Development CLI commands for bundle initialization, runtime lifecycle operations, runtime probing, and container-ID validation.
- Fake-runtime tests plus output-bound, timeout, and failure-path tests.
- GitHub Actions CI for formatting, Clippy, tests, and build validation.
- GoreeCloud Platform Contract v0.2 declaration and conformance validation.

## Build and inspect the Development CLI

```bash
cargo build --workspace
cargo run -p goree -- version
cargo run -p goree -- container validate-id example-container
cargo run -p goree -- runtime probe crun
```

The lifecycle commands are deliberately low-level Development interfaces. They require an already prepared OCI bundle/root filesystem and an explicit absolute runtime executable path:

```bash
cargo run -p goree -- bundle init /absolute/path/to/bundle /bin/echo hello
cargo run -p goree -- runtime create crun /usr/bin/crun example /absolute/path/to/bundle
cargo run -p goree -- runtime start crun /usr/bin/crun example
cargo run -p goree -- runtime state crun /usr/bin/crun example
cargo run -p goree -- runtime delete crun /usr/bin/crun example
```

`bundle init` requires `/absolute/path/to/bundle/rootfs/` to already exist. GoreeCloud Containers does not yet pull an OCI image or construct that root filesystem. The runtime lifecycle commands can execute a selected binary, but current automated evidence uses a fake runtime and must not be treated as real `crun`/`runc` acceptance.

## Documentation

- [User Manual](USER-MANUAL.md)
- [Specifications](SPECIFICATIONS.md)
- [Features](FEATURES.md)
- [Benefits](BENEFITS.md)
- [Competitive Objectives](COMPETITIVE-OBJECTIVES.md)
- [Branding](BRANDING.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Security](docs/SECURITY.md)
- [Recovery](docs/RECOVERY.md)
- [Platform Conformance](docs/PLATFORM_CONFORMANCE.md)

## Next Phase 1 milestone

The next major engine slice is the OCI image/content pipeline: registry manifest resolution, content-digest verification, bounded layer retrieval, content-addressed storage, safe layer unpacking, and root-filesystem construction. Real runtime/rootless acceptance remains a separate evidence gate.

## License

Original GoreeCloud Containers source is licensed under the Apache License 2.0. External runtimes and dependencies retain their own licenses and obligations.
