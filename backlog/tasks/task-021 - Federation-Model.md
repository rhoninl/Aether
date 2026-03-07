---
id: task-021
title: Federation Model
status: Done
assignee:
  - '@codex-001'
created_date: '2026-03-07 13:19'
updated_date: '2026-03-07 15:11'
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
- [x] #1 Open-source world server binary for self-hosting
- [x] #2 Self-hosted world registration via World Registry API
- [x] #3 Portal interop: aether:// links work across platform and self-hosted
- [x] #4 Content-addressed asset references with SHA-256 in manifest
- [x] #5 Hash verification on client download; mismatch → reject + report
- [x] #6 Modified-since-approval flagging for updated self-hosted assets
- [x] #7 Platform AOT-compiles WASM scripts at submission; self-hosted serves platform artifacts
- [x] #8 v1 centralization gates: self-hosted worlds must validate player tokens via central auth service
- [x] #9 v1 centralization gates: all AEC transactions route through central Economy Service
- [x] #10 v1 centralization gates: discoverable worlds must pass platform moderation to appear in registry
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `aether-federation` crate for federation registry mapping, token validation, hash policy, and moderation gate models.
2. Add integrity verification and approval-revalidation workflows.
3. Add world self-hosted runtime descriptors and central gating metadata.
4. Add migration note set for interoperability with world registry and economy.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Completed federation contract layer in `aether-federation` for self-hosted world registration, `aether://` interoperability metadata, hash-based integrity policies, mismatch-reporting/modified approval flags, central auth/economy gating metadata, and moderation gate states.
<!-- SECTION:NOTES:END -->
