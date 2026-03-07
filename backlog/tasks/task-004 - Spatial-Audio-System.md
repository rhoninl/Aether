---
id: task-004
title: Spatial Audio System
status: In Progress
assignee:
  - '@claude-001'
created_date: '2026-03-07 13:17'
updated_date: '2026-03-07 14:56'
labels: []
dependencies:
  - task-001
  - task-005
priority: high
ordinal: 3000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement 3D spatial audio pipeline: HRTF-based spatialization, distance attenuation, room acoustics, voice chat with Opus codec.

Ref: docs/design/DESIGN.md Section 3.4
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 HRTF-based binaural spatialization for all audio sources
- [ ] #2 Distance attenuation with configurable falloff curves
- [ ] #3 Room acoustics: reverb, occlusion, early reflections
- [ ] #4 Voice chat zones: spatial proximity, private channels, world broadcast
- [ ] #5 Opus codec for voice with in-band FEC
- [ ] #6 Audio LOD: fewer processing stages for distant sources
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Create `crates/aether-audio` as an execution-agnostic audio domain layer for HRTF parameters, attenuation models, and voice/chat routing policies.
1) Define audio source/listener data structures and HRTF profile traits with distance-attenuation and spatial blend helpers.
2) Add room-acoustics model (occlusion/reverb/reflections multipliers) and LOD falloff policy by source distance.
3) Model voice communication zones and channel permissions (proximity/private/world broadcast), plus per-zone routing policy.
4) Add Opus envelope metadata representation (frame/sample-rate/bitrate/FEC flags) and stream scheduler constraints.
5) Provide deterministic unit tests for zone selection, attenuation continuity, and policy-based LOD level changes.
6) Add a design note mapping acceptance criteria to implemented primitives, keeping codec/runtime integration as follow-up work.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added `crates/aether-audio` with policy/data model for spatial audio: attenuation profiles, acoustics/room settings, HRTF profile/sample stubs, Opus config/packet metadata, and a voice channel/routing manager with channel kinds (proximity/private/world). This establishes foundations for all six acceptance criteria as policy-level implementations.
<!-- SECTION:NOTES:END -->
