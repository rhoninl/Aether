mod mock_apis;
mod render;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aether_lua::bridge::{self, ScriptContext};
use aether_lua::error;
use aether_lua::metrics::LuaMetrics;
use aether_lua::script;
use aether_lua::vm::LuaVm;
use aether_scripting::{ScriptDescriptor, ScriptExecutionUsage, ScriptRuntime, WorldScriptScheduler};
use minifb::{Key, Window, WindowOptions};

use mock_apis::{
    MockAudioApi, MockEntityApi, MockNetworkApi, MockPhysicsApi, MockStorageApi,
};
use render::{Camera, FrameBuffer, HEIGHT, WIDTH};

const TICK_RATE_HZ: u32 = 60;
const DT: f32 = 1.0 / TICK_RATE_HZ as f32;
const MEMORY_LIMIT: usize = 4 * 1024 * 1024; // 4 MB per script
const CPU_BUDGET: Duration = Duration::from_millis(5);

const CAM_ORBIT_SPEED: f32 = 0.03;
const CAM_ZOOM_SPEED: f32 = 0.5;

// NPC patrol waypoints (matching npc_patrol.lua)
const WAYPOINTS: [[f32; 3]; 4] = [
    [5.0, 0.0, 5.0],
    [15.0, 0.0, 5.0],
    [15.0, 0.0, 15.0],
    [5.0, 0.0, 15.0],
];

fn main() {
    // Create shared API implementations (mock for this demo)
    let entity_api = Arc::new(Mutex::new(MockEntityApi::new()));
    let physics_api = Arc::new(Mutex::new(MockPhysicsApi::new()));
    let audio_api = Arc::new(Mutex::new(MockAudioApi::new()));
    let network_api = Arc::new(Mutex::new(MockNetworkApi::new()));
    let storage_api = Arc::new(Mutex::new(MockStorageApi::new()));

    let mut scheduler = WorldScriptScheduler::default();
    let metrics = LuaMetrics::new();
    let now = Instant::now();

    // Discover and load scripts
    let script_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts");
    let scripts = discover_scripts(script_dir);
    let vms = load_scripts(
        &scripts,
        &entity_api,
        &physics_api,
        &audio_api,
        &network_api,
        &storage_api,
        &mut scheduler,
        &metrics,
        now,
    );

    metrics.set_scripts_active(vms.len() as u64);

    // Window and renderer
    let mut window = Window::new(
        "Aether Lua Scripting Demo (Arrows=orbit, QE=zoom, ESC=quit)",
        WIDTH,
        HEIGHT,
        WindowOptions {
            resize: false,
            ..WindowOptions::default()
        },
    )
    .expect("failed to create window");

    window.set_target_fps(60);

    let mut fb = FrameBuffer::new();
    let mut camera = Camera::default();
    let mut cam_angle: f32 = std::f32::consts::PI * 0.75;
    let mut cam_pitch: f32 = 0.5;
    let mut cam_dist: f32 = 28.0;

    let mut tick: u64 = 0;
    let mut frame_time_ms: f32 = 0.0;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let frame_start = Instant::now();
        tick += 1;

        // Camera controls
        if window.is_key_down(Key::Left) {
            cam_angle += CAM_ORBIT_SPEED;
        }
        if window.is_key_down(Key::Right) {
            cam_angle -= CAM_ORBIT_SPEED;
        }
        if window.is_key_down(Key::Up) {
            cam_pitch = (cam_pitch + CAM_ORBIT_SPEED).min(1.4);
        }
        if window.is_key_down(Key::Down) {
            cam_pitch = (cam_pitch - CAM_ORBIT_SPEED).max(0.05);
        }
        if window.is_key_down(Key::Q) {
            cam_dist = (cam_dist - CAM_ZOOM_SPEED).max(8.0);
        }
        if window.is_key_down(Key::E) {
            cam_dist = (cam_dist + CAM_ZOOM_SPEED).min(60.0);
        }

        // Update camera orbit around scene center
        camera.target = [10.0, 0.0, 10.0];
        camera.eye[0] = camera.target[0] + cam_dist * cam_pitch.cos() * cam_angle.cos();
        camera.eye[1] = camera.target[1] + cam_dist * cam_pitch.sin();
        camera.eye[2] = camera.target[2] + cam_dist * cam_pitch.cos() * cam_angle.sin();

        // Run Lua scripts for this tick
        run_tick(tick, &vms, &mut scheduler, &metrics);

        // Render
        fb.clear();
        render::render_ground(&mut fb, &camera);

        // Render waypoints
        for wp in &WAYPOINTS {
            render::render_waypoint(&mut fb, &camera, *wp);
        }

        // Render entities from mock API
        {
            let api = entity_api.lock().unwrap();

            // Shadows first
            for (_id, _template, pos) in api.entities() {
                render::render_shadow(&mut fb, &camera, [pos.x, pos.y, pos.z], 0.5);
            }

            // Then entities
            for (_id, template, pos) in api.entities() {
                let p = [pos.x, pos.y, pos.z];
                match template {
                    "patrol_guard" => render::render_player(&mut fb, &camera, p, render::color_for_template(template)),
                    "hello_npc" => render::render_sphere(&mut fb, &camera, p, 0.4, render::color_for_template(template)),
                    "physics_sphere" => render::render_sphere(&mut fb, &camera, p, 0.5, render::color_for_template(template)),
                    "physics_cube" => render::render_cube(&mut fb, &camera, p, 0.5, render::color_for_template(template)),
                    "static_platform" => render::render_cube(&mut fb, &camera, p, 1.0, render::color_for_template(template)),
                    _ => render::render_sphere(&mut fb, &camera, p, 0.3, render::color_for_template(template)),
                }
            }
        }

        // HUD
        let entity_count = entity_api.lock().unwrap().alive_count();
        let error_count = metrics.snapshot().errors_total;
        render::render_hud(&mut fb, tick, entity_count, vms.len(), error_count, frame_time_ms);

        window.update_with_buffer(&fb.buf, WIDTH, HEIGHT).unwrap();
        frame_time_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
    }
}

