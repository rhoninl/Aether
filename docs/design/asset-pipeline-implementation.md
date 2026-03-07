# Asset Pipeline & Bundle Format (task-017)

Added pipeline contracts for format descriptors, compression targets, import translators, and LOD metadata.

## Implemented API surface

- Added crate `aether-asset-pipeline` with bundle/manifest/import/compression/lod modules.
- Added binary format descriptors for .aemesh/.aeenv and transcode targets.
- Added LOD and progression policies and task states.
- Added import source descriptors for FBX/GLTF/OBJ.
- Updated workspace membership.

## Remaining implementation work

- Implement actual encoders/decoders and streaming package generation.
- Wire to renderer/loader and compression toolchains.
