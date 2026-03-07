---
id: task-010
title: WASM Scripting Runtime
status: To Do
assignee: []
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 14:13'
labels: []
dependencies:
  - task-001
priority: high
ordinal: 9000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement Wasmtime-based WASM sandbox for world scripting: API bindings, resource limits, script scheduler, visual scripting compiler, and multi-platform AOT/JIT strategy.

Ref: docs/design/DESIGN.md Section 3.8
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Wasmtime sandbox with per-script resource limits (CPU 5ms/tick, 64MB mem)
- [ ] #2 Engine API bindings: Entity, Physics, UI, Audio, Storage, Network
- [ ] #3 World-level script scheduler with priority, aging, and 8ms budget per tick
- [ ] #4 Support Rust, AssemblyScript, C/C++ as WASM target languages
- [ ] #5 Visual scripting that compiles to WASM
- [ ] #6 Server always AOT; PC client JIT; constrained clients server-side only
- [ ] #7 Overload detection: force-suspend lowest-priority scripts after 10s
- [ ] #8 Per-script rate limits: entity spawns (100/s), network RPCs (50/s), storage writes (10/s)
- [ ] #9 World-level hard caps: 512MB total script memory, 10000 scripted entities
<!-- AC:END -->
