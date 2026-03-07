# Spatial Load Balancing & Zone Splitting (task-007)

This milestone introduces a structural skeleton for zone topology management and cross-zone protocols.

## Implemented API surface

- Added crate `aether-zoning` with modules:
  - `partition`: K-d style zone split heuristics and split result descriptors.
  - `authority`: `NetworkIdentity` + single-writer ownership tracking with sequence metadata.
  - `ghost`: ghost-entity cache models for cross-boundary render/collision mirroring.
  - `protocol`: handoff envelope, sequence fences, timeout checks, and arbitration result enums.
- Added zone split policy/config types for merge/threshold and split-axis preference.

## Mapping to acceptance criteria

- `#1` axis selection routine available in `partition::KdTree::choose_axis` and split result includes chosen axis.
- `#2` `authority::NetworkIdentity::authority_zone` represents single-writer ownership.
- `#3` `ghost::GhostEntity` and `GhostCache` model ghost entities and lifecycle operations.
- `#4` `protocol::HandoffEnvelope` plus `HandoffResult` represent handoff requests and outcome with timeout/fence fields.
- `#5` `CrossZonePhysicsDecision` supports initiator/target arbitration flow.
- `#6` `CrossZoneCombatDecision` encodes target-server final authority.
- `#7` `MergeThreshold` and `SplitPolicy` types support merge policy configuration.

## Remaining implementation work

- Implement full tree mutability and recursive zone movement.
- Add persistence and transport serialization for handoff envelopes.
- Add deterministic arbitration and failover state transitions for disconnected players.
