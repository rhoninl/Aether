---
id: task-014
title: 'Backend Services: World Registry'
status: Done
assignee:
  - '@codex-001'
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 15:11'
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
- [x] #1 World manifest storage and validation
- [x] #2 World discovery API with search, categories, featured
- [x] #3 Portal routing (aether:// protocol resolution)
- [x] #4 Session manager: spawn/despawn world server instances
- [x] #5 Matchmaking: route players to nearest region, balance load
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add backend registry crate (`aether-registry`) with manifest schema, discovery filters, and portal URI resolver.
2. Add session manager models for world instance lifecycle and region-aware routing.
3. Add matchmaking policies (nearest region + load balancing) as policy objects.
4. Add implementation notes and state transition markers.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added registry policy in `aether-registry` covering manifest schema validation/storage contracts, discovery APIs, `aether://` portal routing contracts, session manager lifecycle, and load/region-aware matchmaking policy objects.
<!-- SECTION:NOTES:END -->
