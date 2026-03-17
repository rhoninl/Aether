//! Input system: reads keyboard state and updates the local player's
//! camera/transform.

use crate::components::{CameraState, InputState};

/// Movement speed in units per second.
pub const MOVE_SPEED: f32 = 5.0;
/// Rotation speed in radians per second.
pub const ROTATION_SPEED: f32 = 2.0;

/// Apply WASD movement and arrow key rotation to the camera state.
///
/// Movement is relative to the camera's current facing direction.
/// Up/Down keys move along the world Y axis.
pub fn apply_input_to_camera(input: &InputState, camera: &mut CameraState, dt: f32) {
    // Rotation
    if input.yaw_left {
        camera.yaw -= ROTATION_SPEED * dt;
    }
    if input.yaw_right {
        camera.yaw += ROTATION_SPEED * dt;
    }
    if input.pitch_up {
        camera.pitch += ROTATION_SPEED * dt;
    }
    if input.pitch_down {
        camera.pitch -= ROTATION_SPEED * dt;
    }

    // Clamp pitch to avoid gimbal lock
    let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
    camera.pitch = camera.pitch.clamp(-max_pitch, max_pitch);

    // Movement
    let forward = camera_forward(camera);
    let right = camera_right(camera);
    let speed = MOVE_SPEED * dt;

    if input.forward {
        camera.position[0] += forward[0] * speed;
        camera.position[1] += forward[1] * speed;
        camera.position[2] += forward[2] * speed;
    }
    if input.backward {
        camera.position[0] -= forward[0] * speed;
        camera.position[1] -= forward[1] * speed;
        camera.position[2] -= forward[2] * speed;
    }
    if input.right {
        camera.position[0] += right[0] * speed;
        camera.position[1] += right[1] * speed;
        camera.position[2] += right[2] * speed;
    }
    if input.left {
        camera.position[0] -= right[0] * speed;
        camera.position[1] -= right[1] * speed;
        camera.position[2] -= right[2] * speed;
    }
    if input.up {
        camera.position[1] += speed;
    }
    if input.down {
        camera.position[1] -= speed;
    }
}

/// Compute the forward direction vector from the camera's yaw and pitch.
///
/// Returns a unit vector in the direction the camera is looking.
pub fn camera_forward(camera: &CameraState) -> [f32; 3] {
    let (sy, cy) = camera.yaw.sin_cos();
    let (sp, cp) = camera.pitch.sin_cos();
    [cy * cp, sp, sy * cp]
}

/// Compute the right direction vector from the camera's yaw.
///
/// Returns a unit vector pointing to the camera's right side (horizontal plane).
pub fn camera_right(camera: &CameraState) -> [f32; 3] {
    let (sy, cy) = camera.yaw.sin_cos();
    [-sy, 0.0, cy]
}

/// Convert the current camera state into a network avatar state for
/// multiplayer synchronization.
///
/// Uses the camera position as the head position and derives head rotation
/// from yaw/pitch. Hands are placed at default offsets relative to the head.
pub fn camera_to_avatar_state(camera: &CameraState) -> aether_multiplayer::AvatarState {
    let head_rotation = yaw_pitch_to_quaternion(camera.yaw, camera.pitch);

    let right = camera_right(camera);
    let hand_offset_down = 0.7;
    let hand_offset_forward = 0.3;
    let hand_offset_side = 0.3;

    let forward = camera_forward(camera);

    let left_hand_position = [
        camera.position[0] - right[0] * hand_offset_side + forward[0] * hand_offset_forward,
        camera.position[1] - hand_offset_down,
        camera.position[2] - right[2] * hand_offset_side + forward[2] * hand_offset_forward,
    ];

    let right_hand_position = [
        camera.position[0] + right[0] * hand_offset_side + forward[0] * hand_offset_forward,
        camera.position[1] - hand_offset_down,
        camera.position[2] + right[2] * hand_offset_side + forward[2] * hand_offset_forward,
    ];

    aether_multiplayer::AvatarState {
        head_position: camera.position,
        head_rotation,
        left_hand_position,
        left_hand_rotation: head_rotation,
        right_hand_position,
        right_hand_rotation: head_rotation,
    }
}

