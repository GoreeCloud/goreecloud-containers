# Material Dependencies

## Rust toolchain

Purpose: build the GoreeCloud-owned engine libraries and CLI.  
Current repository minimum/pin: Rust 1.85.0, Edition 2024.  
Boundary: build/language toolchain; it does not define GoreeCloud Containers product behavior.  
Update path: deliberate toolchain updates through reviewed source changes and CI.

The current Rust crates intentionally have no third-party Cargo package dependencies.

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

Every material dependency added later must document purpose, necessity, version/compatibility expectations, licensing, security/privacy implications, failure behavior, and replacement path.
