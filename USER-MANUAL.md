# GoreeCloud Containers — User Manual

## Current status

GoreeCloud Containers `0.1.0-dev.1` is **Development-only** software for Linux development environments. It is not a production container engine and does not replace Docker in GoreeCloud production.

The current user-facing surface is the `goree` Development CLI. The lifecycle commands are low-level engineering interfaces intended to exercise the evolving OCI engine boundary.

## Prerequisites

To build the repository you need the pinned Rust 1.85.0 toolchain described by `rust-toolchain.toml`.

For `runtime probe`, the selected runtime executable must be installed and reachable by the path/name you provide.

For lifecycle execution commands:

- Linux is the current supported Development platform.
- You must provide `crun` or `runc` as the selected runtime kind.
- You must provide an **absolute path** to the runtime executable.
- The executable must resolve to a regular executable file.
- `create` requires an **absolute path** to an already prepared OCI bundle with a regular `config.json`.
- GoreeCloud Containers does not yet pull images or build a root filesystem for you.

Real `crun`/`runc` lifecycle behavior has not yet passed GoreeCloud acceptance testing. Use these commands only in disposable Development/test environments.

## Build

From the repository root:

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

## Probe a runtime

Default `crun` from `PATH`:

```bash
cargo run -p goree -- runtime probe
```

Explicit runtime kind:

```bash
cargo run -p goree -- runtime probe runc
```

Explicit executable value:

```bash
cargo run -p goree -- runtime probe crun /usr/bin/crun
```

A successful probe proves only that the executable ran and returned version output. It does not prove container lifecycle compatibility or production readiness.

## Initialize an OCI bundle configuration

First create an absolute bundle directory and populate an existing `rootfs/` directory yourself. Then initialize `config.json`:

```bash
cargo run -p goree -- bundle init /absolute/path/to/bundle /bin/echo "Hello from GoreeCloud"
```

The command:

- requires the bundle path to be absolute;
- rejects a bundle path whose endpoint is a symbolic link;
- requires `rootfs/` to already exist as a directory and not be a symbolic-link endpoint;
- refuses to overwrite an existing `config.json`.

The generated Development configuration targets OCI Runtime Specification 1.3.0, enables `noNewPrivileges`, defaults the container process to UID/GID 65534, uses `/` as the container working directory, and declares PID/network/IPC/UTS/mount namespaces. These defaults are not an accepted production or rootless policy.

## Execute low-level lifecycle operations

Use an absolute executable path. Example forms:

```bash
cargo run -p goree -- runtime create crun /usr/bin/crun example /absolute/path/to/bundle
cargo run -p goree -- runtime start crun /usr/bin/crun example
cargo run -p goree -- runtime state crun /usr/bin/crun example
cargo run -p goree -- runtime delete crun /usr/bin/crun example
```

The current executor invokes the selected runtime directly, uses null stdin, captures stdout/stderr with a bounded retained size, applies a timeout, and surfaces non-zero exits as errors.

`state` currently prints the selected runtime's output; GoreeCloud does not yet normalize that output into a durable engine state model.

## Safety notes

- Do not point the Development lifecycle interface at important production workloads.
- Runtime execution can create and manipulate processes/containers according to the privileges of the caller and runtime. Use only an isolated test environment.
- The current source does not establish rootless safety. Do not assume an unprivileged container process UID/GID means the host runtime is rootless.
- The current timeout terminates the directly invoked runtime process; this is not yet a complete process-tree supervision or recovery contract.
- `config.json` creation is protected against overwrite, but the bundle is not an accepted durable engine-state format.
- Do not place secrets in repository-controlled examples or ordinary diagnostic output.

## Current limitations

Not implemented or not accepted yet:

- OCI image pull, registry authentication, digest verification, layer storage, or root filesystem construction.
- Real `crun`/`runc` lifecycle acceptance and OCI conformance evidence.
- Rootless acceptance, user namespace mapping, networking, volumes, cgroups/resource policy, health checks, and restart policy.
- Durable engine database, backup/restore acceptance, daemon, or remote API.
- Compose, Docker API compatibility, or image builds.
- Graphical/native management clients.
- Accepted GoreeCloud Manager, Identity, Wardveil Security, Privacy Shield, Everkeep, Mesh, or Glaze UI runtime integration.
- Production deployment, Stable qualification, or Docker replacement.

## Troubleshooting

**`OCI runtime executable path must be absolute`** — provide a complete path such as `/usr/bin/crun` for lifecycle execution.

**`OCI bundle path must be absolute`** — use a full filesystem path for bundle initialization/create.

**`rootfs` inspection error** — create and populate the bundle's `rootfs/` directory before `bundle init`.

**`refusing to overwrite existing OCI configuration`** — the command intentionally does not replace `config.json`; inspect/remove it manually only if doing so is safe for your disposable test bundle.

**runtime timeout/non-zero exit** — inspect the bounded error output and the runtime/host configuration. A timeout or runtime failure is surfaced rather than silently switching runtimes.

## Related documentation

See `README.md`, `SPECIFICATIONS.md`, `docs/ARCHITECTURE.md`, `docs/SECURITY.md`, and `docs/RECOVERY.md` for deeper Development details.
