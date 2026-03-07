---
id: task-025
title: 'Backend Services: UGC Upload & Processing'
status: Done
assignee:
  - '@codex-001'
created_date: '2026-03-07 13:38'
updated_date: '2026-03-07 15:11'
labels: []
dependencies:
  - task-011
  - task-017
  - task-018
  - task-014
priority: high
ordinal: 24000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement UGC backend service: asset upload orchestration, moderation scan triggering, AOT WASM compilation, content-addressed storage, artifact lifecycle management.

Ref: docs/design/DESIGN.md Section 4.1, 5.2, 5.3, 8.3.1
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Asset upload API with chunked upload for large files
- [x] #2 Upload validation: file type, size limits, format verification
- [x] #3 Trigger moderation scan pipeline on upload
- [x] #4 AOT WASM compilation for all server targets at upload time
- [x] #5 Content-addressed storage: SHA-256 hash as artifact key
- [x] #6 Approved manifest generation for World Registry
- [x] #7 Artifact lifecycle: upload → scan → approve/reject → publish → archive
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `aether-ugc` crate with upload session, chunk upload, file validation, and artifact state machines.
2. Add moderation-trigger and scan-result contracts and AOT build request abstractions.
3. Add content-addressed storage model keyed by SHA-256.
4. Add world-registry approval manifest pathway and lifecycle transitions.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented UGC service contracts in `aether-ugc` for chunked upload sessions, file validation, moderation trigger/transition states, AOT profile descriptors, hash-addressable artifact model, and publish lifecycle states.
<!-- SECTION:NOTES:END -->
