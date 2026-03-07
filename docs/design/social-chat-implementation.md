# Social & Chat (task-013)

Added social system primitives to support friend relations, groups, presence, and chat channel contracts.

## Implemented API surface

- Added crate `aether-social` with modules:
  - `friend`: friend lifecycle states and request envelopes.
  - `group`: party/group management and membership data.
  - `presence`: online/offline/in-world presence state.
  - `chat`: DM/group/world channel message contracts.
  - `sharding`: user_id-based shard policy and partitioning helper.
- Updated workspace membership to include `aether-social`.

## Mapping to acceptance criteria

- `#1` Friend request and block operations captured via `FriendRequest` variants.
- `#2` Group creation/ownership/member handling represented by `Group` and related events.
- `#3` Presence data model has online/offline/in-world plus world location.
- `#4` Real-time channel/message primitives model DM/group/world communication and message payload kinds.
- `#5` `ShardMapPolicy` provides sharding hints aligned with user_id partitioning.

## Remaining implementation work

- Implement persistence and delivery guarantees for message ordering.
- Add moderation, mute/ban, and anti-spam hooks.
- Wire world/localized chat relay with spatial voice service.
