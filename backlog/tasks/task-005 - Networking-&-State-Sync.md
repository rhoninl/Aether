---
id: task-005
title: Networking & State Sync
status: To Do
assignee: []
created_date: '2026-03-07 13:17'
updated_date: '2026-03-07 14:13'
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
