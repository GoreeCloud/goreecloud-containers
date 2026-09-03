# Architecture

## Current Development architecture

The repository is intentionally small and layered:

```text
goree CLI
   |
   +--> goreecloud-containers-core
   |       container identity
   |       lifecycle state model
   |       development state store
   |
   +--> goreecloud-containers-runtime
           crun/runc runtime identity
           runtime probe
           OCI lifecycle command planning
```

No daemon, remote API, image store, network manager, volume manager, OCI bundle generator, or real container execution path exists yet.

## Ownership boundary

GoreeCloud Containers is not intended to be a wrapper around Docker or Podman. GoreeCloud owns the high-level engine contract and product behavior. Mature OCI runtimes are bounded low-level dependencies used for process/container execution.

The planned boundary is:

```text
Native/Web GoreeCloud management clients
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

## Initial safety decisions

- Rootless operation is the intended normal mode where technically possible.
- Runtime executable selection is explicit.
- The initial runtime adapter only probes versions and plans lifecycle commands; it does not yet execute `create`, `start`, `state`, or `delete`.
- OCI bundle paths are required to be absolute by command planning to avoid current-working-directory ambiguity.
- Container identifiers are validated before becoming engine identities.
- State transitions are explicit rather than arbitrary string mutation.
- The current in-memory store is development/test state only and must not be represented as durable or recoverable.

## Next architecture slice

The next implementation should add:

1. A typed OCI bundle/config model.
2. Filesystem ownership and permission checks around a controlled bundle root.
3. A runtime executor separate from command planning.
4. Exact create/start/state/delete result handling with bounded output and timeouts.
5. Crash-safe durable metadata only after its schema and recovery contract are defined.
6. OCI image manifest/config/layer retrieval and digest verification.

Networking, volumes, Compose, builds, remote management, and graphical clients follow after a complete local lifecycle is proven.
