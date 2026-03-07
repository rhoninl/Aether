---
id: task-007
title: Spatial Load Balancing & Zone Splitting
status: Done
assignee:
  - '@codex-001'
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 15:11'
labels: []
dependencies:
  - task-005
  - task-006
priority: medium
ordinal: 6000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement dynamic zone splitting/merging, cross-zone authority model, player handoff protocol, and ghost entities for seamless multi-server worlds.

Ref: docs/design/DESIGN.md Section 3.5.4
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 K-d tree zone partitioning along axis of greatest player spread
- [x] #2 Single-writer entity ownership (authority_zone in NetworkIdentity)
- [x] #3 Ghost entities for cross-boundary rendering and collision queries
- [x] #4 Player handoff protocol with sequence fence and fail-safe timeout
- [x] #5 Cross-zone physics arbitration (initiator server computes, target validates)
- [x] #6 Cross-zone combat: target server has final say on competitive interactions
- [x] #7 Zone merge when population drops below threshold
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Create spatial partition crate (`aether-zoning`) with zone topology, authority, and handoff models.
2. Add K-d tree splitter/merger primitives and ghost entity mirror primitives.
3. Add cross-zone session protocol records: handoff envelope, sequence fences, arbitration outcomes.
4. Add design note capturing merge/split and combat/physics resolution flow.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented zoning policy primitives in `aether-zoning` including K-d split policy, single-writer authority, ghost entity model, handoff/sequence fence protocol, and split/merge thresholds for cross-zone orchestration; runtime protocol orchestrator remains follow-up.
<!-- SECTION:NOTES:END -->
