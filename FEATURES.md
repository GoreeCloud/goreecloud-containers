# GoreeCloud Containers — Features

## Current Development features

The following capabilities exist in the current source and are Development-only:

- Validated GoreeCloud container identifiers.
- Explicit container lifecycle states and guarded state transitions.
- Deterministic in-memory state for development and tests.
- Typed minimal Linux OCI configuration generation and controlled no-overwrite bundle initialization.
- `crun` and `runc` runtime identities, version probing, and low-level `create`, `start`, `state`, and `delete` execution.
- Absolute/canonical runtime-path and bundle/config endpoint validation.
- Direct child-process invocation without a command shell, bounded stdout/stderr retention, runtime timeout handling, and explicit failure propagation.
- Strict lowercase SHA-256 digest parsing and verification.
- Bounded content-addressed image store with verified existing-content reuse.
- Development retrieval of supported OCI Image Manifest and Docker Registry v2 image manifests.
- Development retrieval and parsing of OCI/Docker image configuration and supported image layers.
- Bounded anonymous Bearer-token challenge handling for pull scopes; public/non-loopback transport requires HTTPS.
- Manifest/config/layer verification before acceptance into the verified content path.
- Linux image validation and image-config layer/diff-ID consistency checks.
- Supported uncompressed tar and gzip-compressed OCI/Docker layer handling.
- Uncompressed diff-ID verification before rootfs publication.
- Restricted layer extraction with path traversal, symlink-parent, per-entry, per-layer entry-count, and unpacked-size protections.
- OCI whiteout and opaque-whiteout handling for supported entries.
- Staged construction of a new rootfs target without merging into an existing target.
- Development CLI commands `goree image ingest` and `goree image pull`.
- Deterministic fixture-registry, content-store, rootfs, fake-runtime, timeout, and failure-path tests.
- Rust CI and GoreeCloud Platform Contract declaration.

## Current Development limitations / pending acceptance

The current source does not establish:

- Registry username/password or other reusable-credential authentication.
- OCI image-index/Docker manifest-list selection or multi-platform resolution.
- Symbolic-link or hard-link archive entry extraction.
- Image signature, attestation, SBOM, provenance, or trust-policy verification.
- Real external-registry interoperability acceptance.
- Accepted real `crun`/`runc` lifecycle behavior or full OCI conformance.
- Rootless execution acceptance and resource-boundary validation.
- A high-level image-to-container `run` workflow.
- Persistent engine metadata and recovery acceptance.
- Complete logs/run/stop/inspect/remove engine workflows.

## Later planned features

Networking, volumes, health checks, restart policies, Compose compatibility, image builds, remote management, Docker-compatible interoperability endpoints, native management clients, web administration, and accepted GoreeCloud platform-system integrations remain later work.

No pending or planned item in this document should be interpreted as current product availability.
