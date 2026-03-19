# Creator Studio: Core Types & Scene Serialization

## Background

The Aether Creator Studio (`aether-creator-studio`) currently provides 3D-only editor primitives (scene objects, manifests, lighting, terrain, etc.). To support both 2D and 3D world creation, we need core dimension-aware types, 2D-specific primitives, and serialization for scene/project files.

## Why

- Creators need to choose between 2D and 3D at world creation time.
- 2D worlds require dedicated types: sprites, tilemaps, 2D physics, 2D lights.
- Scenes and project manifests must serialize/deserialize for persistence (save/load).
- The existing 3D scene types lack a serializable scene format for file I/O.

## What

1. **WorldDimension enum** - 2D vs 3D discriminator, immutable after creation.
2. **2D types** - Transform2D, Collider2D, RigidBody2D, Light2D, SpriteEntity, SpriteSheetDef, TilemapData, TilesetDef.
3. **Scene serialization** - Scene3D/Scene2D structs with JSON round-trip (serialize/deserialize).
4. **World project manifest** - WorldProjectManifest with dimension-aware config, validation.

## How

### Module Layout

```
src/
  dimension.rs       - WorldDimension enum
  types_2d.rs        - All 2D-specific types
  scene_serde.rs     - Scene3D, Scene2D, serialization functions
  project.rs         - WorldProjectManifest, validation
  lib.rs             - Updated with new module exports
```

### Serialization Strategy

The crate has `serde` + `serde_json` but no `toml` dependency. Since Cargo.toml must not be modified, all serialization uses JSON via `serde_json`. The public API uses `Result<String, SerdeError>` wrapping `serde_json` errors.

### Validation (project.rs)

`validate_manifest` checks:
- name not empty
- version is valid semver (major.minor.patch format)
- gravity dimensions match world dimension (2 for 2D, 3 for 3D)
- tick_rate in valid range (1..=240)
- max_players > 0
- scenes list not empty
- default scene is in the scenes list

### Test Design

Tests are written first in each module's `#[cfg(test)] mod tests` block:
- Round-trip serialization (serialize then deserialize, assert equality)
- Edge cases (empty collections, missing optionals, boundary values)
- Validation error coverage (every validation rule has a failing test case)
