# Security & Anti-Cheat (task-019)

Added security primitives for server authoritative checks, sandbox policy, rate limiting, and transport hardening.

## Implemented API surface

- Added crate `aether-security` with modules:
  - `anti_cheat`: input plausibility and cheat signals.
  - `ratelimit`: per-action rate limit descriptors.
  - `transport`: DDoS signal enums and defense states.
  - `encryption`: QUIC/TLS transport policy model.
  - `wasm`: sandbox capability caps and wasm violation errors.
- Updated workspace membership to include `aether-security`.

## Mapping to acceptance criteria

- `#1` Server authority represented by explicit server-side validation signals and verdicts.
- `#2` WASM boundaries represented by `WasmSandboxCapability` and `WasmSurfaceError`.
- `#3` Player action rate limiting per-action via `RateLimit` and `RateLimitBucket`.
- `#4` Input plausibility checks represented by `InputPlausibility` and cheat signals.
- `#5` Transport policy includes TLS mode and transport fields.
- `#6` DDoS-related states/signals represented via `AttackSignal`/`DdosDefenseState`.
- `#7` Asset integrity error surface represented by `WasmSurfaceError::ModuleTampered` (hash/hardening hook).

## Remaining implementation work

- Bind these policies to the networking and scripting runtimes.
- Implement enforcement and telemetry feedback loops for mitigations.
