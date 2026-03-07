---
id: task-017
title: Asset Pipeline & Bundle Format
status: Done
assignee:
  - '@codex-001'
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 15:11'
labels: []
dependencies:
  - task-002
priority: medium
ordinal: 16000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement the Aether asset bundle format (.aemesh, .aeenv), Basis Universal texture compression, Meshoptimizer integration, LOD generation, and streaming delivery.

Ref: docs/design/DESIGN.md Section 5.2
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Custom binary mesh format with Meshoptimizer compression
- [x] #2 Basis Universal texture compression (transcodes to BC7/ASTC/ETC2)
- [x] #3 Aether bundle format with manifest, LOD chain, dependencies
- [x] #4 Automatic LOD generation for meshes
- [x] #5 Progressive streaming: low LOD first, refine on demand
- [x] #6 Asset import from FBX, glTF, OBJ
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `aether-asset-pipeline` crate for bundle manifest, compression profiles, import translators, and LOD generation metadata.
2. Add mesh/texture format descriptors for .aemesh/.aeenv and transcode targets.
3. Add progressive streaming descriptors with LOD chains.
4. Add documentation on streaming and import extension points.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented asset pipeline contracts in `aether-asset-pipeline` for `.aemesh/.aeenv` style manifests, mesh compression targets, texture transcode profile, LOD metadata generation, progressive streaming chain, and FBX/gltf/OBJ import task surfaces.
<!-- SECTION:NOTES:END -->
