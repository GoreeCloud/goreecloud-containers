# GoreeCloud Containers — Competitive Objectives

GoreeCloud Containers is an original GoreeCloud project. Competitive objectives describe outcomes to pursue, not permission to copy another product's identity, private implementation, interface, or trade dress.

## Objectives

- Preserve standards-based OCI image/runtime/distribution interoperability.
- Make ordinary local execution rootless-first when supported and proven.
- Keep container lifecycle, policy, state, API, networking, storage, and UX under a coherent GoreeCloud contract.
- Provide predictable CLI/API behavior with explicit failure rather than silent fallback or privilege escalation.
- Support Docker/Podman ecosystem artifacts and Docker Compose migration where practical and safe.
- Provide strong inspectability of workload relationships, runtime identity, versions, health, storage, networking, and recovery responsibilities.
- Treat backup/restore, portability, and migration as product requirements rather than afterthoughts.
- Integrate GoreeCloud Identity, Wardveil Security, Privacy Shield, Everkeep, Mesh, Manager, Metrics, and Glaze UI only through evidence-backed contracts.
- Offer native platform management experiences where justified while keeping the engine self-hostable and not dependent on a mandatory proprietary hosted control plane.
- Maintain a small, reviewable, replaceable dependency boundary.

## Current comparison boundary

The current Development source is far narrower than mature Docker or Podman installations. It does not yet provide image pulling, root filesystem construction, accepted runtime execution, networking, volumes, Compose, builds, or production management. Those gaps must be closed through implementation and validation rather than marketing claims.
