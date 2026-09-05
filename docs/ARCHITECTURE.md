# Architecture

## Current Development architecture

```text
goree CLI
   |
   +--> goreecloud-containers-core
   |       container identity
   |       lifecycle state model
   |       Development memory state
   |
   +--> goreecloud-containers-image
   |       registry reference + transport policy
   |       manifest/config/layer retrieval
   |       SHA-256 verification
   |       bounded content-addressed store
   |       restricted layer extraction
   |       staged rootfs construction
   |
   +--> goreecloud-containers-oci
   |       typed minimal OCI config
   |       controlled bundle initialization
   |
   +--> goreecloud-containers-runtime
           crun/runc runtime identity
           runtime probe
           lifecycle command planning
           controlled process execution
```

There is still no daemon, remote API, network manager, volume manager, durable metadata store, accepted rootless path, or accepted production runtime path.

## Ownership boundary

GoreeCloud owns the high-level engine contract and product behavior. Mature standards, protocol, cryptographic, compression/archive, and low-level runtime dependencies remain bounded foundations.

```text
Native/Web GoreeCloud management clients (planned)
                 |
              goree CLI
                 |
       GoreeCloud Container Engine
        /       |        |       \
     images   state   networks  storage
       |         |                 |
 registry +      |          future volume layer
 content store   |
       |         |
   rootfs build  |
        \       /
         OCI bundle/config
                |
          OCI runtime adapter
             /       \
           crun      runc
                |
            Linux kernel
```

## Image/content boundary

`goreecloud-containers-image` currently implements a Development path for supported single-image OCI/Docker manifests. It parses validated registry references, enforces secure network URL rules, handles bounded anonymous Bearer pull challenges, retrieves bounded manifests/configs/layers, and verifies expected SHA-256 content before accepting bytes into the content-addressed store.

The content store is not a general mutable filesystem. It requires an absolute validated root, stages incoming bytes, verifies the expected digest and configured size bound, and publishes verified content under a digest-derived path. Existing content is re-verified before reuse.

The image configuration is parsed to validate Linux OS metadata and layer/diff-ID relationships. The current path does not select OCI image indexes/manifest lists and does not implement reusable registry credential authentication.

## Rootfs construction boundary

The rootfs builder consumes verified layer files and expected uncompressed diff IDs. For each supported layer it:

- validates the absolute regular layer-file path;
- supports OCI uncompressed tar, OCI gzip tar, and Docker gzip layer media types;
- bounds uncompressed bytes, individual entry bytes, and entry count;
- hashes the uncompressed tar stream and requires the expected diff ID;
- rejects unsafe paths and parent traversal;
- rejects symbolic-link parents and current symlink/hard-link archive entries;
- applies supported whiteout semantics without following symlink targets;
- extracts into a newly created staging rootfs;
- publishes only after all layers succeed.

The requested rootfs target must not already exist. This prevents the Development path from silently merging untrusted image content into an existing filesystem tree.

## OCI configuration boundary

`goreecloud-containers-oci` produces a minimal deterministic Linux `config.json` representation targeting OCI Runtime Specification 1.3.0. Bundle initialization is deliberately conservative: the bundle and `rootfs/` must already exist, path endpoints must not be symbolic links, and `config.json` is created without overwrite.

The image pull/rootfs builder and bundle initializer are not yet a single accepted high-level `run` workflow. User-namespace mappings and production OCI policy remain pending.

## Runtime execution boundary

The runtime layer separates command construction from controlled execution. Lifecycle execution requires explicit `crun`/`runc` selection and an absolute executable path, canonicalizes/validates the executable, validates/canonicalizes the bundle for `create`, rejects bundle/config symlink endpoints, directly spawns without a command shell, drains stdout/stderr with bounded retention, applies an execution timeout, and returns non-zero status as an explicit error.

Automated acceptance at this layer currently uses a fake runtime. Real `crun`/`runc` behavior is therefore **unverified by acceptance evidence**.

## State and recovery boundary

`MemoryStateStore` remains Development/test state only. Runtime `state` output is not reconciled into durable GoreeCloud engine metadata. Verified image blobs and constructed Development rootfs trees are currently reconstructible artifacts, not authoritative engine state.

## Next architecture/evidence slices

1. Real external-registry interoperability acceptance across representative OCI/Docker-compatible registries.
2. Controlled integration from a verified pulled image/rootfs into bundle creation.
3. Real `crun`/`runc` lifecycle acceptance.
4. Rootless/user-namespace/resource-boundary acceptance.
5. Crash-safe durable metadata only after its schema and recovery contract are defined.
6. Networking, volumes, high-level lifecycle orchestration, and Compose compatibility after the local image-to-runtime path is proven.

Graphical/native management clients and remote administration follow only after the engine/API boundaries they depend on are stable enough to support them.
