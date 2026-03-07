---
id: task-029.07
title: World Runtime + Zoning + Performance Completion
status: To Do
assignee:
  - '@codex-001'
created_date: '2026-03-07 15:12'
labels: []
dependencies:
  - task-007
  - task-024
  - task-023
  - task-022
parent_task_id: task-029
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Finish runtime-side world execution: chunk/prop/lighting streaming, zone orchestration, lifecycle controls, and cross-domain performance hardening.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Load and validate world manifests in running world server lifecycle
- [ ] #2 Implement chunk streaming and LOD terrain/prop loader with resource budgets
- [ ] #3 Execute cross-zone split/merge and ghost entity handoff in runtime path
- [ ] #4 Integrate world settings enforcement and lifecycle transitions with authoritative cleanup
- [ ] #5 Complete platform-wide profiling hooks beyond ECS diagnostics for core render/physics/network/scripting targets
<!-- AC:END -->
