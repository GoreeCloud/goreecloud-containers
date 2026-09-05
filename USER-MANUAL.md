# GoreeCloud Containers — User Manual

## Current status

GoreeCloud Containers `0.1.0-dev.2` is **Development-only** software for Linux development environments. It is not a production container engine and does not replace Docker in GoreeCloud production.

The current user-facing surface is the `goree` Development CLI. Image retrieval/rootfs construction and OCI lifecycle commands are engineering interfaces used to exercise the evolving engine boundary.

## Prerequisites

To build the repository you need the pinned Rust 1.85.0 toolchain described by `rust-toolchain.toml`.

For network image retrieval:

- use an HTTPS registry base URL for non-loopback registries;
- loopback HTTP is permitted only for local Development/testing use;
- the current client supports anonymous access and bounded Bearer-token challenges used for pull access;
- registry username/password or other reusable-credential authentication is not implemented;
- only supported single-image OCI/Docker manifests are handled; image indexes/manifest lists are not selected.

For runtime execution, the selected `crun` or `runc` executable must be installed. Lifecycle execution requires an absolute executable path. Real `crun`/`runc` lifecycle behavior has not yet passed GoreeCloud acceptance testing.

## Build

```bash
cargo build --workspace
```

## Show the Development version

```bash
cargo run -p goree -- version
```

## Validate a container identifier

```bash
cargo run -p goree -- container validate-id example-container
```

A valid ID begins with an ASCII alphanumeric character, uses only ASCII alphanumeric characters plus `-`, `_`, or `.`, and is no longer than 128 characters.

## Verify and ingest local image content

`image ingest` verifies a local file against an expected lowercase SHA-256 digest before accepting it into the Development content-addressed store:

```bash
cargo run -p goree -- image ingest \
  sha256:<64-lowercase-hex> \
  /absolute/path/to/source-blob \
  /absolute/path/to/content-store
```

An optional final argument overrides the default maximum accepted content size:

```bash
cargo run -p goree -- image ingest \
  sha256:<64-lowercase-hex> \
  /absolute/path/to/source-blob \
  /absolute/path/to/content-store \
  104857600
```

The store root must already exist as an absolute non-symlink directory. A digest mismatch or size-limit violation is an error and does not publish the candidate as verified content.

## Pull a Development image and construct a rootfs

```bash
cargo run -p goree -- image pull \
  https://registry.example \
  team/example \
  v1 \
  /absolute/path/to/content-store \
  /absolute/path/to/new-rootfs
```

A SHA-256 manifest digest may be used instead of a tag. On success the command prints the verified manifest/config digests, layer count, image OS/architecture metadata, and resulting rootfs path.

The pull path currently:

- retrieves a supported OCI Image Manifest or Docker Registry v2 image manifest;
- retrieves its image configuration and supported layers;
- verifies expected SHA-256 descriptor digests before content-store acceptance;
- verifies uncompressed layer diff IDs from the image configuration;
- applies supported tar/gzip layers into a staging rootfs;
- applies supported OCI whiteouts;
- publishes the rootfs only after all layers succeed.

The rootfs target must be an absolute path that does **not** already exist. Its parent must resolve canonically without a symbolic-link endpoint. The restricted extractor rejects parent traversal, symbolic-link traversal, currently unsupported symlink/hard-link archive entries, excessive entry sizes/counts, and excessive uncompressed layer size.

This command does not create a complete GoreeCloud container record, configure networking, generate a finished bundle automatically, or start a runtime. Real external-registry interoperability acceptance is still pending.

## Probe a runtime

```bash
cargo run -p goree -- runtime probe
cargo run -p goree -- runtime probe runc
cargo run -p goree -- runtime probe crun /usr/bin/crun
```

A successful probe proves only that the executable ran and returned version output. It does not prove lifecycle compatibility or production readiness.

## Initialize an OCI bundle configuration

Create an absolute bundle directory with an existing `rootfs/` directory, then initialize `config.json`:

```bash
cargo run -p goree -- bundle init /absolute/path/to/bundle /bin/echo "Hello from GoreeCloud"
```

