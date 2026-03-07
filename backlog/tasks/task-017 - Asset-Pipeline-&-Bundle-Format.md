---
id: task-017
title: Asset Pipeline & Bundle Format
status: To Do
assignee: []
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 14:13'
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
- [ ] #1 Custom binary mesh format with Meshoptimizer compression
- [ ] #2 Basis Universal texture compression (transcodes to BC7/ASTC/ETC2)
- [ ] #3 Aether bundle format with manifest, LOD chain, dependencies
- [ ] #4 Automatic LOD generation for meshes
- [ ] #5 Progressive streaming: low LOD first, refine on demand
- [ ] #6 Asset import from FBX, glTF, OBJ
<!-- AC:END -->
