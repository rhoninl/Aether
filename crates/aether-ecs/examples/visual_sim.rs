//! Visual 2D simulation using the Aether ECS.
//!
//! Opens a window showing entities as colored shapes:
//!   - Green circle: Player (keyboard controlled)
//!   - Red circles: NPCs (patrol and chase player)
//!   - Blue squares: Static objects
//!   - Yellow dots: Projectiles
//!
//! Controls:
//!   W/A/S/D or Arrow keys: Move player
//!   Space: Shoot projectile
//!   ESC: Quit
//!
//! Run: cargo run --example visual_sim -p aether-ecs

use std::time::Instant;

use minifb::{Key, Window, WindowOptions};

use aether_ecs::*;

// -- Constants --

const WIDTH: usize = 800;
const HEIGHT: usize = 600;
const TICK_RATE: f32 = 60.0;
const DT: f32 = 1.0 / TICK_RATE;

const PLAYER_SPEED: f32 = 200.0;
const NPC_SPEED: f32 = 80.0;
const PROJECTILE_SPEED: f32 = 400.0;
const PROJECTILE_LIFETIME: f32 = 2.0;
const NPC_CHASE_RANGE: f32 = 150.0;
const NPC_COUNT: usize = 8;
const STATIC_COUNT: usize = 12;
const SHOOT_COOLDOWN: u64 = 10;

// Colors (ARGB)
const COLOR_BG: u32 = 0xFF1a1a2e;
const COLOR_PLAYER: u32 = 0xFF00ff88;
const COLOR_NPC: u32 = 0xFFff4466;
const COLOR_NPC_CHASE: u32 = 0xFFff8800;
const COLOR_STATIC: u32 = 0xFF4488ff;
const COLOR_PROJECTILE: u32 = 0xFFffdd00;
const COLOR_GRID: u32 = 0xFF222244;
const COLOR_HUD_BG: u32 = 0xCC000000;

// -- Components --

#[derive(Debug, Clone, Copy)]
struct Transform {
    x: f32,
    y: f32,
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

#[derive(Clone, Copy)]
struct NpcState {
    home_x: f32,
    home_y: f32,
    patrol_angle: f32,
    chasing: bool,
}
impl Component for NpcState {}

struct ProjectileState {
    lifetime: f32,
}
impl Component for ProjectileState {}

// -- Rendering helpers --

fn draw_filled_circle(buf: &mut [u32], cx: f32, cy: f32, r: f32, color: u32) {
    let r2 = r * r;
    let x0 = ((cx - r) as i32).max(0) as usize;
    let x1 = ((cx + r) as i32 + 1).min(WIDTH as i32) as usize;
    let y0 = ((cy - r) as i32).max(0) as usize;
    let y1 = ((cy + r) as i32 + 1).min(HEIGHT as i32) as usize;
    for py in y0..y1 {
        for px in x0..x1 {
            let dx = px as f32 - cx;
            let dy = py as f32 - cy;
            if dx * dx + dy * dy <= r2 {
                buf[py * WIDTH + px] = color;
            }
        }
    }
}

fn draw_circle_outline(buf: &mut [u32], cx: f32, cy: f32, r: f32, color: u32) {
    let steps = (r * 8.0).max(20.0) as i32;
    for i in 0..steps {
        let angle = (i as f32 / steps as f32) * std::f32::consts::TAU;
        let px = (cx + r * angle.cos()) as i32;
        let py = (cy + r * angle.sin()) as i32;
        if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
            buf[py as usize * WIDTH + px as usize] = color;
        }
    }
}

fn draw_rect(buf: &mut [u32], cx: f32, cy: f32, half: f32, color: u32) {
    let x0 = ((cx - half) as i32).max(0) as usize;
    let x1 = ((cx + half) as i32 + 1).min(WIDTH as i32) as usize;
    let y0 = ((cy - half) as i32).max(0) as usize;
    let y1 = ((cy + half) as i32 + 1).min(HEIGHT as i32) as usize;
    for py in y0..y1 {
        for px in x0..x1 {
            buf[py * WIDTH + px] = color;
        }
    }
}

