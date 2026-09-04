# Recovery and Continuity — Development Foundation

## Current state

`MemoryStateStore` remains volatile Development/test state and is not a database or recovery mechanism. The new bundle initializer writes a Development `config.json` into a caller-provided test bundle, but that bundle is not a GoreeCloud durable engine-state contract.

There is still no durable GoreeCloud Containers metadata implementation or accepted runtime-data recovery workflow.

## Bundle write behavior

Bundle initialization refuses to overwrite an existing `config.json`, creates a new file with create-new semantics, writes the generated configuration, and synchronizes the file. This reduces accidental replacement but does not by itself establish crash-safe transactional engine metadata, directory durability, backup, or restore acceptance.

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

Production qualification will require clean-target restoration, workload/relationship validation, protected secret recovery, migration/rollback validation, cache-vs-critical-state classification, and accepted Everkeep integration where applicable.

A passing source build, generated test bundle, or repository copy is not recovery evidence for future engine runtime state.
