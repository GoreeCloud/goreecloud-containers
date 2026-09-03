# Recovery and Continuity — Development Foundation

## Current state

The current `MemoryStateStore` is intentionally volatile and exists only to establish/test the engine domain model. It is not a backup, database, recovery mechanism, or accepted engine metadata format.

There is currently no durable GoreeCloud Containers state requiring production backup because no durable engine implementation exists yet.

## Required future classification

Before durable state is introduced, the engine must distinguish at least:

- Authoritative workload definitions and engine configuration.
- Durable container/network/volume relationship metadata.
- Persistent user/workload volume data.
- Protected secrets and credential references.
- Reconstructible OCI image blobs/layers and caches.
- Disposable runtime-temporary state.
- Audit/recovery evidence that must be retained.

## Recovery requirements

Production qualification will require:

- Backup of all state classified as required.
- Protected secret recovery through approved mechanisms rather than ordinary exports.
- Clean-target restore testing.
- Validation that restored engine relationships and workloads are usable.
- Explicit handling of content that can be re-pulled/reconstructed instead of unnecessarily treating cache as irreplaceable.
- Rollback and migration validation for engine upgrades.
- Everkeep integration and acceptance where applicable.

A successful source build or a copy of the repository is not recovery evidence for future runtime state.
