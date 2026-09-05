# Material Dependencies

GoreeCloud Containers is an original GoreeCloud-owned product. The dependencies below are bounded technical foundations and retain their own licenses/security lifecycles. Exact direct Rust versions are pinned in the image crate manifest for this Development slice.

## Rust toolchain

Purpose: build the GoreeCloud-owned engine libraries and CLI.  
Current repository minimum/pin: Rust 1.85.0, Edition 2024.  
Boundary: build/language toolchain; it does not define GoreeCloud Containers product behavior.  
Update path: deliberate toolchain updates through reviewed source changes and CI.

## sha2 `0.10.8`

Purpose: mature SHA-256 implementation for OCI-compatible content and uncompressed layer digest verification.  
Boundary: digest calculation only; GoreeCloud owns digest syntax/policy, verification sequencing, size limits, storage layout, errors, and orchestration.  
Security/privacy: hashes bytes locally and does not perform networking or secret storage.  
Failure implication: mismatched content/diff IDs are rejected before accepted publication.

## ureq `2.12.1`

Purpose: synchronous HTTPS/HTTP client foundation for the Development OCI/Docker registry retrieval path.  
Configuration: default features disabled; TLS feature enabled.  
Boundary: HTTP/TLS transport mechanics only. GoreeCloud owns registry URL policy, HTTPS/loopback restrictions, redirects, authentication challenge handling, request scope, size bounds, content verification, and errors.  
Security/privacy: remote registry/token endpoints receive requests necessary for image retrieval; reusable registry credential handling is not implemented.  
Failure implication: transport/status/redirect/authentication failures are surfaced and do not silently downgrade transport.

## url `2.5.0` and idna `0.5.0`

Purpose: standards-oriented URL parsing/normalization and host handling for registry endpoints.  
Boundary: URL syntax/normalization foundation; GoreeCloud owns allowed schemes/origins, path construction, redirect policy, and error redaction.  
Security/privacy: no independent telemetry or credential storage is introduced by these libraries.

## serde `1.0.217` and serde_json `1.0.138`

Purpose: bounded JSON deserialization for OCI/Docker manifests, image configuration, and Bearer-token responses.  
Boundary: serialization/deserialization mechanics only; GoreeCloud defines accepted schemas/media types, validation rules, size bounds, and downstream trust decisions.  
Failure implication: malformed or unsupported image metadata fails explicitly.

## flate2 `1.0.35`

Purpose: gzip decompression for supported OCI/Docker image layers.  
Configuration: default features disabled; Rust backend enabled.  
Boundary: decompression primitive only; GoreeCloud owns media-type acceptance, uncompressed byte bounds, diff-ID verification, extraction policy, and rootfs publication.  
Failure implication: decompression/read/limit failures abort the staged rootfs build.

## tar `0.4.43`

Purpose: tar archive parsing for supported image layers.  
Configuration: default features disabled.  
Boundary: archive record parsing only. GoreeCloud owns safe path validation, allowed entry types, whiteout behavior, size/count limits, symlink policy, filesystem writes, and staged publication.  
Failure implication: unsafe/unsupported entries fail closed rather than broadening extraction behavior.

## crun

Purpose: preferred mature OCI low-level runtime target.  
Current source use: can be explicitly probed, planned, and invoked by the Development lifecycle executor when the caller supplies its absolute executable path.  
Accepted runtime status: **not yet accepted**; automated lifecycle evidence currently uses a fake runtime.  
Architectural boundary: low-level OCI process/container execution only. GoreeCloud retains high-level lifecycle, state, policy, API, networking, storage, and UX ownership.  
License/security: crun retains its own license/security lifecycle; distribution and dependency review are required before GoreeCloud packages or bundles it.  
Failure implication: an explicitly selected crun failure is surfaced; the source does not silently fall back or elevate.

## runc

Purpose: alternative mature OCI low-level runtime target.  
Current source use: can be explicitly probed, planned, and invoked by the Development lifecycle executor when the caller supplies its absolute executable path.  
Accepted runtime status: **not yet accepted**; automated lifecycle evidence currently uses a fake runtime.  
Architectural boundary: same low-level execution boundary as crun.  
License/security: runc retains its own license/security lifecycle.  
Failure implication: an explicitly selected runc failure is surfaced rather than silently replaced.

## Linux kernel facilities

Execution ultimately relies on operating-system container primitives such as namespaces, cgroups, filesystems, capabilities, seccomp, and security modules. These are platform facilities, not components GoreeCloud should reimplement merely for ownership.

Every material dependency added later must document purpose, necessity, version/compatibility expectations, licensing, security/privacy implications, failure behavior, and replacement/update path.
