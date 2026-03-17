//! Single-world demo entry point.
//!
//! Ties together the renderer, physics, input, networking, scripting, and
//! hot-reload subsystems through ECS components in a single winit event loop.

mod components;
mod engine;
mod systems;

use std::sync::Arc;
use std::time::Instant;

use aether_asset_pipeline::hot_reload::{HotReloadConfig, HotReloadWatcher};
use aether_ecs::Entity;
use aether_physics::{BodyType, ColliderShape, CollisionLayers, PhysicsWorld, WorldPhysicsConfig};
use aether_renderer::gpu::GpuRenderer;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use components::{CameraState, InputState, Renderable, Transform};
use engine::{EngineConfig, SceneSetup};
use systems::render;

/// FPS logging interval in seconds.
const FPS_LOG_INTERVAL_SECS: f32 = 2.0;
/// Camera movement speed in units per second.
const CAMERA_MOVE_SPEED: f32 = 5.0;
/// Camera rotation speed in radians per second.
const CAMERA_ROTATE_SPEED: f32 = 1.5;
/// Number of demo cubes to spawn.
const NUM_CUBES: usize = 3;
/// Cube spacing along the X axis.
const CUBE_SPACING: f32 = 3.0;
/// Cube initial Y position.
const CUBE_Y: f32 = 1.0;

/// Scene entity with its components stored inline (simple flat ECS).
struct SceneEntity {
    _entity: Entity,
    transform: Transform,
    renderable: Renderable,
}

/// Application state machine.
enum AppState {
    Initializing,
    Running {
        window: Arc<Window>,
        renderer: GpuRenderer,
        camera: CameraState,
        input: InputState,
        scene: SceneSetup,
        entities: Vec<SceneEntity>,
        physics: PhysicsWorld,
        _watcher: Option<HotReloadWatcher>,
        last_frame: Instant,
        frame_count: u64,
        fps_timer: Instant,
    },
}

struct App {
    state: AppState,
    config: EngineConfig,
    entity_allocator: aether_ecs::entity::EntityAllocator,
}

impl App {
    fn new() -> Self {
        Self {
            state: AppState::Initializing,
            config: EngineConfig::from_env(),
            entity_allocator: aether_ecs::entity::EntityAllocator::new(),
        }
    }

    /// Allocate a unique entity ID.
    fn alloc_entity(&mut self) -> Entity {
        self.entity_allocator.allocate()
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if matches!(self.state, AppState::Running { .. }) {
            return;
        }

        log::info!(
            "initializing single-world demo: width={}, height={}, offline={}",
            self.config.window_width,
            self.config.window_height,
            self.config.offline_mode
        );

        // Create window
        let window_attrs = Window::default_attributes()
            .with_title("Aether Single-World Demo (WASD=move, Arrows=look, ESC=quit)")
            .with_inner_size(PhysicalSize::new(
                self.config.window_width,
                self.config.window_height,
            ));

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };

        let size = window.inner_size();

        // Create wgpu surface + renderer (instance and surface must use the same instance)
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = match instance.create_surface(window.clone()) {
            Ok(s) => s,
            Err(e) => {
                log::error!("failed to create wgpu surface: {e}");
                event_loop.exit();
                return;
            }
        };

        let mut renderer = match pollster::block_on(GpuRenderer::new_with_surface(
            instance,
            surface,
            size.width,
            size.height,
        )) {
            Ok(r) => r,
            Err(e) => {
                log::error!("failed to create GPU renderer: {e}");
                event_loop.exit();
                return;
            }
        };
        log::info!("GPU renderer initialized");

        // Setup scene meshes and materials
        let scene = engine::setup_scene(&mut renderer);
        log::info!("scene meshes and materials uploaded");

        // Setup physics
        let physics_config = WorldPhysicsConfig::default();
        let mut physics = PhysicsWorld::new(&physics_config);
        log::info!("physics world initialized");

        let mut entities = Vec::new();
        let layers = CollisionLayers::default();

