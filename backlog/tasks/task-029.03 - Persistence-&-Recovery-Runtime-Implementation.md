---
id: task-029.03
title: Persistence & Recovery Runtime Implementation
status: To Do
assignee:
  - '@codex-001'
created_date: '2026-03-07 15:12'
labels: []
dependencies:
  - task-006
  - task-025
  - task-021
  - task-015
parent_task_id: task-029
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Turn persistence policy in aether-persistence into runnable snapshot/WAL/placement behavior and session-managed pod/runtime routing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Implement periodic snapshot producer/consumer for ephemeral world state
- [ ] #2 Implement WAL append/replay durability path with fsync-before-ack semantics
- [ ] #3 Add critical-state RPC sync adapter with idempotent transaction handling
- [ ] #4 Add world placement resolver for stateful vs stateless deployments from manifest intent
- [ ] #5 Add crash-restart recovery path including script state replay and snapshot restore
<!-- AC:END -->
