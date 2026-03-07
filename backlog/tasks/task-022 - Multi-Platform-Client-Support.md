---
id: task-022
title: Multi-Platform Client Support
status: In Progress
assignee: []
created_date: '2026-03-07 13:19'
updated_date: '2026-03-07 15:04'
labels: []
dependencies:
  - task-010
  - task-002
  - task-009
priority: medium
ordinal: 21000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement client builds for all target platforms with platform-specific WASM strategy, progressive fidelity, and platform store compliance.

Ref: docs/design/DESIGN.md Section 8.3
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 PC VR (SteamVR/Oculus): full quality, client JIT + server AOT
- [ ] #2 Desktop flat-screen: mouse/keyboard, spectator mode
- [ ] #3 Meta Quest standalone: server-side user scripts, bundled engine AOT
- [ ] #4 Apple Vision Pro (planned): visionOS compliance, server-side user scripts
- [ ] #5 PlayStation VR2 (planned): console certification, server-authoritative scripts
- [ ] #6 Progressive fidelity: graceful degradation across platforms
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `aether-platform` crate for target capabilities, input fidelity, build modes, and runtime constraints across PC VR, desktop, mobile/standalone, and consoles.
2. Add adaptive quality profiles and script execution mode mappings.
3. Add compliance descriptors for cert/approval target metadata.
4. Add runtime feature toggles for progressive fidelity and compatibility profiles.
<!-- SECTION:PLAN:END -->
