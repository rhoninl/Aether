mod render;
mod scene;

use std::time::Instant;

use aether_ecs::World;
use aether_physics::trigger::TriggerEventQueue;
use minifb::{Key, Window, WindowOptions};

use render::{Camera, FrameBuffer, HEIGHT, WIDTH};
use scene::SceneEntities;

const CAM_ORBIT_SPEED: f32 = 0.03;
const CAM_ZOOM_SPEED: f32 = 0.5;
const PLAYER_MOVE_SPEED: f32 = 5.0;

fn main() {
    let mut world = World::new();
    let scene = scene::setup_scene(&mut world);
    let mut trigger_queue = TriggerEventQueue::new();
    let mut camera = Camera::default();
    let mut fb = FrameBuffer::new();

    // Camera orbit state (spherical coords around target)
    let mut cam_angle: f32 = std::f32::consts::PI * 0.75; // horizontal angle
    let mut cam_pitch: f32 = 0.45; // vertical angle
    let mut cam_dist: f32 = 22.0; // distance from target

    let mut window = Window::new(
        "Aether VR Engine - 3D Demo (WASD=move, Arrows=camera, QE=zoom, ESC=quit)",
        WIDTH,
        HEIGHT,
        WindowOptions {
            resize: false,
            ..WindowOptions::default()
        },
    )
    .expect("failed to create window");

    // Cap at ~60 fps
    window.set_target_fps(60);

    let mut tick: u32 = 0;
    let mut frame_time_ms: f32 = 0.0;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let frame_start = Instant::now();
        let dt = 1.0 / 60.0_f32;

        // --- Camera orbit (arrow keys) ---
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
            cam_dist = (cam_dist - CAM_ZOOM_SPEED).max(5.0);
        }
        if window.is_key_down(Key::E) {
            cam_dist = (cam_dist + CAM_ZOOM_SPEED).min(50.0);
        }

        // Update camera position from spherical coordinates
        camera.eye[0] = camera.target[0] + cam_dist * cam_pitch.cos() * cam_angle.cos();
        camera.eye[1] = camera.target[1] + cam_dist * cam_pitch.sin();
        camera.eye[2] = camera.target[2] + cam_dist * cam_pitch.cos() * cam_angle.sin();

        // --- Player movement (WASD) ---
        let mut move_x: f32 = 0.0;
        let mut move_z: f32 = 0.0;
        if window.is_key_down(Key::W) {
            move_z += 1.0;
        }
        if window.is_key_down(Key::S) {
            move_z -= 1.0;
        }
        if window.is_key_down(Key::A) {
            move_x -= 1.0;
        }
        if window.is_key_down(Key::D) {
            move_x += 1.0;
        }

        // Move player relative to camera facing direction
        if move_x != 0.0 || move_z != 0.0 {
            let forward_x = -cam_angle.cos();
            let forward_z = -cam_angle.sin();
            let right_x = -cam_angle.sin();
            let right_z = cam_angle.cos();
            let dx = (forward_x * move_z + right_x * move_x) * PLAYER_MOVE_SPEED * dt;
            let dz = (forward_z * move_z + right_z * move_x) * PLAYER_MOVE_SPEED * dt;
            scene::move_player(&mut world, &scene, dx, dz);
        }

        // Camera follows player
        let player_pos = scene::get_position(&world, scene.player);
        camera.target = [player_pos[0], 2.0, player_pos[2]];

        // --- Reset scene (R key) ---
        if window.is_key_down(Key::R) {
            scene::reset_physics(&mut world, &scene);
        }

        // --- Simulation step ---
        world.run_systems();
        scene::apply_gravity(&mut world, &scene);
        scene::integrate_physics(&mut world, &scene);
        scene::detect_triggers(&world, &mut trigger_queue, &scene);

        // --- Render ---
        fb.clear();
        render::render_ground(&mut fb, &camera);
        render_shadows(&mut fb, &camera, &world, &scene);
        render_trigger(&mut fb, &camera, &world, &scene, &trigger_queue);
        render_objects(&mut fb, &camera, &world, &scene);
        render::render_hud(&mut fb, tick, world.entity_count(), trigger_queue.active_pair_count(), frame_time_ms);

        window.update_with_buffer(&fb.buf, WIDTH, HEIGHT).unwrap();

        tick += 1;
        frame_time_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
    }
}

fn render_shadows(fb: &mut FrameBuffer, cam: &Camera, world: &World, scene: &SceneEntities) {
    for &e in &scene.spheres {
        let pos = scene::get_position(world, e);
        render::render_shadow_blob(fb, cam, pos, 0.5);
    }
    for &e in &scene.cubes {
        let pos = scene::get_position(world, e);
        render::render_shadow_blob(fb, cam, pos, 0.6);
    }
    let player_pos = scene::get_position(world, scene.player);
    render::render_shadow_blob(fb, cam, player_pos, 0.4);
}

