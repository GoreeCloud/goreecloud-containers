# Security Model — Development Foundation

## Current scope

The repository is pre-production. It contains controlled source paths for supported image retrieval/content verification/rootfs construction, OCI bundle/config generation, and low-level runtime lifecycle invocation. Deterministic image tests use a local fixture registry, and automated runtime lifecycle tests use a fake runtime. Real external-registry, real `crun`/`runc`, and rootless acceptance remain pending.

## Implemented image/content safety boundaries

- Expected image content uses strict lowercase SHA-256 digests and is hashed before acceptance into the content-addressed store.
- Existing content-addressed blobs are re-verified before reuse.
- Content-store roots and source paths must be absolute and pass endpoint type/symlink checks.
- Incoming content uses create-new staging and bounded streaming; size/digest failures do not publish the candidate as verified content.
- Non-loopback registry network URLs require HTTPS; loopback HTTP is permitted only for Development fixtures/local testing.
- HTTPS-to-non-HTTPS redirects are refused.
- Registry URLs are validated and ordinary error rendering strips query/fragment data.
- Registry response bodies have explicit bounds for manifests, configs, tokens, and blobs.
- Bearer authentication is limited to the pull-scope flow implemented by the client; reusable registry username/password credential handling is not implemented.
- Manifest, configuration, and layer descriptor digests are verified before dependent content is trusted by the image path.
- Uncompressed layer diff IDs are verified before a completed rootfs is published.
- Rootfs construction occurs in a new staging tree and refuses an already existing target.
- Archive extraction bounds per-entry bytes, entries per layer, and uncompressed bytes per layer.
- Archive paths and parent chains are validated; traversal and symbolic-link-parent extraction are rejected.
- Current symlink/hard-link archive entries and other unsupported entry types fail closed.
- Supported whiteout handling removes targets without following symlinks.

## Implemented runtime/bundle safety boundaries

- Rootless operation remains the intended default, but is not yet accepted.
- Runtime lifecycle execution requires an explicit runtime kind and absolute executable path.
- The executable path is canonicalized and must resolve to a regular executable file with executable permission on Unix.
- Runtime selection does not silently fall back or elevate.
- `create` validates/canonicalizes an absolute bundle, rejects a symbolic-link bundle endpoint, and requires a regular non-symlink `config.json` endpoint.
- Bundle initialization rejects symbolic-link bundle/rootfs endpoints and refuses to overwrite `config.json`.
- Generated Development config enables `noNewPrivileges` and defaults the in-container process to UID/GID 65534.
- The runtime is spawned directly without a command shell and with null stdin.
- stdout/stderr are drained with bounded retained output.
- A default timeout attempts to terminate the directly invoked runtime process.
- Non-zero runtime exits are errors rather than silent success.
- Runtime output remains untrusted input; current `state` output is not parsed into trusted durable state.

## Important limitations

- Digest verification proves byte integrity relative to an expected digest; it does **not** establish publisher identity, image signature trust, provenance, vulnerability status, or policy approval.
- No image signature, attestation, SBOM, transparency-log, or provenance verification exists yet.
- No registry reusable-credential storage/injection flow exists.
- Image indexes/manifest lists are not selected, so multi-platform resolution is not accepted.
- Real external registry behavior is not yet accepted by target-environment testing.
- UID/GID 65534 inside generated container config does **not** prove host-level rootless execution.
- No accepted user-namespace mapping, cgroup policy, capability profile, seccomp profile, SELinux/AppArmor integration, or network configuration exists yet.
- Timeout handling targets the directly invoked runtime process; complete process-tree supervision/recovery is not established.
- No GoreeCloud Identity authorization, Wardveil Security integration, Privacy Shield enforcement, protected secret injection, remote API security, or security-event integration exists.

No production security claim should be inferred from the current source foundation.