fn draw_health_bar(buf: &mut [u32], cx: f32, cy: f32, fraction: f32) {
    let bar_w = 20.0_f32;
    let bar_h = 3.0_f32;
    let by = cy - 16.0;
    // Background
    draw_rect(buf, cx, by, bar_w / 2.0, 0xFF333333);
    // Fill
    let color = if fraction > 0.5 {
        0xFF00ff00
    } else if fraction > 0.25 {
        0xFFffaa00
    } else {
        0xFFff0000
    };
    let bx = cx - bar_w / 2.0;
    let fill_w = bar_w * fraction;
    let fx0 = (bx as i32).max(0) as usize;
    let fx1 = ((bx + fill_w) as i32 + 1).min(WIDTH as i32) as usize;
    let fy0 = ((by - bar_h / 2.0) as i32).max(0) as usize;
    let fy1 = ((by + bar_h / 2.0) as i32 + 1).min(HEIGHT as i32) as usize;
    for py in fy0..fy1 {
        for px in fx0..fx1 {
            buf[py * WIDTH + px] = color;
        }
    }
}

fn draw_hud(buf: &mut [u32], entity_count: usize, kills: u32) {
    for y in 0..22 {
        for x in 0..WIDTH {
            buf[y * WIDTH + x] = COLOR_HUD_BG;
        }
    }
    // Legend dots
    let ly = 7.0;
    draw_filled_circle(buf, 10.0, ly + 4.0, 4.0, COLOR_PLAYER);
    draw_filled_circle(buf, 40.0, ly + 4.0, 4.0, COLOR_NPC);
    draw_rect(buf, 70.0, ly + 4.0, 4.0, COLOR_STATIC);
    draw_filled_circle(buf, 100.0, ly + 4.0, 2.0, COLOR_PROJECTILE);

    // Entity count bar
    let bar_x: usize = 130;
    let bar_w = (entity_count * 4).min(WIDTH - bar_x - 200);
    for y in 5..15 {
        for x in bar_x..(bar_x + bar_w) {
            if x < WIDTH {
                buf[y * WIDTH + x] = 0xFF66ff66;
            }
        }
    }

    // Kill count bar (red)
    let kill_x = bar_x + bar_w + 20;
    let kill_w = (kills as usize * 8).min(WIDTH - kill_x - 10);
    for y in 5..15 {
        for x in kill_x..(kill_x + kill_w) {
            if x < WIDTH {
                buf[y * WIDTH + x] = 0xFFff4466;
            }
        }
    }
}

fn spawn_npc(world: &mut World, angle: f32) -> Entity {
    let dist = 180.0;
    let hx = WIDTH as f32 / 2.0 + angle.cos() * dist;
    let hy = HEIGHT as f32 / 2.0 + angle.sin() * dist;
    let npc = world.spawn_with_3(
        Transform { x: hx, y: hy },
        Velocity { x: 0.0, y: 0.0 },
        Health {
            current: 50.0,
            max: 50.0,
        },
    );
    world.add_component(
        npc,
        NpcState {
            home_x: hx,
            home_y: hy,
            patrol_angle: angle,
            chasing: false,
        },
    );
    npc
}

fn wrap(v: f32, max: f32) -> f32 {
    if v < 0.0 {
        v + max
    } else if v >= max {
        v - max
    } else {
        v
    }
}

// -- Main --

