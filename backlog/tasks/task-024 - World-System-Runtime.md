---
id: task-024
title: World System Runtime
status: Done
assignee:
  - '@codex-001'
created_date: '2026-03-07 13:36'
updated_date: '2026-03-07 15:11'
labels: []
dependencies:
  - task-001
  - task-002
  - task-003
priority: high
ordinal: 23000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement world loading and runtime: chunk streaming, terrain/props/lighting loading from world manifest, spawn point management, world lifecycle (boot, run, shutdown).

Ref: docs/design/DESIGN.md Section 3.9, 5.2
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 World manifest parsing and validation
- [x] #2 Chunk-based terrain streaming with LOD
- [x] #3 Prop loading and placement from manifest
- [x] #4 Lighting and skybox setup from manifest (.aeenv)
- [x] #5 Spawn point management for player entry
- [x] #6 World lifecycle: boot → run → shutdown with graceful cleanup
- [x] #7 World settings runtime enforcement (gravity, tick rate, max players)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `aether-world-runtime` crate for manifest parsing, chunk streaming, prop/lighting setup, spawn management, and lifecycle state machine.
2. Add runtime setting validation and hot-enforcement policies.
3. Add world lifecycle event/state transitions and cleanup contracts.
4. Document implementation checkpoints against all acceptance criteria.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented world runtime policy in `aether-world-runtime` for manifest validation/loader, chunk and prop/lighting loading, spawn point governance, lifecycle finite-state transitions, and setting enforcement hooks.
<!-- SECTION:NOTES:END -->
