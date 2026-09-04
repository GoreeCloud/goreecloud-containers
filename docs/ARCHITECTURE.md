# Architecture

## Current Development architecture

```text
goree CLI
   |
   +--> goreecloud-containers-core
   |       container identity
   |       lifecycle state model
   |       development memory state
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

There is still no daemon, remote API, OCI image store/puller, network manager, volume manager, durable metadata store, or accepted production runtime path.

## Ownership boundary

GoreeCloud owns the high-level engine contract and product behavior. Mature OCI runtimes are bounded low-level dependencies used for process/container execution.

```text
Native/Web GoreeCloud management clients (planned)
                 |
              goree CLI
                 |
       GoreeCloud Container Engine
        /       |        |       \
     images   state   networks  storage
                 |
          OCI runtime adapter
             /       \
           crun      runc
                 |
             Linux kernel
```

## OCI configuration boundary

`goreecloud-containers-oci` currently produces a minimal deterministic Linux `config.json` representation targeting OCI Runtime Specification 1.3.0. Bundle initialization is deliberately conservative: the bundle and `rootfs/` must already exist, path endpoints must not be symbolic links, and `config.json` is created without overwrite.

The crate does not pull or unpack images, create `rootfs/`, establish user-namespace mappings, or claim a complete production OCI policy.

## Runtime execution boundary

The runtime layer separates command construction from controlled execution. Lifecycle execution:

- requires an explicit `crun`/`runc` runtime kind;
- requires an absolute executable path and canonicalizes it;
- requires a regular executable file;
- validates and canonicalizes the bundle for `create`;
- rejects bundle/config symbolic-link endpoints;
- spawns the runtime directly without a command shell;
- drains stdout/stderr concurrently while retaining bounded output;
- applies an execution timeout and attempts to terminate the invoked process;
- returns non-zero status as an explicit error.

Automated acceptance at this layer currently uses a fake runtime. Real `crun`/`runc` behavior is therefore **unverified**, not implemented-by-evidence.

## State boundary

`MemoryStateStore` remains Development/test state only. Runtime `state` output is not yet reconciled into durable GoreeCloud engine metadata. No current runtime state should be represented as recoverable.

## Next architecture slice

1. OCI registry reference resolution and manifest/config retrieval.
2. Digest verification before accepting content.
3. Bounded/content-addressed blob storage.
4. Safe layer unpacking with path/link/device protections.
5. Root-filesystem construction for a validated bundle.
6. Real runtime/rootless acceptance as separate evidence gates.
7. Crash-safe durable metadata only after its schema/recovery contract is defined.

Networking, volumes, Compose, builds, remote management, and graphical clients follow after a complete local image-to-lifecycle path is proven.
