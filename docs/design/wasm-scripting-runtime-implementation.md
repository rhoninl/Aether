# WASM Runtime Implementation Notes (task-010)

This document tracks the current implementation checkpoint for the scripting backlog.

## Implemented in code

- Added new workspace crate `crates/aether-scripting`.
- Implemented task-010 policy primitives:
  - per-script runtime caps (CPU, memory, action rate caps)
  - world-level script resource caps (CPU budget per tick, total memory, scripted entity cap)
  - scheduler with priority ordering and deferred-script aging
  - overload tracking across a 10s window and forced suspension of lowest-priority scripts
  - rate-limited action helpers for entity spawns, network RPCs, and storage writes
- Added API trait surface for engine integration:
  - `EntityApi`
  - `PhysicsApi`
  - `UIApi`
  - `AudioApi`
  - `NetworkApi`
  - `StorageApi`

## Remaining work before full acceptance

- Swap scheduler execution hook with Wasmtime instance execution integration.
- Add concrete engine-script/UGC-script artifact pipeline and manifest handling.
- Add AOT/JIT selection strategy and platform mapping.
- Add Visual Scripting compiler flow into `.wasm` artifacts.
