---
id: task-029.04
title: 'Backend Services Runtime Integration: Identity, Economy, Social, Registry'
status: To Do
assignee:
  - '@codex-001'
created_date: '2026-03-07 15:12'
labels: []
dependencies:
  - task-011
  - task-012
  - task-013
  - task-014
  - task-021
parent_task_id: task-029
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement runtime service executors and data stores for identity/auth, economy, social/chat, and registry based on existing policy crates.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Complete auth token/session validation paths across login, refresh, and federation handoff
- [ ] #2 Materialize wallet/ledger/payout transaction flows and idempotency constraints in persistence layer
- [ ] #3 Implement friend/presence/group/chat service paths with user shard routing
- [ ] #4 Implement world manifest persistence and matchmaking logic using session manager policies
- [ ] #5 Provide service-level integration tests for cross-service contracts
<!-- AC:END -->
