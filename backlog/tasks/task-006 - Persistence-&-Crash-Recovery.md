---
id: task-006
title: Persistence & Crash Recovery
status: To Do
assignee: []
created_date: '2026-03-07 13:17'
updated_date: '2026-03-07 14:13'
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
