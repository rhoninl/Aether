---
id: task-027
title: API Gateway & Voice Relay
status: In Progress
assignee: []
created_date: '2026-03-07 13:43'
updated_date: '2026-03-07 15:04'
labels: []
dependencies:
  - task-011
  - task-005
priority: medium
ordinal: 26000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement edge API gateway for client-facing requests and STUN/TURN relay infrastructure for voice NAT traversal.

Ref: docs/design/DESIGN.md Section 2, 4.1
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 API gateway: rate limiting, auth token validation, request routing
- [ ] #2 STUN server for NAT traversal discovery
- [ ] #3 TURN relay for clients behind symmetric NAT
- [ ] #4 TLS termination at edge
- [ ] #5 Geographic routing to nearest region
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `aether-gateway` crate for edge routing policy, auth/rate controls, and geographic dispatch.
2. Add transport relay descriptors for STUN/TURN and TLS edge profiles.
3. Add voice relay service metadata and route selection helpers.
4. Add design note for gateway observability and failover policy.
<!-- SECTION:PLAN:END -->