        // Floor entity (static)
        let floor_entity = self.alloc_entity();
        let floor_transform = Transform::at(0.0, 0.0, 0.0).with_scale(1.0, 1.0, 1.0);
        physics.add_rigid_body(
            floor_entity,
            BodyType::Static,
            floor_transform.position,
            floor_transform.rotation,
        );
        physics.add_collider(
            floor_entity,
            &ColliderShape::Box {
                half_extents: [20.0, 0.1, 20.0],
            },
            false,
            0.5,
            0.0,
            1.0,
            &layers,
        );
        entities.push(SceneEntity {
            _entity: floor_entity,
            transform: floor_transform,
            renderable: Renderable {
                mesh_id: scene.floor_mesh,
                material_id: scene.floor_material,
                model_index: 0,
            },
        });

        // Cube entities (dynamic)
        for i in 0..NUM_CUBES {
            let x = (i as f32 - (NUM_CUBES as f32 - 1.0) / 2.0) * CUBE_SPACING;
            let cube_entity = self.alloc_entity();
            let cube_transform = Transform::at(x, CUBE_Y, 0.0);
            physics.add_rigid_body(
                cube_entity,
                BodyType::Dynamic,
                cube_transform.position,
                cube_transform.rotation,
            );
            physics.add_collider(
                cube_entity,
                &ColliderShape::Box {
                    half_extents: [1.0, 1.0, 1.0],
                },
                false,
                0.5,
                0.3,
                1.0,
                &layers,
            );
            let model_index = entities.len();
            entities.push(SceneEntity {
                _entity: cube_entity,
                transform: cube_transform,
                renderable: Renderable {
                    mesh_id: scene.cube_mesh,
                    material_id: scene.cube_material,
                    model_index,
                },
            });
        }

        // Sphere entity (static decoration)
        let sphere_entity = self.alloc_entity();
        let sphere_transform = Transform::at(0.0, 3.0, -5.0);
        let sphere_model_index = entities.len();
        entities.push(SceneEntity {
            _entity: sphere_entity,
            transform: sphere_transform,
            renderable: Renderable {
                mesh_id: scene.sphere_mesh,
                material_id: scene.sphere_material,
                model_index: sphere_model_index,
            },
        });

        // Camera
        let camera = CameraState {
            aspect: size.width as f32 / size.height.max(1) as f32,
            ..CameraState::default()
        };

        // Hot-reload watcher
        let hot_reload_config = HotReloadConfig::from_env();
        let watcher = match HotReloadWatcher::start(hot_reload_config) {
            Ok(w) => {
                if w.is_some() {
                    log::info!("hot-reload watcher started");
                }
                w
            }
            Err(e) => {
                log::warn!("failed to start hot-reload watcher: {e}");
                None
            }
        };

        // Initial uniform updates
        let camera_uniforms = render::update_camera_uniforms(&camera);
        renderer.update_camera(&camera_uniforms);

        let light_uniforms = render::build_light_uniforms();
        renderer.update_light(&light_uniforms);

        for entity in &entities {
            let model_uniforms = render::build_model_uniforms(&entity.transform);
            renderer.update_model(entity.renderable.model_index, &model_uniforms);
        }

        let now = Instant::now();

        self.state = AppState::Running {
            window,
            renderer,
            camera,
            input: InputState::default(),
            scene,
            entities,
            physics,
            _watcher: watcher,
            last_frame: now,
            frame_count: 0,
            fps_timer: now,
        };

