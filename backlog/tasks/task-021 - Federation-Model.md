---
id: task-021
title: Federation Model
status: To Do
assignee: []
created_date: '2026-03-07 13:19'
updated_date: '2026-03-07 14:13'
labels: []
dependencies:
  - task-014
  - task-017
  - task-019
  - task-011
  - task-012
  - task-018
priority: medium
ordinal: 20000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement federation: self-hosted world servers, world registry integration, content-addressed asset integrity, and portal interoperability (aether:// protocol).

Ref: docs/design/DESIGN.md Section 6.4
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Open-source world server binary for self-hosting
- [ ] #2 Self-hosted world registration via World Registry API
- [ ] #3 Portal interop: aether:// links work across platform and self-hosted
- [ ] #4 Content-addressed asset references with SHA-256 in manifest
- [ ] #5 Hash verification on client download; mismatch → reject + report
- [ ] #6 Modified-since-approval flagging for updated self-hosted assets
- [ ] #7 Platform AOT-compiles WASM scripts at submission; self-hosted serves platform artifacts
- [ ] #8 v1 centralization gates: self-hosted worlds must validate player tokens via central auth service
- [ ] #9 v1 centralization gates: all AEC transactions route through central Economy Service
- [ ] #10 v1 centralization gates: discoverable worlds must pass platform moderation to appear in registry
<!-- AC:END -->
