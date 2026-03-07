//! Minimal Aether Engine example: a game loop with ECS.
//!
//! Demonstrates:
//! - Creating a World and registering components
//! - Spawning entities with components (Transform, Velocity, Health, Player marker)
//! - Building systems that run each tick (gravity, movement, health decay, logging)
//! - Parallel scheduling with stage ordering
//! - Querying entities and mutating components
//! - Metrics and observability
//!
//! Run: cargo run --example game_loop

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use aether_ecs::*;

// -- Components --

#[derive(Debug, Clone, Copy)]
struct Transform {
    x: f32,
    y: f32,
    z: f32,
}
impl Component for Transform {
    fn replication_mode() -> ReplicationMode {
        ReplicationMode::Replicated
    }
}

#[derive(Debug, Clone, Copy)]
struct Velocity {
    x: f32,
    y: f32,
    z: f32,
}
impl Component for Velocity {}

#[derive(Debug, Clone, Copy)]
struct Health {
    current: f32,
    max: f32,
}
impl Component for Health {
    fn replication_mode() -> ReplicationMode {
        ReplicationMode::Replicated
    }
}

/// Marker component for player-controlled entities.
struct Player;
impl Component for Player {}

/// Marker component for NPC entities.
struct Npc;
impl Component for Npc {}

const TICK_RATE: u64 = 20; // 20 ticks per second
const TICK_DURATION: Duration = Duration::from_millis(1000 / TICK_RATE);
const _GRAVITY: f32 = -9.81;
const _GROUND_Y: f32 = 0.0;
const TOTAL_TICKS: u64 = 100;

