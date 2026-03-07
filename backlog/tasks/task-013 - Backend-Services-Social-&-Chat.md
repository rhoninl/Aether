---
id: task-013
title: 'Backend Services: Social & Chat'
status: In Progress
assignee: []
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 15:00'
labels: []
dependencies:
  - task-011
priority: medium
ordinal: 12000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement social service: friend system, groups, blocking, presence (online/offline/in-world), and real-time text + spatial voice chat.

Ref: docs/design/DESIGN.md Section 4.1
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Friend requests, accept/decline/block
- [ ] #2 Group/party creation and management
- [ ] #3 Presence system (online, offline, in-world with location)
- [ ] #4 Real-time text chat (DMs, group, world)
- [ ] #5 Citus-sharded by user_id with eventual consistency
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add social crate (`aether-social`) with friend/group/presence/chat event primitives.
2. Model DMs/groups/world channels and user visibility transitions.
3. Add sharding metadata contracts for user_id partitioning.
4. Document consistency and moderation interfaces.
<!-- SECTION:PLAN:END -->
