# CLI World Management Commands

## Background

The Aether CLI (`aether-cli`) currently supports basic commands: `new`, `check`, `serve`, `run`, and `version`. The in-world editor design (see `in-world-editor.md`) introduces a dimension-aware world project format (2D/3D) and a version management lifecycle. The CLI needs to be updated to support these new concepts.

## Why

- Creators need to specify world dimension (2D or 3D) at project creation time
- The old flat `world.toml` format and Lua-based scaffolding are obsolete
- Version management (`publish`, `versions`) is required for the world lifecycle
- A `world` subcommand group provides a clean namespace for world management commands

## What

Update the `aether-cli` crate to:

1. Add `dimension` field to `WorldManifest` with "2D"/"3D" validation
2. Update `new` command to scaffold dimension-aware project structures
3. Update `check` command for dimension-aware validation
4. Add `world` subcommand group with `new`, `check`, `publish`, `versions`
5. Maintain backward compatibility (`aether new` = `aether world new`)

## How

### Manifest Changes

The `WorldManifest` struct gains a `dimension` field. The `default_for` method takes a dimension parameter. Validation ensures dimension is "2D" or "3D".

The world.toml format changes from flat keys to nested TOML tables (`[world]`, `[physics]`, `[players]`, `[scenes]`).

### Scaffold Changes

The `new` command accepts `--2d` or `--3d` flags (default `--3d`). Each dimension gets a tailored directory structure:

- 3D: `scenes/`, `scripts/`, `assets/{meshes,textures,audio}`, `terrain/`, `.aether/`
- 2D: `scenes/`, `scripts/`, `assets/{sprites,tilesets,audio}`, `tilemaps/`, `.aether/`

Default scene files are dimension-specific minimal TOML.

### New Commands

- `aether world publish [--major|--minor|--patch] [--changelog "msg"]` - Bump version, append to `.aether/versions.toml`
- `aether world versions [path]` - Show version history in reverse chronological order

### CLI Structure

```
aether new <name> [--2d|--3d]          # alias for world new
aether check [path]                     # alias for world check
aether serve [path] [--port]            # unchanged
aether run [--list] [name]              # unchanged
aether version                          # unchanged
aether world new <name> [--2d|--3d]
aether world check [path]
aether world publish [flags]
aether world versions [path]
```

### Test Design

All tests written before implementation using `tempfile` for filesystem operations:

- **manifest.rs**: Default generation for 2D/3D, validation (valid/invalid dimension, empty name), serialization round-trip
- **commands/new.rs**: Directory structure verification for 2D/3D, world.toml content, scene files, versions.toml, duplicate directory error
- **commands/check.rs**: Valid 2D/3D projects pass, missing directories/scenes fail, invalid dimension fails, missing .aether warns
- **commands/world.rs**: Publish bumps versions correctly (major/minor/patch), updates world.toml, appends to versions.toml, versions lists in reverse order