/// Convert yaw and pitch angles to a quaternion [x, y, z, w].
fn yaw_pitch_to_quaternion(yaw: f32, pitch: f32) -> [f32; 4] {
    // Compose yaw (around Y) then pitch (around X)
    let (sy, cy) = (yaw / 2.0).sin_cos();
    let (sp, cp) = (pitch / 2.0).sin_cos();

    // q = q_yaw * q_pitch
    // q_yaw   = (0, sy, 0, cy)
    // q_pitch = (sp, 0, 0, cp)
    let w = cy * cp;
    let x = cy * sp;
    let y = sy * cp;
    let z = -sy * sp;

    [x, y, z, w]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_speed_is_positive() {
        assert!(MOVE_SPEED > 0.0);
    }

    #[test]
    fn rotation_speed_is_positive() {
        assert!(ROTATION_SPEED > 0.0);
    }

    #[test]
    fn camera_forward_at_zero_yaw_pitch() {
        let camera = CameraState {
            yaw: 0.0,
            pitch: 0.0,
            ..CameraState::default()
        };
        let fwd = camera_forward(&camera);
        // At yaw=0, pitch=0: forward should be along +X (cos(0)*cos(0), sin(0), sin(0)*cos(0))
        assert!((fwd[0] - 1.0).abs() < 1e-6);
        assert!((fwd[1]).abs() < 1e-6);
        assert!((fwd[2]).abs() < 1e-6);
    }

    #[test]
    fn camera_forward_at_90_yaw() {
        let camera = CameraState {
            yaw: std::f32::consts::FRAC_PI_2,
            pitch: 0.0,
            ..CameraState::default()
        };
        let fwd = camera_forward(&camera);
        // At yaw=pi/2: forward should be along +Z
        assert!((fwd[0]).abs() < 1e-6);
        assert!((fwd[1]).abs() < 1e-6);
        assert!((fwd[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn camera_forward_at_pitch_up() {
        let camera = CameraState {
            yaw: 0.0,
            pitch: std::f32::consts::FRAC_PI_4,
            ..CameraState::default()
        };
        let fwd = camera_forward(&camera);
        // Pitch up: Y component should be positive
        assert!(fwd[1] > 0.0);
    }

    #[test]
    fn camera_forward_is_unit_length() {
        let camera = CameraState {
            yaw: 0.5,
            pitch: 0.3,
            ..CameraState::default()
        };
        let fwd = camera_forward(&camera);
        let len = (fwd[0] * fwd[0] + fwd[1] * fwd[1] + fwd[2] * fwd[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-6);
    }

    #[test]
    fn camera_right_at_zero_yaw() {
        let camera = CameraState {
            yaw: 0.0,
            ..CameraState::default()
        };
        let right = camera_right(&camera);
        // At yaw=0: right should be along +Z ([-sin(0), 0, cos(0)] = [0, 0, 1])
        assert!((right[0]).abs() < 1e-6);
        assert!((right[1]).abs() < 1e-6);
        assert!((right[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn camera_right_at_90_yaw() {
        let camera = CameraState {
            yaw: std::f32::consts::FRAC_PI_2,
            ..CameraState::default()
        };
        let right = camera_right(&camera);
        // At yaw=pi/2: right should be along -X
        assert!((right[0] - (-1.0)).abs() < 1e-6);
        assert!((right[1]).abs() < 1e-6);
        assert!((right[2]).abs() < 1e-6);
    }

    #[test]
    fn camera_right_is_horizontal() {
        let camera = CameraState {
            yaw: 1.23,
            pitch: 0.45,
            ..CameraState::default()
        };
        let right = camera_right(&camera);
        assert!((right[1]).abs() < 1e-6);
    }

    #[test]
    fn camera_right_is_unit_length() {
        let camera = CameraState {
            yaw: 0.7,
            ..CameraState::default()
        };
        let right = camera_right(&camera);
        let len = (right[0] * right[0] + right[1] * right[1] + right[2] * right[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-6);
    }

    #[test]
    fn forward_and_right_are_perpendicular() {
        let camera = CameraState {
            yaw: 0.8,
            pitch: 0.0,
            ..CameraState::default()
        };
        let fwd = camera_forward(&camera);
        let right = camera_right(&camera);
        let dot = fwd[0] * right[0] + fwd[1] * right[1] + fwd[2] * right[2];
        assert!(dot.abs() < 1e-6);
    }

    #[test]
    fn apply_input_no_input_no_change() {
        let input = InputState::default();
        let mut camera = CameraState::default();
        let original = camera.clone();
        apply_input_to_camera(&input, &mut camera, 1.0 / 60.0);
        assert_eq!(camera.position, original.position);
        assert!((camera.yaw - original.yaw).abs() < 1e-6);
        assert!((camera.pitch - original.pitch).abs() < 1e-6);
    }

    #[test]
    fn apply_input_forward_moves_position() {
        let input = InputState {
            forward: true,
            ..InputState::default()
        };
        let mut camera = CameraState {
            position: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera, 1.0);

        // At yaw=0, pitch=0, forward is +X
        assert!(camera.position[0] > 0.0);
    }

    #[test]
    fn apply_input_backward_moves_opposite() {
        let input = InputState {
            backward: true,
            ..InputState::default()
        };
        let mut camera = CameraState {
            position: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera, 1.0);

        assert!(camera.position[0] < 0.0);
    }

    #[test]
    fn apply_input_left_moves_left() {
        let input = InputState {
            left: true,
            ..InputState::default()
        };
        let mut camera = CameraState {
            position: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera, 1.0);

        // Right is +Z at yaw=0, so left is -Z
        assert!(camera.position[2] < 0.0);
    }

    #[test]
    fn apply_input_right_moves_right() {
        let input = InputState {
            right: true,
            ..InputState::default()
        };
        let mut camera = CameraState {
            position: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera, 1.0);

        // Right is +Z at yaw=0
        assert!(camera.position[2] > 0.0);
    }

    #[test]
    fn apply_input_up_moves_up() {
        let input = InputState {
            up: true,
            ..InputState::default()
        };
        let mut camera = CameraState {
            position: [0.0, 0.0, 0.0],
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera, 1.0);
        assert!(camera.position[1] > 0.0);
    }

    #[test]
    fn apply_input_down_moves_down() {
        let input = InputState {
            down: true,
            ..InputState::default()
        };
        let mut camera = CameraState {
            position: [0.0, 5.0, 0.0],
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera, 1.0);
        assert!(camera.position[1] < 5.0);
    }

    #[test]
    fn apply_input_yaw_left_decreases_yaw() {
        let input = InputState {
            yaw_left: true,
            ..InputState::default()
        };
        let mut camera = CameraState {
            yaw: 0.0,
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera, 1.0);
        assert!(camera.yaw < 0.0);
    }

    #[test]
    fn apply_input_yaw_right_increases_yaw() {
        let input = InputState {
            yaw_right: true,
            ..InputState::default()
        };
        let mut camera = CameraState {
            yaw: 0.0,
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera, 1.0);
        assert!(camera.yaw > 0.0);
    }

    #[test]
    fn apply_input_pitch_up_increases_pitch() {
        let input = InputState {
            pitch_up: true,
            ..InputState::default()
        };
        let mut camera = CameraState {
            pitch: 0.0,
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera, 1.0);
        assert!(camera.pitch > 0.0);
    }

    #[test]
    fn apply_input_pitch_down_decreases_pitch() {
        let input = InputState {
            pitch_down: true,
            ..InputState::default()
        };
        let mut camera = CameraState {
            pitch: 0.0,
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera, 1.0);
        assert!(camera.pitch < 0.0);
    }

    #[test]
    fn apply_input_pitch_clamped_up() {
        let input = InputState {
            pitch_up: true,
            ..InputState::default()
        };
        let mut camera = CameraState {
            pitch: 0.0,
            ..CameraState::default()
        };
        // Apply many times to exceed pi/2
        for _ in 0..100 {
            apply_input_to_camera(&input, &mut camera, 1.0);
        }
        let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
        assert!(camera.pitch <= max_pitch + 1e-6);
    }

    #[test]
    fn apply_input_pitch_clamped_down() {
        let input = InputState {
            pitch_down: true,
            ..InputState::default()
        };
        let mut camera = CameraState {
            pitch: 0.0,
            ..CameraState::default()
        };
        for _ in 0..100 {
            apply_input_to_camera(&input, &mut camera, 1.0);
        }
        let min_pitch = -(std::f32::consts::FRAC_PI_2 - 0.01);
        assert!(camera.pitch >= min_pitch - 1e-6);
    }

    #[test]
    fn apply_input_movement_scales_with_dt() {
        let input = InputState {
            forward: true,
            ..InputState::default()
        };

        let mut camera1 = CameraState {
            position: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera1, 1.0);

        let mut camera2 = CameraState {
            position: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            ..CameraState::default()
        };
        apply_input_to_camera(&input, &mut camera2, 0.5);

        // Half dt should produce half the movement
        assert!((camera2.position[0] - camera1.position[0] / 2.0).abs() < 1e-6);
    }

    #[test]
    fn camera_to_avatar_state_head_position() {
        let camera = CameraState {
            position: [1.0, 2.0, 3.0],
            ..CameraState::default()
        };
        let avatar = camera_to_avatar_state(&camera);
        assert_eq!(avatar.head_position, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn camera_to_avatar_state_identity_rotation() {
        let camera = CameraState {
            yaw: 0.0,
            pitch: 0.0,
            ..CameraState::default()
        };
        let avatar = camera_to_avatar_state(&camera);
        // At zero yaw/pitch, quaternion should be identity
        let len = avatar
            .head_rotation
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();
        assert!((len - 1.0).abs() < 1e-6);
        assert!((avatar.head_rotation[3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn camera_to_avatar_state_hands_below_head() {
        let camera = CameraState {
            position: [0.0, 2.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            ..CameraState::default()
        };
        let avatar = camera_to_avatar_state(&camera);
        assert!(avatar.left_hand_position[1] < camera.position[1]);
        assert!(avatar.right_hand_position[1] < camera.position[1]);
    }

    #[test]
    fn camera_to_avatar_state_hands_symmetric() {
        let camera = CameraState {
            position: [0.0, 2.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            ..CameraState::default()
        };
        let avatar = camera_to_avatar_state(&camera);
        // Left and right should be symmetric about the center axis
        assert!((avatar.left_hand_position[1] - avatar.right_hand_position[1]).abs() < 1e-6);
    }

    #[test]
    fn camera_to_avatar_state_rotation_is_unit_quaternion() {
        let camera = CameraState {
            yaw: 1.2,
            pitch: 0.3,
            ..CameraState::default()
        };
        let avatar = camera_to_avatar_state(&camera);
        let len = avatar
            .head_rotation
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();
        assert!((len - 1.0).abs() < 1e-6);
    }

    #[test]
    fn yaw_pitch_to_quaternion_identity() {
        let q = yaw_pitch_to_quaternion(0.0, 0.0);
        assert!((q[0]).abs() < 1e-6); // x
        assert!((q[1]).abs() < 1e-6); // y
        assert!((q[2]).abs() < 1e-6); // z
        assert!((q[3] - 1.0).abs() < 1e-6); // w
    }

    #[test]
    fn yaw_pitch_to_quaternion_is_unit() {
        let q = yaw_pitch_to_quaternion(0.7, 0.3);
        let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
        assert!((len - 1.0).abs() < 1e-6);
    }

    #[test]
    fn yaw_pitch_to_quaternion_pure_yaw() {
        let q = yaw_pitch_to_quaternion(std::f32::consts::FRAC_PI_2, 0.0);
        // Pure yaw rotation around Y: (0, sin(pi/4), 0, cos(pi/4))
        let expected_y = (std::f32::consts::FRAC_PI_4).sin();
        let expected_w = (std::f32::consts::FRAC_PI_4).cos();
        assert!((q[0]).abs() < 1e-6);
        assert!((q[1] - expected_y).abs() < 1e-6);
        assert!((q[2]).abs() < 1e-6);
        assert!((q[3] - expected_w).abs() < 1e-6);
    }
}
