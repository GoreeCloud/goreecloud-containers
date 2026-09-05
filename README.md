# GoreeCloud Containers

GoreeCloud Containers is the first-party GoreeCloud container-engine and workload-platform project. It is being developed as an OCI-compatible engine that preserves interoperability with mature OCI standards and runtimes while keeping GoreeCloud ownership of the engine contract, CLI, state model, policy, lifecycle, and future platform integrations.

## Development status

**Lifecycle:** Development  
**Version:** `0.1.0-dev.2`  
**Supported development platform:** Linux  
**Production replacement:** No

The current source includes a controlled OCI runtime lifecycle foundation plus a Development image/content pipeline that can retrieve supported single-image OCI/Docker manifests, image configurations, and layers; verify expected SHA-256 digests; store verified content in a bounded content-addressed store; verify uncompressed layer diff IDs; and construct a new root filesystem using a restricted extractor. Deterministic fixture tests validate this source boundary.

**Real external-registry acceptance, registry credential authentication, image-index selection, real `crun`/`runc` lifecycle acceptance, OCI conformance, rootless acceptance, production deployment, and Docker replacement are not established.** Docker remains the current GoreeCloud production container runtime until a separately validated migration changes that operational state.

## Implemented Development foundation

- Rust 2024 workspace pinned to Rust 1.85.0.
- Validated container identifiers and explicit lifecycle-state transitions.
- Deterministic in-memory Development state store.
- Typed minimal Linux OCI `config.json` generation targeting OCI Runtime Specification 1.3.0.
- Fail-closed bundle initialization requiring an absolute existing bundle directory, existing non-symlink `rootfs/`, and no-overwrite `config.json` creation.
- `crun` and `runc` runtime identities, runtime probing, and controlled `create`, `start`, `state`, and `delete` execution using explicit runtime selection.
- Direct runtime process spawning without a shell, executable/bundle/config validation, bounded output, timeout handling, and non-zero-exit propagation.
- Strict SHA-256 digest parsing and verification before image content acceptance.
- Bounded content-addressed storage with safe staging/publication and existing-blob re-verification.
- Development OCI/Docker v2 single-manifest retrieval with bounded anonymous Bearer-token handling and secure transport rules.
- Manifest, image-config, compressed-layer, and uncompressed diff-ID verification.
- Supported tar/gzip layer extraction with path traversal, symlink-parent, entry-size, entry-count, and unpacked-size protections plus OCI whiteout handling.
- Staged construction of a new rootfs target; existing rootfs targets are not merged into.
- Development CLI commands for local verified content ingest and Development image pull/rootfs construction.
- Deterministic fixture-registry, image-content, rootfs, fake-runtime, timeout, and failure-path tests.
- GitHub Actions CI for formatting, Clippy, tests, and build validation.
- GoreeCloud Platform Contract v0.2 declaration and conformance validation.

## Build and inspect the Development CLI

```bash
cargo build --workspace
cargo run -p goree -- version
cargo run -p goree -- container validate-id example-container
cargo run -p goree -- runtime probe crun
```

### Verify and ingest a local content fixture

```bash
cargo run -p goree -- image ingest \
  sha256:<64-lowercase-hex> \
  /absolute/path/to/blob \
  /absolute/path/to/content-store
```

### Pull a Development image and construct a new rootfs

```bash
cargo run -p goree -- image pull \
  https://registry.example \
  team/example \
  v1 \
  /absolute/path/to/content-store \
  /absolute/path/to/new-rootfs
```

`image pull` is a Development interface, not a production image manager. The rootfs target must be a new absolute path. Public/non-loopback registry transport must use HTTPS. Registry user/password credential authentication and image-index/multi-platform selection are not implemented. Symbolic-link and hard-link archive entries are intentionally unsupported by the current restricted extractor.

### Exercise low-level OCI lifecycle operations

```bash
cargo run -p goree -- bundle init /absolute/path/to/bundle /bin/echo hello
cargo run -p goree -- runtime create crun /usr/bin/crun example /absolute/path/to/bundle
cargo run -p goree -- runtime start crun /usr/bin/crun example
cargo run -p goree -- runtime state crun /usr/bin/crun example
cargo run -p goree -- runtime delete crun /usr/bin/crun example
```

The current image-pull path and bundle/runtime paths are not yet a single accepted high-level `run` workflow. Automated runtime lifecycle evidence still uses a fake runtime and must not be treated as real `crun`/`runc` acceptance.

## Documentation

- [User Manual](USER-MANUAL.md)
- [Specifications](SPECIFICATIONS.md)
- [Features](FEATURES.md)
- [Benefits](BENEFITS.md)
- [Competitive Objectives](COMPETITIVE-OBJECTIVES.md)
- [Branding](BRANDING.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Dependencies](docs/DEPENDENCIES.md)
- [Security](docs/SECURITY.md)
- [Recovery](docs/RECOVERY.md)
- [Platform Conformance](docs/PLATFORM_CONFORMANCE.md)

## Next Phase 1 evidence gates

The next required evidence includes real external-registry interoperability testing, a controlled image-to-bundle integration path, real `crun`/`runc` lifecycle acceptance, and rootless execution/resource-boundary acceptance. These gates remain separate from the source-level fixture tests completed in this version.

## License

Original GoreeCloud Containers source is licensed under the Apache License 2.0. External runtimes and dependencies retain their own licenses and obligations.
