---
id: task-002
title: Rendering Pipeline
status: In Progress
assignee:
  - '@claude-001'
created_date: '2026-03-07 13:17'
updated_date: '2026-03-07 14:56'
labels: []
dependencies:
  - task-001
priority: high
ordinal: 1000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement the VR-optimized rendering pipeline using wgpu (Vulkan/Metal/DX12).

Ref: docs/design/DESIGN.md Section 3.2
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Stereo rendering via VK_KHR_multiview (single draw call for both eyes)
- [ ] #2 Foveated rendering with VRS Tier 2 + eye tracking (40-60% pixel reduction)
- [ ] #3 Clustered forward+ lighting (thousands of lights, bounded overhead)
- [ ] #4 Cascaded shadow maps with per-cascade resolution for VR distances
- [ ] #5 GPU-driven instanced rendering (draw calls batched per material)
- [ ] #6 Automatic LOD system (4 levels, hysteresis-based switching)
- [ ] #7 Progressive mesh streaming (low LOD first, refine as bandwidth allows)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Implement `crates/aether-renderer` as the pipeline policy and scheduling layer for rendering features, then keep it lightweight and backend-agnostic.
1) Add workspace crate `crates/aether-renderer` and public modules for profile/feature toggles: stereo config, foveation, lighting, shadows, LOD, and mesh streaming policy.
2) Implement deterministic policy primitives: LOD band thresholds with hysteresis, cluster-light binning knobs, shadow cascade selection, foveation rate-of-change smoothing, and draw-call batching hints.
3.1) Add `FramePolicy` selection and `FrameCost` estimation helpers to estimate workload and pick fidelity tiers.
4) Add tests for LOD hysteresis transitions, cascade budget clamp behavior, and batch key sorting.
5) Add design/implementation note mapping these primitives to task-002 acceptance criteria, explicitly marking execution/render backend as a follow-up integration.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added `crates/aether-renderer` and implemented policy primitives for all task-002 feature areas (multiview/foveation/lighting/shadows/LOD hysteresis/batching/streaming). Added deterministic workload and scheduler helpers to expose frame-mode selection signals for backend integration. No concrete `wgpu` backend implementation yet; this is a pre-execution foundation stage.
<!-- SECTION:NOTES:END -->