fn main() {
    let mut window = Window::new(
        "Aether Engine - ECS Visual Sim [WASD: Move | Space: Shoot | ESC: Quit]",
        WIDTH,
        HEIGHT,
        WindowOptions::default(),
    )
    .expect("failed to create window");

    window.set_target_fps(TICK_RATE as usize);

    let mut buf = vec![0u32; WIDTH * HEIGHT];
    let mut world = World::new();

    // Register components
    world.register_component::<Transform>();
    world.register_component::<Velocity>();
    world.register_component::<Health>();
    world.register_component::<NpcState>();
    world.register_component::<ProjectileState>();

    // Spawn player
    let player = world.spawn_with_3(
        Transform {
            x: WIDTH as f32 / 2.0,
            y: HEIGHT as f32 / 2.0,
        },
        Velocity { x: 0.0, y: 0.0 },
        Health {
            current: 100.0,
            max: 100.0,
        },
    );

    // Spawn NPCs
    let mut npcs: Vec<Entity> = Vec::new();
    for i in 0..NPC_COUNT {
        let angle = (i as f32 / NPC_COUNT as f32) * std::f32::consts::TAU;
        npcs.push(spawn_npc(&mut world, angle));
    }

    // Spawn statics
    let mut statics: Vec<Entity> = Vec::new();
    for i in 0..STATIC_COUNT {
        let angle = (i as f32 / STATIC_COUNT as f32) * std::f32::consts::TAU + 0.3;
        let dist = 100.0 + (i as f32 * 37.0) % 220.0;
        let e = world.spawn_with_1(Transform {
            x: WIDTH as f32 / 2.0 + angle.cos() * dist,
            y: HEIGHT as f32 / 2.0 + angle.sin() * dist,
        });
        statics.push(e);
    }

    let mut projectiles: Vec<Entity> = Vec::new();
    let mut tick: u64 = 0;
    let mut last_shoot_tick: u64 = 0;
    let mut facing_x: f32 = 1.0;
    let mut facing_y: f32 = 0.0;
    let mut kills: u32 = 0;
    let mut fps_timer = Instant::now();
    let mut frame_count: u32 = 0;
    let mut _current_fps: f32 = 60.0;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        tick += 1;
        frame_count += 1;
        if fps_timer.elapsed().as_secs_f32() >= 1.0 {
            _current_fps = frame_count as f32 / fps_timer.elapsed().as_secs_f32();
            frame_count = 0;
            fps_timer = Instant::now();
        }

        // === INPUT ===
        {
            let mut vx = 0.0f32;
            let mut vy = 0.0f32;
            if window.is_key_down(Key::W) || window.is_key_down(Key::Up) { vy -= 1.0; }
            if window.is_key_down(Key::S) || window.is_key_down(Key::Down) { vy += 1.0; }
            if window.is_key_down(Key::A) || window.is_key_down(Key::Left) { vx -= 1.0; }
            if window.is_key_down(Key::D) || window.is_key_down(Key::Right) { vx += 1.0; }
            let len = (vx * vx + vy * vy).sqrt();
            if len > 0.0 {
                vx = (vx / len) * PLAYER_SPEED;
                vy = (vy / len) * PLAYER_SPEED;
                facing_x = vx / PLAYER_SPEED;
                facing_y = vy / PLAYER_SPEED;
            }
            if let Some(vel) = world.get_component_mut::<Velocity>(player) {
                vel.x = vx;
                vel.y = vy;
            }
        }

        // === SHOOT ===
        if window.is_key_down(Key::Space) && tick - last_shoot_tick > SHOOT_COOLDOWN {
            last_shoot_tick = tick;
            if let Some(pos) = world.get_component::<Transform>(player) {
                let (px, py) = (pos.x, pos.y);
                let proj = world.spawn_with_2(
                    Transform { x: px, y: py },
                    Velocity {
                        x: facing_x * PROJECTILE_SPEED,
                        y: facing_y * PROJECTILE_SPEED,
                    },
                );
                world.add_component(proj, ProjectileState { lifetime: PROJECTILE_LIFETIME });
                projectiles.push(proj);
            }
        }

        // === PHYSICS: move entities ===
        // Player
        {
            let (vx, vy) = world
                .get_component::<Velocity>(player)
                .map(|v| (v.x, v.y))
                .unwrap_or((0.0, 0.0));
            if let Some(pos) = world.get_component_mut::<Transform>(player) {
                pos.x = wrap(pos.x + vx * DT, WIDTH as f32);
                pos.y = wrap(pos.y + vy * DT, HEIGHT as f32);
            }
        }
        // NPCs
        for &npc in &npcs {
            let (vx, vy) = world
                .get_component::<Velocity>(npc)
                .map(|v| (v.x, v.y))
                .unwrap_or((0.0, 0.0));
            if let Some(pos) = world.get_component_mut::<Transform>(npc) {
                pos.x = wrap(pos.x + vx * DT, WIDTH as f32);
                pos.y = wrap(pos.y + vy * DT, HEIGHT as f32);
            }
        }
        // Projectiles
        for &proj in &projectiles {
            let (vx, vy) = world
                .get_component::<Velocity>(proj)
                .map(|v| (v.x, v.y))
                .unwrap_or((0.0, 0.0));
            if let Some(pos) = world.get_component_mut::<Transform>(proj) {
                pos.x += vx * DT;
                pos.y += vy * DT;
            }
        }

        // === NPC AI ===
        let player_pos = world
            .get_component::<Transform>(player)
            .map(|t| (t.x, t.y))
            .unwrap_or((0.0, 0.0));

        for &npc in &npcs {
            if !world.is_alive(npc) {
                continue;
            }
            let (ex, ey) = world
                .get_component::<Transform>(npc)
                .map(|t| (t.x, t.y))
                .unwrap_or((0.0, 0.0));
            let dx = player_pos.0 - ex;
            let dy = player_pos.1 - ey;
            let dist = (dx * dx + dy * dy).sqrt();
            let chasing = dist < NPC_CHASE_RANGE;

            let (hx, hy, pa) = world
                .get_component::<NpcState>(npc)
                .map(|n| (n.home_x, n.home_y, n.patrol_angle))
                .unwrap_or((ex, ey, 0.0));

            if let Some(ns) = world.get_component_mut::<NpcState>(npc) {
                ns.chasing = chasing;
                ns.patrol_angle = pa + DT * 0.5;
            }

            if chasing {
                let len = dist.max(1.0);
                if let Some(vel) = world.get_component_mut::<Velocity>(npc) {
                    vel.x = (dx / len) * NPC_SPEED * 1.5;
                    vel.y = (dy / len) * NPC_SPEED * 1.5;
                }
            } else {
                let new_angle = pa + DT * 0.5;
                let tx = hx + new_angle.cos() * 40.0;
                let ty = hy + new_angle.sin() * 40.0;
                let tdx = tx - ex;
                let tdy = ty - ey;
                let tlen = (tdx * tdx + tdy * tdy).sqrt().max(1.0);
                if let Some(vel) = world.get_component_mut::<Velocity>(npc) {
                    vel.x = (tdx / tlen) * NPC_SPEED;
                    vel.y = (tdy / tlen) * NPC_SPEED;
                }
            }
        }

        // === PROJECTILE LIFETIME + COLLISION ===
        {
            let mut dead_projs: Vec<usize> = Vec::new();
            for (i, &proj) in projectiles.iter().enumerate() {
                if !world.is_alive(proj) {
                    dead_projs.push(i);
                    continue;
                }
                // Decay lifetime
                let expired = {
                    let ps = world.get_component_mut::<ProjectileState>(proj).unwrap();
                    ps.lifetime -= DT;
                    ps.lifetime <= 0.0
                };
                // Off-screen check
                let offscreen = world
                    .get_component::<Transform>(proj)
                    .map(|t| t.x < -20.0 || t.x > WIDTH as f32 + 20.0 || t.y < -20.0 || t.y > HEIGHT as f32 + 20.0)
                    .unwrap_or(true);

                if expired || offscreen {
                    world.despawn(proj);
                    dead_projs.push(i);
                    continue;
                }

                // Check collision with NPCs
                let proj_pos = world
                    .get_component::<Transform>(proj)
                    .map(|t| (t.x, t.y))
                    .unwrap_or((0.0, 0.0));

                let mut hit = false;
                for &npc in &npcs {
                    if !world.is_alive(npc) {
                        continue;
                    }
                    let npc_pos = world
                        .get_component::<Transform>(npc)
                        .map(|t| (t.x, t.y))
                        .unwrap_or((0.0, 0.0));
                    let ddx = proj_pos.0 - npc_pos.0;
                    let ddy = proj_pos.1 - npc_pos.1;
                    if ddx * ddx + ddy * ddy < 14.0 * 14.0 {
                        if let Some(hp) = world.get_component_mut::<Health>(npc) {
                            hp.current -= 25.0;
                        }
                        hit = true;
                        break;
                    }
                }
                if hit {
                    world.despawn(proj);
                    dead_projs.push(i);
                }
            }
            // Remove dead projectiles (reverse order)
            dead_projs.sort_unstable();
            for &i in dead_projs.iter().rev() {
                projectiles.swap_remove(i);
            }
        }

        // === RESPAWN DEAD NPCs ===
        for npc_slot in npcs.iter_mut() {
            if !world.is_alive(*npc_slot) {
                kills += 1;
                let angle = (tick as f32 * 1.7 + kills as f32 * 2.3) % std::f32::consts::TAU;
                *npc_slot = spawn_npc(&mut world, angle);
            } else {
                let dead = world
                    .get_component::<Health>(*npc_slot)
                    .map(|h| h.current <= 0.0)
                    .unwrap_or(false);
                if dead {
                    kills += 1;
                    world.despawn(*npc_slot);
                    let angle = (tick as f32 * 1.7 + kills as f32 * 2.3) % std::f32::consts::TAU;
                    *npc_slot = spawn_npc(&mut world, angle);
                }
            }
        }

        // === RENDER ===
        buf.fill(COLOR_BG);

        // Grid
        for y in (0..HEIGHT).step_by(50) {
            for x in 0..WIDTH {
                buf[y * WIDTH + x] = COLOR_GRID;
            }
        }
        for x in (0..WIDTH).step_by(50) {
            for y in 0..HEIGHT {
                buf[y * WIDTH + x] = COLOR_GRID;
            }
        }

        // Static objects
        for &e in &statics {
            if let Some(pos) = world.get_component::<Transform>(e) {
                draw_rect(&mut buf, pos.x, pos.y, 8.0, COLOR_STATIC);
            }
        }

        // NPCs
        for &npc in &npcs {
            if !world.is_alive(npc) {
                continue;
            }
            let (nx, ny) = match world.get_component::<Transform>(npc) {
                Some(t) => (t.x, t.y),
                None => continue,
            };
            let chasing = world
                .get_component::<NpcState>(npc)
                .map(|n| n.chasing)
                .unwrap_or(false);
            let color = if chasing { COLOR_NPC_CHASE } else { COLOR_NPC };
            draw_filled_circle(&mut buf, nx, ny, 10.0, color);
            if let Some(hp) = world.get_component::<Health>(npc) {
                draw_health_bar(&mut buf, nx, ny, (hp.current / hp.max).max(0.0));
            }
            if chasing {
                draw_circle_outline(&mut buf, nx, ny, NPC_CHASE_RANGE, 0x33ff8800);
            }
        }

        // Projectiles
        for &proj in &projectiles {
            if let Some(pos) = world.get_component::<Transform>(proj) {
                draw_filled_circle(&mut buf, pos.x, pos.y, 3.0, COLOR_PROJECTILE);
            }
        }

        // Player
        if let Some(pos) = world.get_component::<Transform>(player) {
            draw_filled_circle(&mut buf, pos.x, pos.y, 12.0, COLOR_PLAYER);
            draw_filled_circle(
                &mut buf,
                pos.x + facing_x * 18.0,
                pos.y + facing_y * 18.0,
                3.0,
                0xFFffffff,
            );
        }

        // HUD
        draw_hud(&mut buf, world.entity_count(), kills);

        window.update_with_buffer(&buf, WIDTH, HEIGHT).unwrap();
    }

    println!("Kills: {}, Ticks: {}, Final entities: {}", kills, tick, world.entity_count());
}
