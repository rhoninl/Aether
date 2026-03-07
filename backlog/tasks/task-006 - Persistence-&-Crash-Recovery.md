---
id: task-006
title: Persistence & Crash Recovery
status: In Progress
assignee: []
created_date: '2026-03-07 13:17'
updated_date: '2026-03-07 14:58'
labels: []
dependencies:
  - task-005
  - task-011
  - task-012
  - task-014
priority: high
ordinal: 5000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement state persistence model: periodic snapshots for ephemeral state, WAL for durable script state, transactional persistence for critical state via service RPCs.

Ref: docs/design/DESIGN.md Section 3.5.3.1
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Critical state (economy, inventory, identity) persisted via synchronous service RPCs
- [ ] #2 Ephemeral state snapshots every 5s (positions, rotations, props)
- [ ] #3 WAL for durable script state keys: fsync before ack, replay on crash
- [ ] #4 PVC-backed StatefulSet for worlds with durable/economy features
- [ ] #5 Stateless Deployment pods for simple worlds (no PVC)
- [ ] #6 Session Manager routes worlds to correct pod type based on manifest
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Introduce persistence crate (`aether-persistence`) with snapshot, WAL, transaction, and deployment selectors.
2. Add persistence policy/state models for critical state writes, durable script WAL lifecycle, and pod placement manifest mapping.
3. Add design document capturing recovery and routing decisions, including session manager placement hints.
4. Mark acceptance criteria as implemented after API scaffolding is in place.
<!-- SECTION:PLAN:END -->
