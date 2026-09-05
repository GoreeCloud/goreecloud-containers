# Recovery and Continuity — Development Foundation

## Current state

`MemoryStateStore` remains volatile Development/test state and is not a database or recovery mechanism. Bundle initialization writes a Development `config.json` into a caller-provided test bundle, but that bundle is not a GoreeCloud durable engine-state contract.

The image/content path now creates verified content-addressed blobs and can construct a Development rootfs from verified supported layers. These artifacts are currently **reconstructible image/cache state**, not authoritative workload definitions or accepted durable engine metadata.

There is still no durable GoreeCloud Containers metadata implementation or accepted runtime-data recovery workflow.

## Image/content recovery classification

At the current Development boundary:

- Registry manifests, configs, and layers in the content store are reconstructible when their upstream source remains available and trustworthy.
- Constructed rootfs trees are reconstructible from verified image metadata/layers and are not authoritative user data.
- Content digests provide integrity identifiers but do not replace backup/recovery requirements for future authoritative state.
- A rootfs build is staged and published only after all supported layers verify/apply; partial staging directories are not an accepted recovery format.

Future garbage collection, cache eviction, offline-retention, and registry-loss policy must distinguish reconstructible cache content from content deliberately retained for continuity.

## Bundle write behavior

Bundle initialization refuses to overwrite an existing `config.json`, creates a new file with create-new semantics, writes the generated configuration, and synchronizes the file. This reduces accidental replacement but does not by itself establish crash-safe transactional engine metadata, directory durability, backup, or restore acceptance.

## Required future classification

Before durable state is introduced, the engine must distinguish at least:

- Authoritative workload definitions and engine configuration.
- Durable container/network/volume relationship metadata.
- Persistent user/workload volume data.
- Protected secrets and credential references.
- Reconstructible OCI image manifests/configs/blobs/layers and derived rootfs/cache data.
- Disposable runtime-temporary state.
- Audit/recovery evidence that must be retained.

## Recovery requirements

Production qualification will require clean-target restoration, workload/relationship validation, protected secret recovery, migration/rollback validation, cache-vs-critical-state classification, and accepted Everkeep integration where applicable.

A passing source build, verified cache blob, constructed Development rootfs, generated test bundle, or repository copy is not recovery evidence for future authoritative engine runtime state.
