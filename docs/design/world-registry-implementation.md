# World Registry (task-014)

Added registry primitives for manifest validation, discovery filters, portal routing, and session orchestration.

## Implemented API surface

- Added crate `aether-registry` with modules:
  - `manifest`: world record schema, status/category types, and validation checks.
  - `discovery`: query/filter/sort models for world discovery API.
  - `portal`: portal scheme resolver for `aether://` and fallback routing.
  - `session`: instance lifecycle states and session assignment policy outputs.
- Updated workspace members for `aether-registry`.

## Mapping to acceptance criteria

- `#1` world manifest validation and model types are present in `manifest`.
- `#2` discovery API contract via `DiscoveryFilter` and `DiscoveryResult`.
- `#3` portal protocol resolution via `PortalResolver::parse` and `resolve`.
- `#4` world instance lifecycle with `SessionState` and `SessionManager`.
- `#5` region-aware policy and population-based routing via `RegionPolicy` and `route_player`.

## Remaining implementation work

- Persist manifests and sessions in backend store.
- Add scoring and nearest-region telemetry for matchmaking.
- Implement portal verification and federation interop in world registry service.