fn run_tick(
    tick: u64,
    vms: &[(LuaVm, String)],
    scheduler: &mut WorldScriptScheduler,
    metrics: &LuaMetrics,
) {
    let tick_plan = scheduler.plan_tick(Instant::now());

    for (vm, hook_name) in vms {
        let script_id = vm.script_id();
        if tick_plan.deferred.contains(&script_id) {
            continue;
        }

        vm.set_time(DT, tick).ok();

        let budget = scheduler.script_cpu_budget(script_id);
        let start = Instant::now();
        vm.set_instruction_hook(budget, start);

        let result = vm.call_hook(hook_name);
        let elapsed = start.elapsed();

        let usage = ScriptExecutionUsage {
            script_id,
            cpu_used: elapsed,
        };
        scheduler.record_usage(Instant::now(), &[usage]);

        if let Err(e) = result {
            let formatted = error::format_lua_error(&e);
            eprintln!("[tick {tick}] ERROR in {}: {formatted}", vm.script_name());
            metrics.inc_errors();
        }
    }
}

fn load_scripts(
    scripts: &[(String, String)],
    entity_api: &Arc<Mutex<MockEntityApi>>,
    physics_api: &Arc<Mutex<MockPhysicsApi>>,
    audio_api: &Arc<Mutex<MockAudioApi>>,
    network_api: &Arc<Mutex<MockNetworkApi>>,
    storage_api: &Arc<Mutex<MockStorageApi>>,
    scheduler: &mut WorldScriptScheduler,
    metrics: &LuaMetrics,
    now: Instant,
) -> Vec<(LuaVm, String)> {
    let mut vms = Vec::new();

    for (id, (path, source)) in scripts.iter().enumerate() {
        let script_id = (id + 1) as u64;
        let file_name = std::path::Path::new(path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();

        let meta = script::parse_metadata(source);
        let hook_name = script::stage_to_hook_name(&meta.stage);

        println!("Loading [{script_id}] {file_name} (stage: {} → {hook_name})", meta.stage);

        let descriptor = ScriptDescriptor {
            id: script_id,
            name: file_name.to_string(),
            priority: 100,
            cpu_budget_per_tick: CPU_BUDGET,
            memory_bytes: MEMORY_LIMIT as u64,
            initial_entities: 0,
            runtime: ScriptRuntime::Lua,
        };
        scheduler.register_script(descriptor, now).unwrap();

        let vm = LuaVm::new(script_id, file_name, MEMORY_LIMIT).expect("failed to create VM");

        let ctx = ScriptContext {
            entity_api: entity_api.clone(),
            physics_api: physics_api.clone(),
            audio_api: audio_api.clone(),
            network_api: network_api.clone(),
            storage_api: storage_api.clone(),
            script_id,
        };
        {
            let lua = vm.lua_lock();
            bridge::register_all(&lua, &ctx).expect("failed to register API bridge");
        }

        if let Err(e) = vm.load_script(source) {
            eprintln!("  LOAD ERROR: {}", error::format_lua_error(&e));
            metrics.inc_errors();
            continue;
        }

        if let Err(e) = vm.call_hook("on_init") {
            eprintln!("  on_init ERROR: {}", error::format_lua_error(&e));
            metrics.inc_errors();
        }

        vms.push((vm, hook_name.to_string()));
    }

    vms
}

/// Discovers all .lua files in a directory and returns (path, source) pairs.
fn discover_scripts(dir: &str) -> Vec<(String, String)> {
    let mut scripts = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Warning: could not read script directory {dir}: {e}");
            return scripts;
        }
    };

    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "lua")
        })
        .map(|e| e.path())
        .collect();

    paths.sort();

    for path in paths {
        match std::fs::read_to_string(&path) {
            Ok(source) => scripts.push((path.display().to_string(), source)),
            Err(e) => eprintln!("Warning: could not read {}: {e}", path.display()),
        }
    }

    scripts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_scripts_finds_lua_files() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts");
        let scripts = discover_scripts(dir);
        assert!(
            scripts.len() >= 4,
            "should find at least 4 .lua scripts, found {}",
            scripts.len()
        );
        for (path, source) in &scripts {
            assert!(path.ends_with(".lua"), "should only find .lua files: {path}");
            assert!(!source.is_empty(), "script should not be empty: {path}");
        }
    }

    #[test]
    fn test_discover_scripts_nonexistent_dir() {
        let scripts = discover_scripts("/tmp/nonexistent_aether_scripts_dir");
        assert!(scripts.is_empty());
    }

    fn create_test_context() -> (ScriptContext, Arc<Mutex<MockEntityApi>>) {
        let entity_api = Arc::new(Mutex::new(MockEntityApi::new()));
        let ctx = ScriptContext {
            entity_api: entity_api.clone(),
            physics_api: Arc::new(Mutex::new(MockPhysicsApi::new())),
            audio_api: Arc::new(Mutex::new(MockAudioApi::new())),
            network_api: Arc::new(Mutex::new(MockNetworkApi::new())),
            storage_api: Arc::new(Mutex::new(MockStorageApi::new())),
            script_id: 1,
        };
        (ctx, entity_api)
    }

    #[test]
    fn test_hello_world_script_loads_and_runs() {
        let source = include_str!("../scripts/hello_world.lua");
        let vm = LuaVm::new(1, "hello_world.lua", MEMORY_LIMIT).unwrap();

        let (ctx, entity_api) = create_test_context();
        {
            let lua = vm.lua_lock();
            bridge::register_all(&lua, &ctx).unwrap();
        }

        vm.load_script(source).unwrap();
        vm.call_hook("on_init").unwrap();

        assert_eq!(entity_api.lock().unwrap().spawn_count(), 1);

        for tick in 1..=120 {
            vm.set_time(DT, tick).unwrap();
            vm.call_hook("on_tick").unwrap();
        }
    }

    #[test]
    fn test_npc_patrol_script_moves_entity() {
        let source = include_str!("../scripts/npc_patrol.lua");
        let vm = LuaVm::new(2, "npc_patrol.lua", MEMORY_LIMIT).unwrap();

        let (ctx, entity_api) = create_test_context();
        {
            let lua = vm.lua_lock();
            bridge::register_all(&lua, &ctx).unwrap();
        }

        vm.load_script(source).unwrap();
        vm.call_hook("on_init").unwrap();

        assert_eq!(entity_api.lock().unwrap().spawn_count(), 1);

        for tick in 1..=120 {
            vm.set_time(DT, tick).unwrap();
            vm.call_hook("on_tick").unwrap();
        }

        let api = entity_api.lock().unwrap();
        let pos = api.position(1);
        assert!(pos.x > 5.0, "NPC should have moved from start, pos.x = {}", pos.x);
    }

    #[test]
    fn test_npc_patrol_metadata_parsed() {
        let source = include_str!("../scripts/npc_patrol.lua");
        let meta = script::parse_metadata(source);
        assert_eq!(meta.stage, "PrePhysics");
        assert_eq!(meta.reads, vec!["Transform"]);
        assert_eq!(meta.writes, vec!["Velocity"]);
    }

    #[test]
    fn test_physics_playground_spawns_objects() {
        let source = include_str!("../scripts/physics_playground.lua");
        let vm = LuaVm::new(3, "physics_playground.lua", MEMORY_LIMIT).unwrap();

        let (ctx, entity_api) = create_test_context();
        {
            let lua = vm.lua_lock();
            bridge::register_all(&lua, &ctx).unwrap();
        }

        vm.load_script(source).unwrap();
        vm.call_hook("on_init").unwrap();

        assert_eq!(entity_api.lock().unwrap().spawn_count(), 1);

        for tick in 1..=180 {
            vm.set_time(DT, tick).unwrap();
            vm.call_hook("on_tick").unwrap();
        }

        assert!(
            entity_api.lock().unwrap().spawn_count() >= 2,
            "should have spawned objects over time"
        );
    }

    #[test]
    fn test_multiplayer_sync_uses_storage_and_network() {
        let source = include_str!("../scripts/multiplayer_sync.lua");
        let vm = LuaVm::new(4, "multiplayer_sync.lua", MEMORY_LIMIT).unwrap();

        let network_api = Arc::new(Mutex::new(MockNetworkApi::new()));
        let storage_api = Arc::new(Mutex::new(MockStorageApi::new()));

        let ctx = ScriptContext {
            entity_api: Arc::new(Mutex::new(MockEntityApi::new())),
            physics_api: Arc::new(Mutex::new(MockPhysicsApi::new())),
            audio_api: Arc::new(Mutex::new(MockAudioApi::new())),
            network_api: network_api.clone(),
            storage_api: storage_api.clone(),
            script_id: 4,
        };
        {
            let lua = vm.lua_lock();
            bridge::register_all(&lua, &ctx).unwrap();
        }

        vm.load_script(source).unwrap();
        vm.call_hook("on_init").unwrap();

        assert_eq!(network_api.lock().unwrap().emit_count(), 1);

        for tick in 1..=720 {
            vm.set_time(DT, tick).unwrap();
            vm.call_hook("on_network_sync").unwrap();
        }

        assert!(storage_api.lock().unwrap().write_count() > 0, "should have persisted score");
        assert!(network_api.lock().unwrap().emit_count() > 1, "should have emitted score updates");
        assert!(network_api.lock().unwrap().rpc_count() > 0, "should have sent leaderboard RPCs");
    }

    #[test]
    fn test_all_scripts_parse_without_errors() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts");
        let scripts = discover_scripts(dir);
        for (path, source) in &scripts {
            let vm = LuaVm::new(99, path, MEMORY_LIMIT).unwrap();
            let result = vm.load_script(source);
            assert!(result.is_ok(), "script {path} should parse: {:?}", result.err());
        }
    }

    #[test]
    fn test_cpu_budget_limits_runaway_script() {
        let vm = LuaVm::new(1, "evil.lua", MEMORY_LIMIT).unwrap();
        vm.load_script(
            r#"
            function on_tick()
                while true do end
            end
            "#,
        )
        .unwrap();

        let start = Instant::now();
        vm.set_instruction_hook(Duration::from_millis(5), start);
        let result = vm.call_hook("on_tick");
        assert!(result.is_err(), "infinite loop should be killed");
        assert!(start.elapsed() < Duration::from_secs(1), "should terminate quickly");
    }

    #[test]
    fn test_sandbox_prevents_os_access() {
        let vm = LuaVm::new(1, "bad.lua", MEMORY_LIMIT).unwrap();
        let result = vm.load_script(r#"os.execute("rm -rf /")"#);
        assert!(result.is_err(), "os access should be blocked");
    }
}