The command requires an absolute bundle path, rejects symbolic-link bundle/rootfs endpoints, requires `rootfs/` to exist, and refuses to overwrite `config.json`.

The generated Development configuration targets OCI Runtime Specification 1.3.0, enables `noNewPrivileges`, defaults the container process to UID/GID 65534, uses `/` as the container working directory, and declares PID/network/IPC/UTS/mount namespaces. These defaults are not an accepted production or rootless policy.

The current `image pull` target is a standalone rootfs path; integrating a pulled rootfs into a complete bundle remains a separate Development workflow rather than a high-level `goree run` operation.

## Execute low-level lifecycle operations

```bash
cargo run -p goree -- runtime create crun /usr/bin/crun example /absolute/path/to/bundle
cargo run -p goree -- runtime start crun /usr/bin/crun example
cargo run -p goree -- runtime state crun /usr/bin/crun example
cargo run -p goree -- runtime delete crun /usr/bin/crun example
```

The executor invokes the selected runtime directly, uses null stdin, captures stdout/stderr with bounded retained size, applies a timeout, and surfaces non-zero exits as errors.

`state` prints the selected runtime's output; GoreeCloud does not yet normalize that output into durable trusted engine state.

## Safety notes

- Use the Development interfaces only in disposable/isolated test environments.
- Do not point lifecycle commands at important production workloads.
- Do not treat successful fixture-registry tests as proof that arbitrary real registries are accepted.
- Do not place reusable registry credentials in command arguments; credential authentication is not implemented.
- Digest verification establishes byte integrity relative to expected digest values, not publisher identity, signature trust, provenance, or vulnerability status.
- Rootfs extraction intentionally rejects unsupported archive entry types rather than broadening its attack surface silently.
- Runtime execution can create/manipulate processes according to caller/runtime privileges; current source does not establish rootless safety.
- UID/GID 65534 inside generated config does not prove host-level rootless execution.
- The runtime timeout terminates the directly invoked process; complete process-tree supervision/recovery is not established.
- `config.json`, the Development content store, and constructed rootfs are not accepted durable engine-state contracts.

## Current limitations

Not implemented or not accepted yet:

- Registry reusable-credential authentication and private-registry credential handling.
- Image-index/manifest-list multi-platform selection.
- Symlink/hard-link layer entry extraction.
- Signature/attestation/SBOM/provenance trust policy.
- Real external-registry acceptance.
- Real `crun`/`runc` lifecycle acceptance and OCI conformance evidence.
- Rootless acceptance, user namespace mapping, networking, volumes, cgroups/resource policy, health checks, and restart policy.
- Durable engine database, backup/restore acceptance, daemon, or remote API.
- High-level run/inspect/logs/remove lifecycle orchestration.
- Compose, Docker API compatibility, or image builds.
- Graphical/native management clients.
- Accepted GoreeCloud Manager, Identity, Wardveil Security, Privacy Shield, Everkeep, Mesh, or Glaze UI runtime integration.
- Production deployment, Stable qualification, or Docker replacement.

## Troubleshooting

**digest mismatch** — verify the expected SHA-256 value and source/registry content. Mismatched bytes are rejected.

**insecure registry URL** — use HTTPS for a non-loopback registry. HTTP is restricted to loopback Development/test use.

**rootfs target already exists** — choose a new absolute target. The Development builder refuses to merge into an existing rootfs.

**unsafe archive path / symbolic-link parent / unsupported entry type** — the restricted extractor rejected the layer. Do not disable these checks for convenience.

**`OCI runtime executable path must be absolute`** — provide a complete path such as `/usr/bin/crun` for lifecycle execution.

**`OCI bundle path must be absolute`** — use a full filesystem path for bundle initialization/create.

**`refusing to overwrite existing OCI configuration`** — inspect/remove `config.json` manually only when safe for the disposable test bundle.

**runtime timeout/non-zero exit** — inspect the bounded error output and runtime/host configuration. The engine surfaces the failure instead of silently switching runtimes.

## Related documentation

See `README.md`, `SPECIFICATIONS.md`, `docs/ARCHITECTURE.md`, `docs/SECURITY.md`, `docs/DEPENDENCIES.md`, and `docs/RECOVERY.md` for deeper Development details.