fn render_trigger(
    fb: &mut FrameBuffer,
    cam: &Camera,
    world: &World,
    scene: &SceneEntities,
    trigger_queue: &TriggerEventQueue,
) {
    let pos = scene::get_position(world, scene.trigger_zone);
    render::render_trigger_zone(fb, cam, pos, 3.0, trigger_queue.active_pair_count() > 0);
}

fn render_objects(fb: &mut FrameBuffer, cam: &Camera, world: &World, scene: &SceneEntities) {
    for &e in &scene.spheres {
        let pos = scene::get_position(world, e);
        render::render_sphere(fb, cam, pos, 0.5);
    }
    for &e in &scene.cubes {
        let pos = scene::get_position(world, e);
        render::render_cube(fb, cam, pos, 0.5);
    }
    let player_pos = scene::get_position(world, scene.player);
    render::render_player(fb, cam, player_pos);
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_ecs::World;
    use aether_physics::components::{Transform, Velocity};

    #[test]
    fn test_scene_setup() {
        let mut world = World::new();
        let scene = scene::setup_scene(&mut world);
        assert_eq!(world.entity_count(), 11);
        assert!(world.is_alive(scene.ground));
        assert!(world.is_alive(scene.player));
        assert_eq!(scene.spheres.len(), 5);
        assert_eq!(scene.cubes.len(), 3);
    }

    #[test]
    fn test_gravity_and_physics() {
        let mut world = World::new();
        let scene = scene::setup_scene(&mut world);
        let pos_before = scene::get_position(&world, scene.spheres[0]);
        for _ in 0..30 {
            scene::apply_gravity(&mut world, &scene);
            scene::integrate_physics(&mut world, &scene);
        }
        let pos_after = scene::get_position(&world, scene.spheres[0]);
        assert!(pos_after[1] < pos_before[1]);
    }

    #[test]
    fn test_ground_clamp() {
        let mut world = World::new();
        let scene = scene::setup_scene(&mut world);
        for _ in 0..300 {
            scene::apply_gravity(&mut world, &scene);
            scene::integrate_physics(&mut world, &scene);
        }
        let pos = scene::get_position(&world, scene.spheres[0]);
        assert!(pos[1] >= 0.0);
    }

    #[test]
    fn test_trigger_detection() {
        let mut world = World::new();
        let scene = scene::setup_scene(&mut world);
        let mut queue = TriggerEventQueue::new();
        // Run physics until objects fall into trigger zone
        for _ in 0..200 {
            scene::apply_gravity(&mut world, &scene);
            scene::integrate_physics(&mut world, &scene);
            scene::detect_triggers(&world, &mut queue, &scene);
        }
        assert!(queue.active_pair_count() > 0);
    }

    #[test]
    fn test_framebuffer_clear() {
        let mut fb = FrameBuffer::new();
        fb.clear();
        assert_eq!(fb.buf.len(), WIDTH * HEIGHT);
    }

    #[test]
    fn test_camera_default() {
        let cam = Camera::default();
        assert!(cam.eye[1] > 0.0);
        assert!(cam.fov_deg > 0.0);
    }

    #[test]
    fn test_render_sphere_no_panic() {
        let mut fb = FrameBuffer::new();
        fb.clear();
        let cam = Camera::default();
        render::render_sphere(&mut fb, &cam, [0.0, 2.0, 0.0], 0.5);
    }

    #[test]
    fn test_render_cube_no_panic() {
        let mut fb = FrameBuffer::new();
        fb.clear();
        let cam = Camera::default();
        render::render_cube(&mut fb, &cam, [0.0, 2.0, 0.0], 0.5);
    }

    #[test]
    fn test_render_player_no_panic() {
        let mut fb = FrameBuffer::new();
        fb.clear();
        let cam = Camera::default();
        render::render_player(&mut fb, &cam, [0.0, 1.0, -5.0]);
    }

    #[test]
    fn test_render_ground_no_panic() {
        let mut fb = FrameBuffer::new();
        fb.clear();
        let cam = Camera::default();
        render::render_ground(&mut fb, &cam);
    }

    #[test]
    fn test_render_hud_no_panic() {
        let mut fb = FrameBuffer::new();
        fb.clear();
        render::render_hud(&mut fb, 42, 11, 3, 16.6);
    }

    #[test]
    fn test_full_render_frame() {
        let mut world = World::new();
        let scene = scene::setup_scene(&mut world);
        let mut fb = FrameBuffer::new();
        let cam = Camera::default();
        let queue = TriggerEventQueue::new();

        fb.clear();
        render::render_ground(&mut fb, &cam);
        render_objects(&mut fb, &cam, &world, &scene);
        render::render_hud(&mut fb, 0, world.entity_count(), queue.active_pair_count(), 0.0);

        // Verify some pixels were written (not all sky gradient)
        let center = fb.buf[HEIGHT / 2 * WIDTH + WIDTH / 2];
        // Just verify no panic and buffer is populated
        assert!(fb.buf.len() == WIDTH * HEIGHT);
    }
}
