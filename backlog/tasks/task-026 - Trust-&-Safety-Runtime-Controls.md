---
id: task-026
title: Trust & Safety Runtime Controls
status: Done
assignee:
  - '@codex-001'
created_date: '2026-03-07 13:41'
updated_date: '2026-03-07 15:11'
labels: []
dependencies:
  - task-005
  - task-013
priority: medium
ordinal: 25000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement player safety features: personal space bubble, visibility modes (invisible, friends-only), anonymous mode, parental controls, and in-world moderation tools.

Ref: docs/design/DESIGN.md Section 6.1
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Personal space bubble with configurable radius
- [x] #2 Visibility modes: visible, friends-only, invisible
- [x] #3 Anonymous/temporary avatar mode (no persistent identity)
- [x] #4 Parental controls: content filtering, time limits, social restrictions
- [x] #5 In-world moderation tools: mute, kick, ban for world owners
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `aether-trust-safety` crate with player safety envelope, social visibility, anonymous mode, parental controls, and world moderation actions.
2. Add personal-space policies and moderation controls with time-bound actions.
3. Add settings and enforcement profile contracts.
4. Add documentation for in-world controls and escalation behavior.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented trust-and-safety policy in `aether-trust-safety` with personal-space controls, visibility modes, anonymous session mode, parental limits, and in-world moderation tool actions.
<!-- SECTION:NOTES:END -->
