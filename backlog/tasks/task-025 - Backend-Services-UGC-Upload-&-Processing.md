---
id: task-025
title: 'Backend Services: UGC Upload & Processing'
status: To Do
assignee: []
created_date: '2026-03-07 13:38'
updated_date: '2026-03-07 14:13'
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
- [ ] #1 Asset upload API with chunked upload for large files
- [ ] #2 Upload validation: file type, size limits, format verification
- [ ] #3 Trigger moderation scan pipeline on upload
- [ ] #4 AOT WASM compilation for all server targets at upload time
- [ ] #5 Content-addressed storage: SHA-256 hash as artifact key
- [ ] #6 Approved manifest generation for World Registry
- [ ] #7 Artifact lifecycle: upload → scan → approve/reject → publish → archive
<!-- AC:END -->
