---
id: task-018
title: Content Moderation Pipeline
status: In Progress
assignee: []
created_date: '2026-03-07 13:19'
updated_date: '2026-03-07 15:04'
labels: []
dependencies:
  - task-017
  - task-010
  - task-014
priority: medium
ordinal: 17000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement automated + human content moderation: ML-based scanning for meshes/textures/scripts, WASM static analysis, human review queue, and reporting system.

Ref: docs/design/DESIGN.md Section 5.3
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Automated ML scanning for textures (NSFW, violence, hate symbols)
- [ ] #2 WASM static analysis for banned API patterns
- [ ] #3 Mesh analysis for prohibited geometry
- [ ] #4 Human review queue for flagged content
- [ ] #5 Player reporting system with priority escalation
- [ ] #6 Content rating assignment per design doc categories
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `aether-content-moderation` crate covering ML scan, wasm static analysis, mesh heuristics, and human review queue state.
2. Add report/escalation model with priorities and category mappings.
3. Add content rating artifacts for moderation decisions.
4. Add design notes for moderation service integration.
<!-- SECTION:PLAN:END -->
