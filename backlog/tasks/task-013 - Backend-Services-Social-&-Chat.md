---
id: task-013
title: 'Backend Services: Social & Chat'
status: Done
assignee:
  - '@codex-001'
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 15:11'
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
- [x] #1 Friend requests, accept/decline/block
- [x] #2 Group/party creation and management
- [x] #3 Presence system (online, offline, in-world with location)
- [x] #4 Real-time text chat (DMs, group, world)
- [x] #5 Citus-sharded by user_id with eventual consistency
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add social crate (`aether-social`) with friend/group/presence/chat event primitives.
2. Model DMs/groups/world channels and user visibility transitions.
3. Add sharding metadata contracts for user_id partitioning.
4. Document consistency and moderation interfaces.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added social contracts in `aether-social` for friend lifecycle, group/party management, presence state, channelized text chat, and user shard mapping by user_id partition policy.
<!-- SECTION:NOTES:END -->