        log::info!("single-world demo ready");
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let AppState::Running {
            window,
            renderer,
            camera,
            input,
            entities,
            physics,
            _watcher,
            last_frame,
            frame_count,
            fps_timer,
            ..
        } = &mut self.state
        else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                log::info!("window close requested, shutting down");
                event_loop.exit();
            }

            WindowEvent::Resized(new_size) => {
                if new_size.width > 0 && new_size.height > 0 {
                    renderer.resize(new_size.width, new_size.height);
                    camera.aspect = new_size.width as f32 / new_size.height as f32;
                    log::info!(
                        "window resized: width={}, height={}",
                        new_size.width,
                        new_size.height
                    );
                }
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key_code),
                        state: key_state,
                        ..
                    },
                ..
            } => {
                if key_code == KeyCode::Escape && key_state == ElementState::Pressed {
                    event_loop.exit();
                    return;
                }
                let pressed = key_state == ElementState::Pressed;
                match key_code {
                    KeyCode::KeyW => input.forward = pressed,
                    KeyCode::KeyS => input.backward = pressed,
                    KeyCode::KeyA => input.left = pressed,
                    KeyCode::KeyD => input.right = pressed,
                    KeyCode::Space => input.up = pressed,
                    KeyCode::ShiftLeft | KeyCode::ShiftRight => input.down = pressed,
                    KeyCode::ArrowLeft => input.yaw_left = pressed,
                    KeyCode::ArrowRight => input.yaw_right = pressed,
                    KeyCode::ArrowUp => input.pitch_up = pressed,
                    KeyCode::ArrowDown => input.pitch_down = pressed,
                    _ => {}
                }
            }

            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(*last_frame).as_secs_f32();
                *last_frame = now;

                // Apply input to camera
                apply_input_to_camera(input, camera, dt);

                // Step physics
                physics.step();

                // Sync physics results to entity transforms
                let physics_results = physics.sync_to_ecs();
                for (phys_entity, phys_transform, _velocity) in &physics_results {
                    for scene_ent in entities.iter_mut() {
                        if scene_ent._entity == *phys_entity {
                            scene_ent.transform.position = phys_transform.position;
                            scene_ent.transform.rotation = phys_transform.rotation;
                        }
                    }
                }

                // Poll hot-reload events
                if let Some(watcher) = _watcher {
                    while let Some(event) = watcher.try_recv() {
                        log::info!(
                            "hot-reload event: path={}",
                            event.path.display()
                        );
                    }
                }

                // Update GPU uniforms
                let camera_uniforms = render::update_camera_uniforms(camera);
                renderer.update_camera(&camera_uniforms);

                for entity in entities.iter() {
                    let model_uniforms = render::build_model_uniforms(&entity.transform);
                    renderer.update_model(entity.renderable.model_index, &model_uniforms);
                }

                // Build draw commands
                let renderables: Vec<(Renderable, Transform)> = entities
                    .iter()
                    .map(|e| (e.renderable.clone(), e.transform.clone()))
                    .collect();
                let draw_commands = render::build_draw_commands(&renderables);

                if let Err(e) = renderer.render(&draw_commands) {
                    log::error!("render failed: {e}");
                }

                // FPS counter
                *frame_count += 1;
                let fps_elapsed = now.duration_since(*fps_timer).as_secs_f32();
                if fps_elapsed >= FPS_LOG_INTERVAL_SECS {
                    let fps = *frame_count as f32 / fps_elapsed;
                    log::info!("frame rate: {fps:.1} FPS");
                    *frame_count = 0;
                    *fps_timer = now;
                }

                window.request_redraw();
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let AppState::Running { window, .. } = &self.state {
            window.request_redraw();
        }
    }
}

