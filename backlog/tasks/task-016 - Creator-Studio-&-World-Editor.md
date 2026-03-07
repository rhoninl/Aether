---
id: task-016
title: Creator Studio & World Editor
status: In Progress
assignee: []
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 15:04'
labels: []
dependencies:
  - task-002
  - task-017
priority: medium
ordinal: 15000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Build the Creator Studio: standalone app + in-VR mode for world building, terrain editing, prop placement, script authoring, and asset import pipeline.

Ref: docs/design/DESIGN.md Section 5.1
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Standalone desktop application for world editing
- [ ] #2 In-VR world editing mode
- [ ] #3 Terrain editor with sculpting and painting tools
- [ ] #4 Prop placement with transform gizmos and snapping
- [ ] #5 Script editor with visual scripting + code editor
- [ ] #6 Live preview / hot reload of changes
- [ ] #7 World manifest editor (settings, spawn points, physics)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `aether-creator-studio` crate with editor surface/event models for desktop and in-VR workflows.
2. Add terrain, prop, and script-editing operation records plus manifest editing state.
3. Add live preview/hot-reload contract with snapshot/rollback descriptors.
4. Add design notes for cross-platform editor toolchain integration.
<!-- SECTION:PLAN:END -->
