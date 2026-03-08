# Multiplayer Runtime Design (task-029)

## Background

The `aether-world-runtime` crate currently provides world loading, chunk streaming, zone rebalancing, and lifecycle management. It lacks the multiplayer game loop primitives needed for a single-server authoritative simulation: tick scheduling, input reconciliation, entity prediction/interpolation, state synchronization, RPCs, event distribution, and player session management.

## Why

A VR engine serving multiple players in a shared world needs an authoritative server simulation to prevent cheating, resolve conflicts, and provide a consistent experience. Clients must send inputs to the server, which validates and applies them, then broadcasts the resulting state. Client-side prediction and interpolation hide network latency.

## What

Add seven modules to `aether-world-runtime`:

| Module | Responsibility |
|--------|---------------|
| `tick` | Fixed-rate server tick loop with accumulator |
| `input_buffer` | Per-player input buffering, ordering, validation |
| `prediction` | Entity state interpolation and client-side prediction snapshots |
| `state_sync` | Delta-based entity state broadcast via reliable/unreliable channels |
| `rpc` | Typed RPC dispatch between client and server |
| `session` | Player session lifecycle (join, active, disconnect, reconnect) |
| `events` | Game event distribution with interest-based filtering |

## How

### Architecture

```
Client -> PlayerInput -> InputBuffer -> ServerTick -> WorldState
                                              |
                                              v
                                        StateSyncManager -> StateSnapshot -> Client
                                              |
                                        RpcDispatcher -> RpcResponse -> Client
                                        EventDispatcher -> GameEvent -> Client
```

All modules are pure data-driven with no async or threading. The caller drives the tick loop externally.

### Detailed Design

#### 1. Server Tick (`tick.rs`)

A `TickScheduler` manages fixed-timestep simulation. It uses a time accumulator pattern: the caller provides elapsed wall-clock time, and the scheduler determines how many simulation ticks to run.

```rust
struct TickScheduler { tick_rate_hz: u32, tick_number: u64, accumulator_us: u64 }
```

Key method: `update(elapsed_us) -> Vec<ServerTick>` returns 0..N ticks to process this frame, capped by `max_ticks_per_update` to prevent spiral-of-death.

#### 2. Input Buffer (`input_buffer.rs`)

Per-player circular buffer that stores `PlayerInput` frames indexed by tick number. Validates ordering (monotonically increasing ticks), rejects duplicates, and provides inputs for a given tick across all players.

```rust
struct InputBuffer { buffers: HashMap<PlayerId, VecDeque<PlayerInput>>, max_buffer_size: usize }
```

#### 3. Entity Prediction (`prediction.rs`)

Stores timestamped `EntityState` snapshots. Provides linear interpolation between two snapshots at a given time fraction. For client-side prediction, stores predicted states and computes correction deltas when server state arrives.

```rust
struct InterpolationBuffer { snapshots: VecDeque<EntityState>, max_snapshots: usize }
```

Interpolation formula: `lerp(a, b, t)` for position/velocity, `slerp(a, b, t)` for quaternion rotation.

#### 4. State Sync (`state_sync.rs`)

Tracks per-entity state and generates delta snapshots. Each entity has a "last acknowledged tick" per client. On each tick, the manager compares current state against last-acked state to produce a diff.

Channel types:
- **Reliable**: position, health, inventory changes (guaranteed delivery)
- **Unreliable**: velocity, rotation updates (latest-wins)

```rust
struct StateSyncManager { entities: HashMap<u64, EntityState>, client_acks: HashMap<PlayerId, HashMap<u64, u64>> }
```

#### 5. RPC System (`rpc.rs`)

Request/response RPC with string-based method names. Handlers are registered by name and invoked with serialized payloads. Supports both client-to-server and server-to-client directions.

```rust
struct RpcDispatcher { handlers: HashMap<String, Box<dyn Fn(RpcRequest) -> RpcResponse>> }
```

#### 6. Player Session (`session.rs`)

Manages player lifecycle: Connecting -> Active -> Disconnected -> Reconnecting -> Active (or removed). Tracks connection metadata, last input tick, and reconnect window.

```rust
struct SessionManager { sessions: HashMap<PlayerId, PlayerSession>, reconnect_window_ms: u64 }
```

#### 7. Event Distribution (`events.rs`)

Broadcasts game events to interested clients. Events have a scope (Global, NearEntity, Player-specific). The dispatcher filters recipients based on scope.

```rust
struct EventDispatcher { pending_events: Vec<GameEvent> }
```

### Database Design

N/A -- all state is in-memory within the runtime.

### API Design

All modules expose pure functions or structs with methods. No network I/O; the caller is responsible for serialization and transport.

### Test Design

Each module has comprehensive unit tests:
- **tick**: timing accuracy, max-tick capping, zero-elapsed edge case
- **input_buffer**: ordering, duplicate rejection, multi-player retrieval, buffer overflow
- **prediction**: interpolation accuracy, snapshot management, correction deltas
- **state_sync**: delta generation, ack tracking, channel classification, full-state fallback
- **rpc**: handler registration, dispatch, missing handler, payload round-trip
- **session**: full lifecycle, reconnect window, timeout expiry, concurrent sessions
- **events**: scope filtering, broadcast, per-player delivery

### Dependencies

```toml
serde = { version = "1", features = ["derive"] }
uuid = { version = "1", features = ["v4"] }
```
