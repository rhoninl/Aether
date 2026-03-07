---
id: task-029.06
title: 'Platform, Federation, UGC, and Gateway Runtime Wiring'
status: To Do
assignee:
  - '@codex-001'
created_date: '2026-03-07 15:12'
labels: []
dependencies:
  - task-021
  - task-022
  - task-025
  - task-027
  - task-015
  - task-016
parent_task_id: task-029
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement cross-platform, federation, gateway, and deployment-facing runtime flows not yet executed as real services.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Add platform-aware profile switching for client runtimes and script mode enforcement
- [ ] #2 Wire gateway edge auth/rate limit/relay routing to live region and auth backends
- [ ] #3 Complete asset moderation scan and approval flow integration in UGC upload lifecycle
- [ ] #4 Implement federation token validation and transaction routing against central services
- [ ] #5 Add registry/manifest validation and discovery API compatibility for self-hosted and platform worlds
<!-- AC:END -->
