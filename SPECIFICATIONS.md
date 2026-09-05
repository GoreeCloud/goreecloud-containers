# GoreeCloud Containers — Repository Specifications

## Status

**Lifecycle:** Development  
**Source version:** `0.1.0-dev.2`  
**Primary implementation language:** Rust  
**Current development platform:** Linux  
**Production status:** Not production-ready

This repository specification describes the implemented Development source boundary plus immediate engineering requirements. The broader canonical product specification remains the GoreeCloud project record. Planned capabilities are not current implementation unless explicitly identified below.

## Product boundary

GoreeCloud Containers is a first-party OCI-compatible container engine and workload-platform project. GoreeCloud owns the high-level engine contract, CLI/API direction, state model, policy, workload lifecycle, networking/storage orchestration, and platform integrations. Mature OCI runtimes, standards implementations, cryptographic primitives, compression/archive libraries, and protocol libraries remain bounded technical foundations.

Docker and Podman interoperability is a goal; copying their private implementation architecture or product identity is not.

## Current workspace

The Rust workspace contains:

- `goreecloud-containers-core`: container identifiers, lifecycle states, transitions, and Development memory state.
- `goreecloud-containers-image`: digest verification, bounded content-addressed storage, registry retrieval, image metadata parsing, and controlled rootfs construction.
- `goreecloud-containers-oci`: typed minimal OCI Linux configuration and controlled bundle initialization.
- `goreecloud-containers-runtime`: `crun`/`runc` runtime selection, probing, command planning, validation, and controlled lifecycle process execution.
- `goree`: current Development CLI.

Unsafe Rust is forbidden at the workspace lint level.

## OCI image/content pipeline

The current Development image crate provides a source-level single-manifest image path:

1. Parse a registry base URL, repository, and tag or SHA-256 digest reference.
2. Resolve a supported OCI Image Manifest or Docker Registry v2 image manifest.
3. Use bounded anonymous/Bearer pull authorization where required; reusable registry credentials are not implemented.
4. Verify the manifest against a digest reference and/or returned `Docker-Content-Digest` when present.
5. Retrieve the image configuration and layer descriptors.
6. Verify config and layer bytes against expected SHA-256 descriptor digests before accepting them into the content store.
7. Parse the Linux image configuration and require a one-to-one layer/diff-ID relationship.
8. Verify each layer's uncompressed diff ID.
9. Apply supported layer entries into a staged rootfs using the restricted extractor.
10. Publish the rootfs only after all supported layers succeed.

Network URLs require HTTPS except for loopback HTTP used by deterministic development fixtures. Redirect handling refuses HTTPS downgrade. Responses, content blobs, archive entries, archive entry counts, and uncompressed layer sizes are bounded.

The content store requires an absolute existing root directory whose endpoint is not a symbolic link. Incoming content is verified before publication and existing content-addressed blobs are re-verified before reuse.

The rootfs builder requires a new absolute target with a canonical non-symlink parent and refuses to merge into an existing target. It rejects unsafe archive traversal, symlink-parent traversal, unsupported archive entry types including current symlink/hard-link entries, and oversized content. Supported OCI whiteouts are applied without following symlink targets.

Deterministic local fixture-registry tests validate this source boundary. They are not real external-registry acceptance evidence.

## OCI bundle/config foundation

The OCI crate:

- targets OCI Runtime Specification version `1.3.0` in generated configuration;
- requires at least one process argument;
- defaults the container process to UID/GID `65534`;
- enables `noNewPrivileges`;
- declares PID, network, IPC, UTS, and mount namespaces;
- uses `rootfs` as the bundle-relative root filesystem;
- requires an absolute existing bundle directory;
- rejects a bundle path whose endpoint is a symbolic link;
- requires an existing `rootfs/` directory whose endpoint is not a symbolic link;
- refuses to overwrite an existing `config.json`;
- writes a new `config.json` with create-new semantics and synchronizes the file.

This remains a Development configuration foundation. It is not an accepted rootless profile, production security profile, or complete OCI compatibility implementation.

## Runtime execution foundation

Lifecycle execution supports the low-level operations `create`, `start`, `state`, and `delete` for an explicitly selected `crun` or `runc` executable. Execution requires an absolute runtime executable path; the path is canonicalized and must resolve to a regular executable file. `create` additionally validates/canonicalizes an absolute bundle and requires a regular non-symlink `config.json`.

The executor invokes the runtime directly rather than through a shell, provides null stdin, captures stdout/stderr concurrently, retains at most 1 MiB per output stream by default while continuing to drain it, applies a 30-second default timeout, attempts to terminate the invoked runtime process on timeout, and preserves non-zero exit status with bounded error output.

Automated lifecycle tests use a deterministic fake executable. They validate the GoreeCloud execution boundary, not real OCI runtime conformance.

## Current CLI contract

Development commands currently include:

```text
goree version
goree bundle init <absolute-bundle> <command> [args...]
goree image ingest <sha256:digest> <absolute-source-file> <absolute-store-root> [max-content-bytes]
goree image pull <registry-base-url> <repository> <tag-or-sha256-digest> <absolute-store-root> <absolute-new-rootfs>
goree runtime probe [crun|runc] [executable]
goree runtime create <crun|runc> <absolute-executable> <container-id> <absolute-bundle>
goree runtime start <crun|runc> <absolute-executable> <container-id>
goree runtime state <crun|runc> <absolute-executable> <container-id>
goree runtime delete <crun|runc> <absolute-executable> <container-id>
goree container validate-id <container-id>
```

These interfaces are Development-only and may change before a stable CLI contract is declared.

## Current non-goals and missing/unsupported capabilities

The current source does not provide or establish:

- Registry reusable-credential authentication.
- OCI image-index or Docker manifest-list selection/multi-platform resolution.
- Symbolic-link/hard-link layer entry extraction.
- Image signature, provenance, attestation, SBOM, or trust-policy verification.
- Real external-registry interoperability acceptance.
- Accepted real `crun`/`runc` lifecycle tests or complete OCI conformance.
- Accepted rootless execution, user-namespace mapping, cgroup policy, or network setup.
- A complete high-level image-to-container run workflow.
- Persistent engine metadata or a recovery-accepted database.
- Networks, volumes, port publishing, service discovery, or resource policy.
- Health/restart management.
- Daemon or versioned remote API.
- Compose or Docker API compatibility.
- Image builds.
- Graphical/native management clients.
- Accepted GoreeCloud Manager, Identity, Wardveil Security, Privacy Shield, Everkeep, Mesh, or Glaze UI runtime integration.
- Production deployment, Stable qualification, or Docker replacement.

## Validation requirements

Every material source candidate must pass formatting, Clippy with warnings denied, workspace tests, workspace build, and applicable GoreeCloud Platform Contract validation. Real external-registry, runtime, rootless, recovery, networking, storage, compatibility, and production claims require separate target-environment evidence.
