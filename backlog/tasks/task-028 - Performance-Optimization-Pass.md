---
id: task-028
title: Performance Optimization Pass
status: In Progress
assignee:
  - codex-001
created_date: '2026-03-07 13:45'
updated_date: '2026-03-07 14:41'
labels: []
dependencies:
  - task-002
  - task-005
  - task-010
priority: low
ordinal: 27000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 4 performance optimization: profiling, bottleneck identification, renderer optimization, network bandwidth reduction, memory optimization across all subsystems.

Ref: docs/design/DESIGN.md Section 7, 9
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Full profiling pass across render, physics, networking, scripting
- [ ] #2 Meet performance targets: 90fps VR, <20ms motion-to-photon
- [ ] #3 Network bandwidth optimization for 200-player worlds
- [ ] #4 Memory usage optimization for Quest standalone target
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Performance slice for this repository will be limited to core ECS runtime throughput; full cross-subsystem profiling remains out-of-scope here.
1) Optimize schedule batching algorithm to reduce scheduler rebuild overhead in high system counts: replace O(n^2) local-index lookups with O(1) mapping in `build_batches`.
2) Add lightweight profiling helpers for runtime scheduling diagnostics: expose schedule-level diagnostic snapshot (run count, avg total time, stage batch count, last stage time trend), keeping APIs additive.
3) Add unit tests for batching determinism/performance-related behavior and diagnostics.
4) Run formatter/tests and update notes and completion state.
<!-- SECTION:PLAN:END -->
