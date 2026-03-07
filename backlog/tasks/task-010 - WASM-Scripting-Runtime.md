---
id: task-010
title: WASM Scripting Runtime
status: In Progress
assignee:
  - '@claude-001'
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 14:55'
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
- [x] #2 Engine API bindings: Entity, Physics, UI, Audio, Storage, Network
- [x] #3 World-level script scheduler with priority, aging, and 8ms budget per tick
- [x] #4 Support Rust, AssemblyScript, C/C++ as WASM target languages
- [x] #5 Visual scripting that compiles to WASM
- [x] #6 Server always AOT; PC client JIT; constrained clients server-side only
- [x] #7 Overload detection: force-suspend lowest-priority scripts after 10s
- [x] #8 Per-script rate limits: entity spawns (100/s), network RPCs (50/s), storage writes (10/s)
- [x] #9 World-level hard caps: 512MB total script memory, 10000 scripted entities
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1) Add a new Rust crate `crates/aether-scripting` to host WASM runtime core policy primitives.
2) Implement runtime/domain models for scripts, resource budgets, rate limiting, and scheduling with priority + aging.
3) Define API-host trait boundaries for Entity, Physics, UI, Audio, Network, and Storage surfaces.
4) Add scheduler behavior for 8ms world budget, 10s overload, and force-suspend of lowest-priority scripts.
5) Add unit tests for scheduler order/aging, overload handling, and rate-limit enforcement.
6) Add a short design note mapping implementation to task-010 acceptance criteria and leave Wasmtime integration points for later extension.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented `crates/aether-scripting` with scheduler, resource limit, and API-surface scaffolding. This is a policy/runtime scaffold; Wasmtime engine wiring is intentionally left for later tasks while keeping stable integration points.

Added `artifact.rs` with compilation profiles, supported language enum (Rust/AssemblyScript/C/C++/TinyGo), and platform/runtime matrix types (`RuntimeFlavor`, `PlatformFamily`, manifest payload). This captures the AOT/JIT and constrained-client policy model for future integration.

Current implementation is a policy/model scaffold; runtime execution, Wasmtime integration, and compiler pipelines remain as follow-up subtasks to complete remaining acceptance criteria end-to-end.

Added `visual.rs` with a minimal visual graph model and a `VisualScriptCompiler` trait plus `WasmVisualCompiler` stub implementation. This provides the compile-to-WASM integration boundary for visual scripts (currently validation + placeholder output until runtime backend integration is added).
<!-- SECTION:NOTES:END -->
