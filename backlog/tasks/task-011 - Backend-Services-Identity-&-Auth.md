---
id: task-011
title: 'Backend Services: Identity & Auth'
status: In Progress
assignee:
  - '@claude-005'
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 14:32'
labels: []
dependencies: []
priority: high
ordinal: 10000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement identity service: JWT-based authentication, OAuth2 social login, session management, player profiles, and token validation for federated world servers.

Ref: docs/design/DESIGN.md Section 4.1
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 JWT-based auth with RSA/Ed25519 signing
- [ ] #2 OAuth2 social login (Google, Apple, Discord, Steam)
- [ ] #3 Session management with refresh tokens
- [ ] #4 Player profile CRUD (name, avatar, bio, settings)
- [ ] #5 Token validation endpoint for self-hosted world servers
- [ ] #6 Single-primary DB with async read replicas per region
- [ ] #7 WebAuthn/passkey support for passwordless login
- [ ] #8 RBAC/ABAC permission model with role hierarchy (player, creator, moderator, admin)
- [ ] #9 Audit log for auth events (login, token refresh, permission changes)
<!-- AC:END -->
