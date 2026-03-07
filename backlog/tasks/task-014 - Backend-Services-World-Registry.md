---
id: task-014
title: 'Backend Services: World Registry'
status: To Do
assignee: []
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 14:13'
labels: []
dependencies:
  - task-011
priority: high
ordinal: 13000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement world registry service: world discovery, session management, world manifest storage, and matchmaking.

Scope boundary: This task covers the core registry service for platform-hosted worlds. Self-hosted world registration and portal interop across federated worlds are covered in task-021 (Federation Model).

Ref: docs/design/DESIGN.md Section 4.1, 4.2
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 World manifest storage and validation
- [ ] #2 World discovery API with search, categories, featured
- [ ] #3 Portal routing (aether:// protocol resolution)
- [ ] #4 Session manager: spawn/despawn world server instances
- [ ] #5 Matchmaking: route players to nearest region, balance load
<!-- AC:END -->
