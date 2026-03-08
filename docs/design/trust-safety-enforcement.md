# Trust & Safety Enforcement (task-021)

## Background

The `aether-trust-safety` crate currently defines data models (structs/enums) for personal space bubbles, visibility modes, parental controls, and moderation tools, but contains no enforcement logic. These types are inert -- nothing in the engine actually evaluates them to produce runtime behavior.

## Why

VR environments require strong personal safety guarantees. Users must be able to:
- Prevent other avatars from entering their personal space.
- Block other users with mutual invisibility.
- Enforce parental controls (age gates, time limits, content categories).
- Control their own visibility to others.
- Quickly escape to a safe zone when feeling threatened.

Without enforcement logic, the existing type definitions provide no protection.

## What

Implement enforcement functions for five subsystems:

1. **Personal space bubble** -- compute push-away forces on nearby avatars.
2. **User blocking** -- mutual invisibility and interaction filtering.
3. **Parental controls** -- age-gate checks, daily time limit enforcement, category blocking.
4. **Visibility modes** -- determine whether user A can see user B.
5. **Safety zone** -- instant teleport to a safe spawn point.

## How

All enforcement is implemented as pure functions operating on the existing types plus minimal new types. No hardware or network dependencies -- everything is unit-testable.

### Module structure

```
crates/aether-trust-safety/src/
  lib.rs              -- re-exports (updated)
  control.rs          -- existing PersonalSpaceBubble, etc.
  moderation.rs       -- existing moderation tools
  parental.rs         -- existing ParentalControl (extended with enforcement)
  visibility.rs       -- existing VisibilityMode (extended with enforcement)
  personal_space.rs   -- NEW: push force computation
  blocking.rs         -- NEW: mutual blocking system
  safety_zone.rs      -- NEW: safe-spawn teleport
```

### Detailed design

#### personal_space.rs

```rust
pub struct Vec3 { pub x: f32, pub y: f32, pub z: f32 }

pub struct PushResult {
    pub target_id: u64,
    pub displacement: Vec3,  // direction and magnitude to push
}

pub fn compute_push(bubble: &PersonalSpaceBubble, self_pos: &Vec3,
                     other_id: u64, other_pos: &Vec3) -> Option<PushResult>
```

Returns `Some(PushResult)` when `other_pos` is within `bubble.radius_m` of `self_pos`. The displacement vector points away from `self_pos` with magnitude proportional to `push_force * (radius - distance) / radius`.

#### blocking.rs

```rust
pub struct BlockList { blocked: HashSet<u64> }

impl BlockList {
    pub fn block(&mut self, user_id: u64);
    pub fn unblock(&mut self, user_id: u64);
    pub fn is_blocked(&self, user_id: u64) -> bool;
    pub fn is_mutually_blocked(a: &BlockList, a_id: u64, b: &BlockList, b_id: u64) -> bool;
    pub fn filter_visible<'a>(&self, user_ids: &'a [u64]) -> Vec<u64>;
}
```

Mutual blocking: if A blocks B OR B blocks A, neither can see the other.

#### parental.rs (extend existing)

Add enforcement functions:

```rust
pub fn check_age_gate(control: &ParentalControl, world_min_age: Option<u8>, user_age: u8) -> bool;
pub fn check_time_remaining(limit: &TimeLimit, minutes_used: u32) -> TimeLimitStatus;
pub fn is_category_allowed(control: &ParentalControl, category: &str) -> bool;

pub enum TimeLimitStatus { Allowed { remaining: u32 }, Warning { remaining: u32 }, Expired }
```

Warning threshold: 5 minutes remaining.

#### visibility.rs (extend existing)

Add enforcement:

```rust
pub fn can_see(observer: &VisibleScope, target: &VisibleScope,
               are_friends: bool) -> bool;
```

Rules:
- Invisible targets are never visible (except to themselves, handled by caller).
- FriendsOnly targets are visible only if `are_friends` is true.
- Visible targets are always visible.

#### safety_zone.rs

```rust
pub struct SafeZone { pub spawn_point: Vec3, pub world_id: String }

pub struct TeleportRequest { pub user_id: u64, pub destination: Vec3, pub world_id: String }

pub fn trigger_safety_teleport(user_id: u64, zone: &SafeZone) -> TeleportRequest;
pub fn is_panic_gesture(gesture_name: &str) -> bool;
```

The panic gesture is recognized by name (configurable list, defaults: `"panic"`, `"safety"`, `"escape"`).

### Database design

No database changes -- all state is in-memory per session.

### API design

All functions are crate-public. No HTTP or network APIs in this crate.

### Test design

Each module has comprehensive unit tests:
- **personal_space**: bubble disabled, outside radius, on boundary, inside radius, zero distance
- **blocking**: block/unblock, mutual blocking, filter_visible
- **parental**: age gate pass/fail, time limit allowed/warning/expired, category checks
- **visibility**: all mode combinations with/without friendship
- **safety_zone**: teleport request, panic gesture recognition
