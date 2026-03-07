# Persistence & Crash Recovery (task-006)

This milestone defines world-state persistence policy and WAL/transaction abstraction used by runtime services.

## Implemented API surface

- Added crate `aether-persistence` with persistence primitives:
  - `config` for world persistence profile and classification (`stateful` vs `stateless`).
  - `snapshot` for periodic snapshot cadence, window timing, and sampling caps.
  - `wal` for in-memory WAL write coordination, durability mode (`FsyncBeforeAck`), and replay stream.
  - `placement` for session placement hints (StatefulSet vs Deployment).
  - `transactions` for synchronous critical-state mutation envelopes and ACK tracking.
- Updated workspace membership to include `aether-persistence`.

## Mapping to acceptance criteria

- `#1` critical state sync objects and mutation envelopes are represented in `transactions` with synchronous result paths.
- `#2` snapshot cadence default is 5s (`DEFAULT_EPHEMERAL_SNAPSHOT_INTERVAL`) with replayable window checks.
- `#3` WAL support includes fsync-before-ack durability mode and durable replay records.
- `#4` `PodRuntimeClass::StatefulSet` models PVC-backed world pod class.
- `#5` `PodRuntimeClass::Deployment` models stateless worlds.
- `#6` `WorldManifest::make_placement_hint` and `PodPlacementHint` provide session routing metadata for placement.

## Remaining implementation work

- Bind this crate into world/session orchestration services.
- Replace in-memory coordinator with real durable store and WAL filesystem implementation.
- Add crash and restart integration tests around replay + pending mutation drain.
