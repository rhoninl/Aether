# Creator Studio Editor Logic

## Background

The `aether-creator-studio` crate currently defines type contracts (EditorMode, TerrainTool, GizmoMode, PropPlacement, etc.) but contains no actual editor logic. This document designs the implementation of the editor subsystems that operate on those types.

## Why

Without editor logic, the Creator Studio types are inert data structures. Users cannot:
- Sculpt or paint terrain
- Place/move/rotate props with gizmos and grid snap
- Configure lighting
- Edit or validate a world manifest
- Undo/redo any operation
- Select or multi-select scene objects

## What

Implement six subsystems:

1. **Undo/Redo** - Generic command pattern stack for all editor operations
2. **Selection** - Object selection system (single, multi, toggle, clear)
3. **Terrain editor** - Sculpt (raise/lower/smooth/flatten), paint, vegetation placement
4. **Prop placer** - Place, move, rotate, scale props with grid snap support
5. **Lighting editor** - Place light probes, configure ambient settings
6. **World manifest editor** - Create, validate, and patch world manifests

## How

### Architecture

All editor operations are expressed as **commands** implementing an `EditorCommand` trait. The `UndoStack` holds executed commands and supports undo/redo. Every subsystem produces commands rather than mutating state directly, enabling full undo support.

A central `EditorScene` holds all mutable scene state: objects, terrain data, selection, and lighting configuration.

### Module layout

```
src/
  lib.rs            - Re-exports (updated)
  editor.rs         - EditorMode, EditorEvent (existing, unchanged)
  manifest.rs       - Manifest types (existing, unchanged)
  preview.rs        - Preview types (existing, unchanged)
  terrain.rs        - TerrainBrush, SculptBrush (existing, unchanged)
  tools.rs          - GizmoMode, PropPlacement (existing, unchanged)
  undo.rs           - EditorCommand trait + UndoStack
  selection.rs      - Selection + SelectionCommand
  terrain_editor.rs - Terrain sculpt/paint/vegetation commands
  prop_editor.rs    - Prop placement/manipulation commands
  lighting_editor.rs - Light probe + ambient commands
  manifest_editor.rs - World manifest create/validate/patch
  scene.rs          - EditorScene, SceneObject, ObjectId
```

### Detail Design

#### Core types (`scene.rs`)

```
ObjectId = u64
SceneObject { id, name, kind, position, rotation, scale }
ObjectKind { Prop, Light, SpawnPoint, Vegetation }
Position { x, y, z }
Rotation { yaw_deg, pitch_deg, roll_deg }
EditorScene { objects, selection, terrain, lighting, next_id }
```

#### Undo/Redo (`undo.rs`)

```
trait EditorCommand: Send + Sync
  execute(&mut self, scene) -> Result<()>
  undo(&mut self, scene) -> Result<()>
  description() -> &str

UndoStack { undo_stack, redo_stack, max_history }
  push(cmd) - executes and pushes
  undo(scene) - pops undo, pushes to redo
  redo(scene) - pops redo, pushes to undo
  can_undo() / can_redo()
  clear()
```

When a new command is pushed, the redo stack is cleared (standard undo/redo semantics).

#### Selection (`selection.rs`)

```
Selection { selected: HashSet<ObjectId> }
  select(id)
  deselect(id)
  toggle(id)
  select_all(ids)
  clear()
  is_selected(id) -> bool
  count() -> usize

SelectCommand - selects one object (undoable)
DeselectCommand - deselects one object (undoable)
ClearSelectionCommand - clears all (undoable, stores previous selection)
```

#### Terrain editor (`terrain_editor.rs`)

Operations apply to a `TerrainData` heightmap grid.

```
TerrainData { width, height, heightmap: Vec<f32>, paint_layers }
PaintLayer { texture_id, weights: Vec<f32> }

SculptCommand { brush, center, previous_heights }
PaintCommand { layer_idx, center, radius, intensity, previous_weights }
PlaceVegetationCommand { position, template_id }
```

Sculpt modifies heightmap values within brush radius using falloff. Paint modifies texture weight for a layer. Both store previous values for undo.

#### Prop editor (`prop_editor.rs`)

```
PlacePropCommand { template, position, rotation, snap_to_grid, grid_size }
MovePropCommand { object_id, new_position, old_position }
RotatePropCommand { object_id, new_rotation, old_rotation }
ScalePropCommand { object_id, new_scale, old_scale }
DeletePropCommand { object_id, removed_object }
```

Grid snap: when enabled, position coordinates are rounded to nearest `grid_size` multiple.

#### Lighting editor (`lighting_editor.rs`)

```
AmbientSettings { color_r, color_g, color_b, intensity }
LightProbe { id, position, radius, intensity }

PlaceLightProbeCommand { position, radius, intensity }
RemoveLightProbeCommand { probe_id, removed_probe }
SetAmbientCommand { new_settings, old_settings }
```

#### Manifest editor (`manifest_editor.rs`)

```
WorldManifest { world_id, name, description, physics, spawn_points, max_players }
ManifestValidationError { field, message }

create_default_manifest(world_id, name) -> WorldManifest
validate_manifest(manifest) -> Result<(), Vec<ManifestValidationError>>
apply_patch(manifest, patch) -> Result<WorldManifest>
```

Validation rules:
- world_id must not be empty
- name must be 1-100 chars
- max_players must be 1-1000
- gravity must be -100..100 range
- at least one spawn point required

### Test Design

All tests are in-memory with no external dependencies.

- **undo.rs**: push/undo/redo ordering, max history eviction, clear, redo cleared on new push
- **selection.rs**: select/deselect/toggle/clear/multi-select, command undo
- **terrain_editor.rs**: sculpt raise/lower modifies heightmap, paint modifies weights, undo restores
- **prop_editor.rs**: place adds object, move/rotate/scale/delete with undo, grid snap rounding
- **lighting_editor.rs**: place/remove probes, ambient settings with undo
- **manifest_editor.rs**: create default, validate passes/fails, apply patch

### Dependencies

```toml
serde = { version = "1", features = ["derive"] }
```

No uuid dependency needed; we use a simple `u64` counter for ObjectId.