/// Apply input state to camera position and orientation.
fn apply_input_to_camera(input: &InputState, camera: &mut CameraState, dt: f32) {
    let fwd = camera.forward_xz();
    let right = camera.right_xz();

    let mut dx = 0.0f32;
    let mut dy = 0.0f32;
    let mut dz = 0.0f32;

    if input.forward {
        dx += fwd[0] * CAMERA_MOVE_SPEED * dt;
        dz += fwd[2] * CAMERA_MOVE_SPEED * dt;
    }
    if input.backward {
        dx -= fwd[0] * CAMERA_MOVE_SPEED * dt;
        dz -= fwd[2] * CAMERA_MOVE_SPEED * dt;
    }
    if input.right {
        dx += right[0] * CAMERA_MOVE_SPEED * dt;
        dz += right[2] * CAMERA_MOVE_SPEED * dt;
    }
    if input.left {
        dx -= right[0] * CAMERA_MOVE_SPEED * dt;
        dz -= right[2] * CAMERA_MOVE_SPEED * dt;
    }
    if input.up {
        dy += CAMERA_MOVE_SPEED * dt;
    }
    if input.down {
        dy -= CAMERA_MOVE_SPEED * dt;
    }

    camera.position[0] += dx;
    camera.position[1] += dy;
    camera.position[2] += dz;

    if input.yaw_left {
        camera.yaw -= CAMERA_ROTATE_SPEED * dt;
    }
    if input.yaw_right {
        camera.yaw += CAMERA_ROTATE_SPEED * dt;
    }
    if input.pitch_up {
        camera.pitch += CAMERA_ROTATE_SPEED * dt;
    }
    if input.pitch_down {
        camera.pitch -= CAMERA_ROTATE_SPEED * dt;
    }

    // Clamp pitch to avoid gimbal lock
    let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
    camera.pitch = camera.pitch.clamp(-max_pitch, max_pitch);
}

fn main() {
    env_logger::init();

    log::info!("starting Aether single-world demo");

    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("event loop error");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_starts_in_initializing_state() {
        let app = App::new();
        assert!(matches!(app.state, AppState::Initializing));
    }

    #[test]
    fn engine_config_defaults_are_valid() {
        let config = EngineConfig::from_env();
        assert!(config.window_width > 0);
        assert!(config.window_height > 0);
    }

    #[test]
    fn apply_input_no_movement_when_idle() {
        let input = InputState::default();
        let mut camera = CameraState::default();
        let pos_before = camera.position;
        let yaw_before = camera.yaw;
        let pitch_before = camera.pitch;
        apply_input_to_camera(&input, &mut camera, 0.016);
        assert_eq!(camera.position, pos_before);
        assert_eq!(camera.yaw, yaw_before);
        assert_eq!(camera.pitch, pitch_before);
    }

    #[test]
    fn apply_input_forward_moves_camera() {
        let mut input = InputState::default();
        input.forward = true;
        let mut camera = CameraState {
            position: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera, 1.0);
        // With yaw=0, forward is -Z
        assert!(camera.position[2] < 0.0);
    }

    #[test]
    fn apply_input_backward_moves_camera() {
        let mut input = InputState::default();
        input.backward = true;
        let mut camera = CameraState {
            position: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera, 1.0);
        // With yaw=0, backward is +Z
        assert!(camera.position[2] > 0.0);
    }

    #[test]
    fn apply_input_up_down_moves_vertically() {
        let mut input = InputState::default();
        input.up = true;
        let mut camera = CameraState {
            position: [0.0, 0.0, 0.0],
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera, 1.0);
        assert!(camera.position[1] > 0.0);

        input.up = false;
        input.down = true;
        let mut camera2 = CameraState {
            position: [0.0, 0.0, 0.0],
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera2, 1.0);
        assert!(camera2.position[1] < 0.0);
    }

    #[test]
    fn apply_input_yaw_rotates_camera() {
        let mut input = InputState::default();
        input.yaw_right = true;
        let mut camera = CameraState::default();
        let yaw_before = camera.yaw;
        apply_input_to_camera(&input, &mut camera, 1.0);
        assert!(camera.yaw > yaw_before);
    }

    #[test]
    fn apply_input_pitch_clamped() {
        let mut input = InputState::default();
        input.pitch_up = true;
        let mut camera = CameraState::default();
        // Apply many frames to saturate pitch
        for _ in 0..100 {
            apply_input_to_camera(&input, &mut camera, 0.1);
        }
        let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
        assert!(camera.pitch <= max_pitch + 1e-6);
    }

    #[test]
    fn constants_are_reasonable() {
        assert!(CAMERA_MOVE_SPEED > 0.0);
        assert!(CAMERA_ROTATE_SPEED > 0.0);
        assert!(NUM_CUBES > 0);
        assert!(CUBE_SPACING > 0.0);
        assert!(FPS_LOG_INTERVAL_SECS > 0.0);
    }
}
