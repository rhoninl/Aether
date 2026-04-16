mod camera;
mod scene;

use std::env;
use std::sync::Arc;
use std::time::Instant;

use aether_renderer::gpu::GpuRenderer;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use camera::Camera;
use scene::Scene;

/// Default window width.
const DEFAULT_WINDOW_WIDTH: u32 = 1280;
/// Default window height.
const DEFAULT_WINDOW_HEIGHT: u32 = 720;
/// Target frames per second.
#[cfg(test)]
const TARGET_FPS: f32 = 60.0;
/// Target frame time in seconds.
#[cfg(test)]
const TARGET_FRAME_TIME: f32 = 1.0 / TARGET_FPS;

/// Read window width from env var.
fn window_width_from_env() -> u32 {
    env::var("AETHER_WINDOW_WIDTH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_WINDOW_WIDTH)
}

/// Read window height from env var.
fn window_height_from_env() -> u32 {
    env::var("AETHER_WINDOW_HEIGHT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_WINDOW_HEIGHT)
}

/// Tracks which movement keys are currently pressed.
#[derive(Default)]
struct InputState {
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    turn_left: bool,
    turn_right: bool,
    look_up: bool,
    look_down: bool,
}

impl InputState {
    fn handle_key(&mut self, key_code: KeyCode, pressed: bool) {
        match key_code {
            KeyCode::KeyW => self.forward = pressed,
            KeyCode::KeyS => self.backward = pressed,
            KeyCode::KeyA => self.left = pressed,
            KeyCode::KeyD => self.right = pressed,
            KeyCode::Space => self.up = pressed,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => self.down = pressed,
            KeyCode::ArrowLeft => self.turn_left = pressed,
            KeyCode::ArrowRight => self.turn_right = pressed,
            KeyCode::ArrowUp => self.look_up = pressed,
            KeyCode::ArrowDown => self.look_down = pressed,
            _ => {}
        }
    }

    fn apply_to_camera(&self, camera: &mut Camera, dt: f32) {
        let mut forward_amount = 0.0f32;
        let mut right_amount = 0.0f32;
        let mut up_amount = 0.0f32;

        if self.forward {
            forward_amount += camera.move_speed * dt;
        }
        if self.backward {
            forward_amount -= camera.move_speed * dt;
        }
        if self.right {
            right_amount += camera.move_speed * dt;
        }
        if self.left {
            right_amount -= camera.move_speed * dt;
        }
        if self.up {
            up_amount += camera.move_speed * dt;
        }
        if self.down {
            up_amount -= camera.move_speed * dt;
        }

        if forward_amount != 0.0 || right_amount != 0.0 || up_amount != 0.0 {
            camera.translate(forward_amount, right_amount, up_amount);
        }

        let mut yaw_delta = 0.0f32;
        let mut pitch_delta = 0.0f32;

        if self.turn_left {
            yaw_delta -= camera.rotate_speed * dt;
        }
        if self.turn_right {
            yaw_delta += camera.rotate_speed * dt;
        }
        if self.look_up {
            pitch_delta += camera.rotate_speed * dt;
        }
        if self.look_down {
            pitch_delta -= camera.rotate_speed * dt;
        }

        if yaw_delta != 0.0 || pitch_delta != 0.0 {
            camera.rotate(yaw_delta, pitch_delta);
        }
    }
}

/// Application state machine for the winit event loop.
enum AppState {
    /// Waiting for the event loop to resume and create the window.
    Uninitialized,
    /// Running with a window, renderer, and scene.
    Running {
        window: Arc<Window>,
        renderer: Box<GpuRenderer>,
        scene: Box<Scene>,
        camera: Camera,
        input: InputState,
        last_frame: Instant,
        frame_count: u64,
        fps_timer: Instant,
    },
    /// An error occurred during initialization.
    #[allow(dead_code)]
    Failed(String),
}

struct App {
    state: AppState,
}

impl App {
    fn new() -> Self {
        Self {
            state: AppState::Uninitialized,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if matches!(self.state, AppState::Running { .. }) {
            return;
        }

        let width = window_width_from_env();
        let height = window_height_from_env();

        let window_attrs = Window::default_attributes()
            .with_title("Aether GPU Demo (WASD=move, Arrows=look, Space/Shift=up/down, ESC=quit)")
            .with_inner_size(PhysicalSize::new(width, height));

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("failed to create window: {e}");
                self.state = AppState::Failed(format!("window creation failed: {e}"));
                event_loop.exit();
                return;
            }
        };

        let size = window.inner_size();

        // Create wgpu surface
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = match instance.create_surface(window.clone()) {
            Ok(s) => s,
            Err(e) => {
                log::error!("failed to create surface: {e}");
                self.state = AppState::Failed(format!("surface creation failed: {e}"));
                event_loop.exit();
                return;
            }
        };

        // Create GPU renderer (pass same instance that created the surface)
        let mut renderer = match pollster::block_on(GpuRenderer::new_with_surface(
            instance,
            surface,
            size.width,
            size.height,
        )) {
            Ok(r) => r,
            Err(e) => {
                log::error!("failed to create GPU renderer: {e}");
                self.state = AppState::Failed(format!("renderer creation failed: {e}"));
                event_loop.exit();
                return;
            }
        };

        // Setup scene
        let scene = scene::setup_scene(&mut renderer);

        // Setup camera
        let mut camera = Camera::new([0.0, 5.0, 12.0]);
        camera.set_aspect_ratio(size.width, size.height);

        // Initial uniform updates
        renderer.update_camera(&camera.to_uniforms());
        renderer.update_light(&scene.light);
        scene::update_transforms(&renderer, &scene);

        let now = Instant::now();

        self.state = AppState::Running {
            window,
            renderer: Box::new(renderer),
            scene: Box::new(scene),
            camera,
            input: InputState::default(),
            last_frame: now,
            frame_count: 0,
            fps_timer: now,
        };

        log::info!(
            "GPU demo initialized: width={}, height={}",
            size.width,
            size.height
        );
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
            scene,
            camera,
            input,
            last_frame,
            frame_count,
            fps_timer,
        } = &mut self.state
        else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::Resized(new_size) => {
                if new_size.width > 0 && new_size.height > 0 {
                    renderer.resize(new_size.width, new_size.height);
                    camera.set_aspect_ratio(new_size.width, new_size.height);
                    log::debug!(
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
                input.handle_key(key_code, key_state == ElementState::Pressed);
            }

            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(*last_frame).as_secs_f32();
                *last_frame = now;

                // Update camera from input
                input.apply_to_camera(camera, dt);

                // Update GPU uniforms
                renderer.update_camera(&camera.to_uniforms());
                scene::update_transforms(renderer, scene);

                // Build draw commands and render
                let commands = scene::build_draw_commands(scene);
                if let Err(e) = renderer.render(&commands) {
                    log::error!("render failed: {e}");
                }

                // FPS counter
                *frame_count += 1;
                let fps_elapsed = now.duration_since(*fps_timer).as_secs_f32();
                if fps_elapsed >= 2.0 {
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

fn main() {
    env_logger::init();

    log::info!("starting Aether GPU rendering demo");

    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("event loop error");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_window_dimensions() {
        assert_eq!(DEFAULT_WINDOW_WIDTH, 1280);
        assert_eq!(DEFAULT_WINDOW_HEIGHT, 720);
    }

    #[test]
    fn target_fps_is_reasonable() {
        assert!(TARGET_FPS > 0.0);
        assert!(TARGET_FRAME_TIME > 0.0);
        assert!((TARGET_FRAME_TIME - 1.0 / TARGET_FPS).abs() < 1e-6);
    }

    #[test]
    fn input_state_default_all_false() {
        let input = InputState::default();
        assert!(!input.forward);
        assert!(!input.backward);
        assert!(!input.left);
        assert!(!input.right);
        assert!(!input.up);
        assert!(!input.down);
        assert!(!input.turn_left);
        assert!(!input.turn_right);
        assert!(!input.look_up);
        assert!(!input.look_down);
    }

    #[test]
    fn input_state_handle_wasd() {
        let mut input = InputState::default();
        input.handle_key(KeyCode::KeyW, true);
        assert!(input.forward);
        input.handle_key(KeyCode::KeyW, false);
        assert!(!input.forward);

        input.handle_key(KeyCode::KeyA, true);
        assert!(input.left);
        input.handle_key(KeyCode::KeyS, true);
        assert!(input.backward);
        input.handle_key(KeyCode::KeyD, true);
        assert!(input.right);
    }

    #[test]
    fn input_state_handle_arrows() {
        let mut input = InputState::default();
        input.handle_key(KeyCode::ArrowLeft, true);
        assert!(input.turn_left);
        input.handle_key(KeyCode::ArrowRight, true);
        assert!(input.turn_right);
        input.handle_key(KeyCode::ArrowUp, true);
        assert!(input.look_up);
        input.handle_key(KeyCode::ArrowDown, true);
        assert!(input.look_down);
    }

    #[test]
    fn input_state_handle_vertical() {
        let mut input = InputState::default();
        input.handle_key(KeyCode::Space, true);
        assert!(input.up);
        input.handle_key(KeyCode::ShiftLeft, true);
        assert!(input.down);
    }

    #[test]
    fn input_apply_no_movement_when_idle() {
        let input = InputState::default();
        let mut camera = Camera::new([0.0, 0.0, 0.0]);
        let pos_before = camera.position;
        input.apply_to_camera(&mut camera, 0.016);
        assert_eq!(camera.position, pos_before);
    }

    #[test]
    fn input_apply_forward_movement() {
        let mut input = InputState::default();
        input.forward = true;
        let mut camera = Camera::new([0.0, 0.0, 0.0]);
        input.apply_to_camera(&mut camera, 1.0);
        // Camera should have moved in the forward direction
        let dist = (camera.position[0].powi(2) + camera.position[2].powi(2)).sqrt();
        assert!(dist > 0.0);
    }

    #[test]
    fn input_apply_rotation() {
        let mut input = InputState::default();
        input.turn_right = true;
        let mut camera = Camera::new([0.0, 0.0, 0.0]);
        let yaw_before = camera.yaw;
        input.apply_to_camera(&mut camera, 1.0);
        assert!(camera.yaw != yaw_before);
    }

    #[test]
    fn window_dimensions_from_env_defaults() {
        // When env vars aren't set, should return defaults
        let w = window_width_from_env();
        let h = window_height_from_env();
        assert!(w > 0);
        assert!(h > 0);
    }
}