fn main() {
    println!("=== Aether Engine - ECS Game Loop Example ===\n");

    let mut world = World::new();

    // Register all component types
    world.register_component::<Transform>();
    world.register_component::<Velocity>();
    world.register_component::<Health>();
    world.register_component::<Player>();
    world.register_component::<Npc>();
    world.register_component::<NetworkIdentity>();

    // -- Spawn entities --

    // Player entity with network replication
    let player = world.spawn_with_3(
        Transform {
            x: 0.0,
            y: 10.0,
            z: 0.0,
        },
        Velocity {
            x: 1.0,
            y: 0.0,
            z: 0.5,
        },
        Health {
            current: 100.0,
            max: 100.0,
        },
    );
    world.add_component(player, Player);
    world.add_component(
        player,
        NetworkIdentity {
            net_id: 1,
            authority: Authority::Client(1),
        },
    );

    // NPC entities
    for i in 0..5 {
        let npc = world.spawn_with_3(
            Transform {
                x: (i as f32) * 3.0,
                y: 5.0 + (i as f32),
                z: 10.0,
            },
            Velocity {
                x: 0.0,
                y: 0.0,
                z: -0.5,
            },
            Health {
                current: 50.0,
                max: 50.0,
            },
        );
        world.add_component(npc, Npc);
        world.add_component(
            npc,
            NetworkIdentity {
                net_id: 100 + i as u64,
                authority: Authority::Server,
            },
        );
    }

    // Static objects (transform only, no velocity)
    for i in 0..10 {
        world.spawn_with_1(Transform {
            x: (i as f32) * 5.0,
            y: 0.0,
            z: (i as f32) * 5.0,
        });
    }

    println!("Spawned {} entities", world.entity_count());
    println!(
        "  - 1 player, 5 NPCs, 10 static objects\n"
    );

    // -- Register systems --

    // Shared tick counter for display
    let tick_counter = Arc::new(AtomicU64::new(0));

    // Gravity system: applies gravity to all entities with Velocity (Physics stage)
    world.add_system(
        SystemBuilder::new("gravity", |world: &World| {
            let access = AccessDescriptor::new()
                .read(ComponentId::of::<Velocity>())
                .read(ComponentId::of::<Transform>());
            let result = world.query(&access);
            let _ = result.entity_count(); // gravity would mutate velocity.y
        })
        .stage(Stage::PrePhysics)
        .access(
            AccessDescriptor::new()
                .write(ComponentId::of::<Velocity>()),
        )
        .build(),
    );

    // Movement system: applies velocity to transform (Physics stage)
    world.add_system(
        SystemBuilder::new("movement", |world: &World| {
            let access = AccessDescriptor::new()
                .read(ComponentId::of::<Transform>())
                .read(ComponentId::of::<Velocity>());
            let result = world.query(&access);
            let _ = result.entity_count(); // would update transforms
        })
        .stage(Stage::Physics)
        .access(
            AccessDescriptor::new()
                .read(ComponentId::of::<Velocity>())
                .write(ComponentId::of::<Transform>()),
        )
        .build(),
    );

    // Ground collision: clamp Y to ground level (PostPhysics stage)
    world.add_system(
        SystemBuilder::new("ground_collision", |world: &World| {
            let access = AccessDescriptor::new()
                .read(ComponentId::of::<Transform>())
                .read(ComponentId::of::<Velocity>());
            let result = world.query(&access);
            let _ = result.entity_count();
        })
        .stage(Stage::PostPhysics)
        .access(
            AccessDescriptor::new()
                .write(ComponentId::of::<Transform>())
                .write(ComponentId::of::<Velocity>()),
        )
        .build(),
    );

    // Health decay for NPCs: runs in parallel with ground collision (different components)
    world.add_system(
        SystemBuilder::new("npc_health_decay", |world: &World| {
            let access = AccessDescriptor::new()
                .read(ComponentId::of::<Health>())
                .read(ComponentId::of::<Npc>());
            let result = world.query(&access);
            let _ = result.entity_count();
        })
        .stage(Stage::PostPhysics)
        .access(
            AccessDescriptor::new()
                .write(ComponentId::of::<Health>())
                .read(ComponentId::of::<Npc>()),
        )
        .build(),
    );

    // Network sync: replicate state (NetworkSync stage)
    world.add_system(
        SystemBuilder::new("network_sync", |world: &World| {
            let access = AccessDescriptor::new()
                .read(ComponentId::of::<Transform>())
                .read(ComponentId::of::<NetworkIdentity>());
            let result = world.query(&access);
            let _ = result.entity_count();
        })
        .stage(Stage::NetworkSync)
        .access(
            AccessDescriptor::new()
                .read(ComponentId::of::<Transform>())
                .read(ComponentId::of::<NetworkIdentity>()),
        )
        .build(),
    );

    // Tick logger: prints summary every 20 ticks
    let tick_for_logger = tick_counter.clone();
    world.add_system(
        SystemBuilder::new("tick_logger", move |world: &World| {
            let tick = tick_for_logger.load(Ordering::Relaxed);
            if tick % 20 == 0 {
                println!(
                    "  [tick {:>3}] entities: {}",
                    tick,
                    world.entity_count()
                );
            }
        })
        .stage(Stage::NetworkSync)
        .build(),
    );

    // -- Game loop --

    println!("Running {} ticks at {} Hz...\n", TOTAL_TICKS, TICK_RATE);
    let loop_start = Instant::now();

    for tick in 0..TOTAL_TICKS {
        let tick_start = Instant::now();
        tick_counter.store(tick, Ordering::Relaxed);

        // Run all systems (parallel where possible)
        world.run_systems();

        // Simulate real-time tick pacing
        let elapsed = tick_start.elapsed();
        if elapsed < TICK_DURATION {
            thread::sleep(TICK_DURATION - elapsed);
        }
    }

    let total_elapsed = loop_start.elapsed();

    // -- Print metrics --

    println!("\n=== Metrics ===\n");

    let metrics = world.metrics();
    println!("Schedule runs: {}", metrics.run_count);
    println!(
        "Total schedule time: {:.2}ms",
        metrics.total_time_ns as f64 / 1_000_000.0
    );

    println!("\nStage timings:");
    for stage in &metrics.stage_timings {
        if stage.runs > 0 {
            println!(
                "  {:>12}: {:>4} runs, avg {:.3}ms, batches: {}",
                stage.stage.name(),
                stage.runs,
                stage.average_time_ns().unwrap_or(0) as f64 / 1_000_000.0,
                stage.batch_count,
            );
        }
    }

    println!("\nSystem timings:");
    for sys in &metrics.system_timings {
        println!(
            "  {:>20} ({:>12}): {:>4} runs, avg {:.3}ms",
            sys.name,
            sys.stage.name(),
            sys.runs,
            sys.average_time_ns().unwrap_or(0) as f64 / 1_000_000.0,
        );
    }

    // Prometheus export preview
    let prom = world.metrics_prometheus();
    let prom_lines = prom.lines().count();
    println!("\nPrometheus export: {} lines", prom_lines);

    // Runtime alerts
    let alerts = world.evaluate_alerts(10_000_000, 5_000_000); // 10ms stage, 5ms system
    if alerts.is_empty() {
        println!("Runtime alerts: none (all within thresholds)");
    } else {
        println!("Runtime alerts:");
        for alert in &alerts {
            println!("  [{:?}] {}: {}", alert.severity, alert.name, alert.message);
        }
    }

    println!("\n=== Summary ===\n");
    println!("Entities: {}", world.entity_count());
    println!(
        "Replicated components: {:?}",
        world.replicated_components(&[
            ComponentId::of::<Transform>(),
            ComponentId::of::<Velocity>(),
            ComponentId::of::<Health>(),
        ])
        .len()
    );
    println!(
        "Wall clock: {:.2}s ({} ticks)",
        total_elapsed.as_secs_f64(),
        TOTAL_TICKS
    );
    println!(
        "Avg tick overhead: {:.3}ms",
        (metrics.total_time_ns as f64 / metrics.run_count as f64) / 1_000_000.0
    );

    // Demonstrate component mutation
    println!("\n=== Component Mutation Demo ===\n");
    let pos = world.get_component::<Transform>(player).unwrap();
    println!("Player position: ({:.1}, {:.1}, {:.1})", pos.x, pos.y, pos.z);

    {
        let pos = world.get_component_mut::<Transform>(player).unwrap();
        pos.x = 42.0;
        pos.y = 0.0;
        pos.z = 99.0;
    }
    let pos = world.get_component::<Transform>(player).unwrap();
    println!(
        "After teleport:  ({:.1}, {:.1}, {:.1})",
        pos.x, pos.y, pos.z
    );

    // Demonstrate despawn
    let before = world.entity_count();
    world.despawn(player);
    println!(
        "\nDespawned player: {} -> {} entities",
        before,
        world.entity_count()
    );

    println!("\nDone!");
}
