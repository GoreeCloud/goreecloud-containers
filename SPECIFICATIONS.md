# GoreeCloud Containers — Repository Specifications

## Status

**Lifecycle:** Development  
**Source version:** `0.1.0-dev.1`  
**Primary implementation language:** Rust  
**Current development platform:** Linux  
**Production status:** Not production-ready

This repository specification describes the implemented Development source boundary plus the immediate engineering requirements. The broader canonical product specification remains the GoreeCloud project record. Planned capabilities are not current implementation unless explicitly identified below.

## Product boundary

GoreeCloud Containers is intended to be a first-party OCI-compatible container engine and workload platform. GoreeCloud owns the high-level engine contract, CLI/API direction, state model, policy, workload lifecycle, networking/storage orchestration, and platform integrations. Mature OCI runtimes remain bounded low-level execution foundations.

Docker and Podman interoperability is a goal; copying their private implementation architecture or product identity is not.

## Current workspace

The Rust workspace contains:

- `goreecloud-containers-core`: container identifiers, lifecycle states, transitions, and Development memory state.
- `goreecloud-containers-oci`: typed minimal OCI Linux configuration and controlled bundle initialization.
- `goreecloud-containers-runtime`: `crun`/`runc` runtime selection, probing, command planning, validation, and controlled lifecycle process execution.
- `goree`: current Development CLI.

Unsafe Rust is forbidden at the workspace lint level.

## OCI bundle/config foundation

The current OCI crate:

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

This is a Development configuration foundation. It is not yet an accepted rootless profile, production security profile, or complete OCI compatibility implementation.

## Runtime execution foundation

Lifecycle execution currently supports the low-level operations:

- `create`
- `start`
- `state`
- `delete`

Execution requires an explicitly selected runtime kind (`crun` or `runc`) and an absolute runtime executable path. The executable is canonicalized and must resolve to a regular executable file. `create` additionally validates an absolute bundle, rejects symbolic-link bundle/config endpoints, canonicalizes the bundle, and requires a regular `config.json`.

The executor:

- invokes the runtime directly rather than through a shell;
- provides null stdin;
- captures stdout and stderr concurrently;
- retains at most 1 MiB per output stream by default while continuing to drain the stream;
- applies a 30-second default timeout;
- attempts to terminate the invoked runtime process on timeout;
- preserves non-zero exit status and bounded error output.

Automated lifecycle tests use a deterministic fake executable. They validate the GoreeCloud execution boundary, not real OCI runtime conformance.

## Current CLI contract

Development commands currently include:

```text
goree version
goree bundle init <absolute-bundle> <command> [args...]
goree runtime probe [crun|runc] [executable]
goree runtime create <crun|runc> <absolute-executable> <container-id> <absolute-bundle>
goree runtime start <crun|runc> <absolute-executable> <container-id>
goree runtime state <crun|runc> <absolute-executable> <container-id>
goree runtime delete <crun|runc> <absolute-executable> <container-id>
goree container validate-id <container-id>
```

These interfaces are Development-only and may change before a stable CLI contract is declared.

## Current non-goals and missing capabilities

The current source does not provide:

- OCI registry authentication or manifest resolution;
- image digest/layer verification and content-addressed image storage;
- image-layer unpacking or root-filesystem construction;
- accepted real `crun`/`runc` lifecycle tests;
- accepted rootless execution, user-namespace mapping, cgroup policy, or network setup;
- persistent engine metadata or a recovery-accepted database;
- networks, volumes, port publishing, service discovery, or resource policy;
- health/restart management;
- daemon or versioned remote API;
- Compose or Docker API compatibility;
- image builds;
- graphical/native management clients;
- accepted GoreeCloud Manager, Identity, Wardveil Security, Privacy Shield, Everkeep, Mesh, or Glaze UI runtime integration;
- production deployment, Stable qualification, or Docker replacement.

## Validation requirements

Every material source candidate must pass formatting, Clippy with warnings denied, unit/tests, workspace build, and applicable GoreeCloud Platform Contract validation. Real runtime, rootless, recovery, networking, storage, compatibility, and production claims require separate target-environment evidence.
