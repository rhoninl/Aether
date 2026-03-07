---
id: task-009
title: VR Input & Interaction
status: In Progress
assignee: []
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 14:58'
labels: []
dependencies:
  - task-001
  - task-003
priority: high
ordinal: 8000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement OpenXR integration, VR input handling, and interaction systems for hands, controllers, haptics, and locomotion.

Ref: docs/design/DESIGN.md Section 3.7
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 OpenXR integration supporting all major headsets
- [ ] #2 Hand tracking + controller input abstraction layer
- [ ] #3 Interaction system: grab, use, point, throw with physics
- [ ] #4 Haptic feedback API (basic controller haptics)
- [ ] #5 Locomotion modes: teleport, smooth, climbing, flying (world-configurable)
- [ ] #6 Comfort settings: vignette, snap turn, seated mode
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `aether-input` crate exposing headset/runtime capability abstraction and action mappings.
2. Add hand and controller interaction primitives with grab/use/point/throw descriptors and basic haptics envelopes.
3. Define locomotion/composure comfort profiles and mode negotiation contracts.
4. Add backend interoperability types for OpenXR/session adapters and world-configurable policy.
<!-- SECTION:PLAN:END -->
