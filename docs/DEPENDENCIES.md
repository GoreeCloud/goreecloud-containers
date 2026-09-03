# Material Dependencies

## Rust toolchain

Purpose: build the GoreeCloud-owned engine libraries and CLI.  
Current repository minimum/pin: Rust 1.85.0, Edition 2024.  
Boundary: build and language toolchain; it does not define GoreeCloud Containers product behavior.  
Update path: deliberate toolchain updates through reviewed source changes and CI.

The current Rust crates intentionally have no third-party Cargo package dependencies.

## crun

Purpose: preferred mature OCI low-level runtime candidate.  
Current use: optional explicit `goree runtime probe crun`; no container lifecycle execution is implemented.  
Architectural boundary: low-level OCI process/container execution only. GoreeCloud retains high-level lifecycle, state, policy, API, networking, storage, and UX ownership.  
License/security: crun retains its own license and security lifecycle. Distribution and dependency review are required before GoreeCloud packages or bundles it.  
Failure implication: when selected, unavailable or failing crun must fail the requested runtime operation; the engine must not silently elevate or fall back to a different runtime.

## runc

Purpose: alternative mature OCI low-level runtime candidate.  
Current use: optional explicit `goree runtime probe runc`; no container lifecycle execution is implemented.  
Architectural boundary: same low-level boundary as crun.  
License/security: runc retains its own license and security lifecycle.  
Failure implication: an explicitly selected runc failure must be surfaced rather than silently replaced.

## Linux kernel facilities

Future execution will rely on operating-system container primitives such as namespaces, cgroups, filesystems, capabilities, seccomp, and security modules. These are foundational platform facilities, not components GoreeCloud should reimplement merely for ownership.

Every material dependency added later must document purpose, necessity, version/compatibility expectations, licensing, security/privacy implications, failure behavior, and replacement path.
