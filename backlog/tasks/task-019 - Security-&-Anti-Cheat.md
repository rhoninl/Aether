---
id: task-019
title: Security & Anti-Cheat
status: To Do
assignee: []
created_date: '2026-03-07 13:19'
updated_date: '2026-03-07 14:13'
labels: []
dependencies:
  - task-005
  - task-010
priority: high
ordinal: 18000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement security architecture: WASM sandboxing, server-authoritative validation, rate limiting, DDoS protection, network encryption, and input validation.

Ref: docs/design/DESIGN.md Section 6.1, 6.2
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Server-authoritative game state — clients are untrusted
- [ ] #2 WASM sandbox prevents access outside defined API surface
- [ ] #3 Rate limiting per-player per-action at server
- [ ] #4 Input validation and plausibility checking
- [ ] #5 QUIC with TLS 1.3 for all client-server traffic
- [ ] #6 DDoS protection at network edge
- [ ] #7 Content-addressed asset integrity (SHA-256 hash verification)
<!-- AC:END -->
