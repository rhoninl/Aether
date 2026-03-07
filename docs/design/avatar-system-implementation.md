# Avatar System (task-008)

Added first-pass avatar domain abstractions to support tracking, animation blending, lip-sync, and policy-driven quality tiers.

## Implemented API surface

- Added crate `aether-avatar` with modules:
  - `tracking`: 3-point/6-point tracking source configuration and IK payload models.
  - `animation`: procedural locomotion/gesture state machine and blend descriptors.
  - `lipsync`: viseme pipeline frame contracts.
  - `rating`: performance tiers/constraints (S/A/B/C), budgets, and world minimum controls.
  - `formats`: VRM/custom avatar metadata and import decision models.
  - `lod`: distance-based avatar LOD bands.
- Updated workspace membership to include `aether-avatar`.

## Mapping to acceptance criteria

- `#1` IK source options are represented by `TrackingSource` and `IkConfiguration`.
- `#2` procedural states and gestures are represented in `ProceduralStateMachine`/`ProceduralGesture`.
- `#3` `BlendCurve`/`BlendTransition` models the animation blending pathway.
- `#4` `LipSyncFrame` and `LipSyncConfig` expose audio→viseme contracts.
- `#5` `AvatarRatingBucket` with `BudgetConstraint` and `PerformanceOverride` provide minimum enforcement hooks.
- `#6` `AvatarFormat` includes VRM formats plus custom Aether binary format.
- `#7` `AvatarLodProfile` defines distance-based LOD transitions.

## Remaining implementation work

- Add runtime evaluators, IK solve routines, and animation graph execution.
- Implement real VRM/custom parser/validator and asset moderation integration.
