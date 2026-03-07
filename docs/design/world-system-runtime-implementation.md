# World System Runtime (task-024)

Added world runtime primitives for parsing manifests, streaming chunks, boot lifecycle, and runtime settings.

## Implemented API surface

- Added crate `aether-world-runtime` with modules:
  - `manifest`: runtime manifest + validation.
  - `chunking`: chunk descriptors and LOD-aware streaming policy.
  - `props`: terrain chunks, prop instances, lighting setup, spawn points.
  - `spawn`: runtime setting validation + lifecycle enums.
  - `lifecycle`: state transitions and lifecycle events.
- Updated workspace membership for `aether-world-runtime`.

## Mapping to acceptance criteria

- `#1` manifest parsing and validation model in `WorldRuntimeManifest` / `validate_runtime_manifest`.
- `#2` chunk-based terrain streaming represented by `ChunkDescriptor` / `ChunkStreamingPolicy`.
- `#3` prop placement from manifest via `PropInstance` and `TileLayer`.
- `#4` lighting/skybox representation via `LightingSetup` and `.aeenv` path field.
- `#5` spawn management through `SpawnPoint` and manifest spawn count.
- `#6` lifecycle represented by `RuntimeState`, `WorldLifecycleEvent` contracts.
- `#7` runtime setting enforcement modeled by `RuntimeSettings` / `RuntimeSettingsError`.

## Remaining implementation work

- Add authoritative world bootstrap execution and cleanup hooks.
- Connect streaming/asset load to renderer and network systems.
- Add runtime mutation APIs for dynamic setting changes.
