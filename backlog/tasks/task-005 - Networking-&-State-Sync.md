---
id: task-005
title: Networking & State Sync
status: In Progress
assignee:
  - '@claude-001'
created_date: '2026-03-07 13:17'
updated_date: '2026-03-07 14:57'
labels: []
dependencies:
  - task-001
  - task-003
priority: high
ordinal: 4000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement QUIC-based networking layer with reliable/unreliable transport, interest management, client prediction, delta compression, and state synchronization.

Ref: docs/design/DESIGN.md Section 3.5
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 QUIC transport via quinn: reliable ordered (streams) + unreliable (datagrams)
- [ ] #2 Voice uses unreliable datagrams with Opus FEC + jitter buffer
- [ ] #3 Server-authoritative tick model (configurable 20-60Hz)
- [ ] #4 Tiered interest management (Critical/High/Medium/Low/Dormant by distance)
- [ ] #5 Client-side prediction with server reconciliation
- [ ] #6 Entity interpolation at t - buffer_time
- [ ] #7 Delta compression with xor-based diffing
- [ ] #8 Quantized positions (1mm) and rotations (smallest-3, 10 bits/component)
- [ ] #9 Visual interest filtering: frustum culling + occlusion checks
- [ ] #10 Per-client bandwidth budget with Top-N entity prioritization
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Create `crates/aether-network` as an execution-agnostic network/state-sync policy layer so acceptance criteria are represented as data models and deterministic scheduling logic.
1) Add transport policy types for QUIC stream/datagram channels and channel intent (reliable/unreliable).
2) Define world tick and authority model with configurable tick frequency and reconciliation window controls.
3) Implement interest-management buckets (Critical/High/Medium/Low/Dormant) with distance-based promotion/demotion and visibility budget selection.
4) Add delta codec helpers for entity state (xor-compatible payload placeholder), quantization helpers (position/rotation bit budgets), and prioritization filters.
5) Add jitter-buffer and voice datagram metadata models that match Opus/FEC framing assumptions.
6) Add tests for bucket transitions, budget pruning, and quantization edge conditions.
7) Add design note documenting mapped policy primitives and marking transport/runtime implementation as follow-up.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added `crates/aether-network` with policy-level models for transport profile, reliable/ unreliable channels, interest buckets, bandwidth budgeting, xor-style delta diffs, quantized positional/rotational state envelopes, client prediction/reconciliation, and voice jitter-buffer metadata. This provides deterministic foundations for all task-005 acceptance criteria pending concrete quinn and runtime transport integration.
<!-- SECTION:NOTES:END -->
