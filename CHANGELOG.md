# GoreeCloud Containers — Change Log

## 0.1.0-dev.2 — OCI image/content pipeline foundation

Status: Development only.

Added:

- New `goreecloud-containers-image` crate for digest-verified image content, registry retrieval, and controlled root-filesystem construction.
- Strict lowercase `sha256:<64-hex>` digest parsing and SHA-256 verification using the bounded RustCrypto `sha2` foundation.
- Bounded content-addressed storage with absolute-path validation, endpoint symlink checks, create-new staging, digest verification before acceptance, and verified existing-blob reuse.
- Development OCI/Docker v2 manifest, image-config, and layer retrieval for single-image manifests.
- Bounded anonymous Bearer-token challenge flow with pull-scope validation, HTTPS enforcement for non-loopback registries, redirect controls, and response-size limits.
- Manifest/config/layer digest verification before content is treated as accepted image content.
- OCI image configuration parsing, Linux-image validation, layer/diff-ID count validation, and supported layer-media-type validation.
- Controlled tar/gzip layer extraction with uncompressed diff-ID verification, traversal and symlink-parent protections, entry/count/size bounds, OCI whiteout handling, staged rootfs construction, and no merge into an existing rootfs target.
- Development CLI commands `goree image ingest` and `goree image pull`.
- Deterministic local fixture-registry and rootfs tests covering successful retrieval/construction plus digest mismatch, traversal, symlink, size, and diff-ID failure cases.
- Pinned material Rust dependencies and reconciled Development documentation/Platform Contract evidence.

Validation boundary:

Automated image retrieval tests use a deterministic local fixture registry. They validate the GoreeCloud source boundary for retrieval, verification, storage, and supported-layer rootfs construction, but do not establish real external-registry interoperability acceptance or production readiness.

Release boundary:

Registry credential authentication, image-index/multi-platform selection, symbolic-link/hard-link layer entries, signatures/attestations/SBOM policy, real external-registry acceptance, real `crun`/`runc` lifecycle acceptance, rootless acceptance, durable metadata, networking, volumes, Compose, builds, platform-system runtime integrations, production deployment, Stable qualification, and Docker replacement remain pending.

## 0.1.0-dev.1 — Controlled OCI lifecycle execution foundation

Status: Development only.

Added:

- New `goreecloud-containers-oci` crate with typed minimal Linux OCI configuration targeting Runtime Specification 1.3.0.
- Controlled bundle initialization requiring absolute bundle paths, an existing non-symlink `rootfs/`, and no-overwrite `config.json` creation.
- Generated Development configuration with `noNewPrivileges`, UID/GID 65534 process defaults, and PID/network/IPC/UTS/mount namespaces.
- Controlled `create`, `start`, `state`, and `delete` runtime execution path for explicit `crun`/`runc` selections.
- Absolute/canonical runtime executable validation plus bundle/config validation.
- Direct process spawning, bounded stdout/stderr retention, timeout handling, and non-zero-exit propagation.
- Development CLI commands for bundle initialization and lifecycle execution.
- Deterministic fake-runtime lifecycle tests, output-bound tests, timeout tests, and failure-path tests.
- Mandatory repository documentation and synchronized user-manual foundation.

Validation boundary:

Automated lifecycle tests use a fake runtime. They validate the GoreeCloud source execution boundary but do not establish real `crun`/`runc` lifecycle acceptance, OCI conformance, rootless execution, or production readiness.

Release boundary:

OCI image pulling/unpacking, real runtime acceptance, rootless acceptance, durable metadata, networking, volumes, Compose, builds, platform-system runtime integrations, production deployment, Stable qualification, and Docker replacement remained pending at this version.

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

This foundation did not implement OCI image pulling, root filesystem unpacking, container execution, networking, persistent volumes, durable engine metadata, daemon/API behavior, Compose, image builds, remote management, GoreeCloud Integral Platform System runtime integrations, production deployment, or Docker replacement.
