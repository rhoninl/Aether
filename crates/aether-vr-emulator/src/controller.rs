//! Emulated VR controller input from keyboard and mouse.
//!
//! Maps desktop input to two virtual VR controllers with 6-DOF poses,
//! button states, and analog inputs, producing `ControllerState` output
//! compatible with the `aether-input` tracking system.

use aether_input::openxr_tracking::{
    ControllerAnalog, ControllerButtons, ControllerState, Hand, TrackingConfidence,
};
use aether_input::Pose3;

use crate::config::InputSensitivity;
use crate::head_tracking::yaw_pitch_to_quaternion;

/// Default left controller offset from head [x, y, z] in meters.
const LEFT_HAND_OFFSET: [f32; 3] = [-0.2, -0.3, -0.4];

/// Default right controller offset from head [x, y, z] in meters.
const RIGHT_HAND_OFFSET: [f32; 3] = [0.2, -0.3, -0.4];

/// Maximum thumbstick axis value.
const THUMBSTICK_MAX: f32 = 1.0;

/// Emulated controller input state (raw key/mouse states for one frame).
#[derive(Debug, Clone, Default)]
pub struct ControllerInput {
    // Left thumbstick (WASD)
    pub left_stick_up: bool,
    pub left_stick_down: bool,
    pub left_stick_left: bool,
    pub left_stick_right: bool,

    // Left hand rotation (Q/E)
    pub left_rotate_left: bool,
    pub left_rotate_right: bool,

    // Right hand aim (mouse position in normalized [-1, 1] screen coords)
    pub right_aim_x: f32,
    pub right_aim_y: f32,

    // Triggers and grips
    pub left_trigger: bool,
    pub right_trigger: bool,
    pub left_grip: bool,
    pub right_grip: bool,

    // Face buttons (number keys)
    pub button_a: bool,
    pub button_b: bool,
    pub button_x: bool,
    pub button_y: bool,
    pub button_menu: bool,

    // Thumbstick clicks
    pub left_thumbstick_click: bool,
    pub right_thumbstick_click: bool,
}

/// Emulated VR controllers manager.
#[derive(Debug, Clone)]
pub struct EmulatedControllers {
    left_yaw_offset: f32,
    controller_rotation_speed: f32,
}

impl EmulatedControllers {
    /// Create a new controller emulator with the given sensitivity settings.
    pub fn new(sensitivity: &InputSensitivity) -> Self {
        Self {
            left_yaw_offset: 0.0,
            controller_rotation_speed: sensitivity.controller_rotation_speed,
        }
    }

    /// Update controller state from input and head pose.
    /// Returns (left_controller, right_controller).
    pub fn update(
        &mut self,
        input: &ControllerInput,
        head_position: [f32; 3],
        head_yaw_rad: f32,
        dt_s: f32,
    ) -> (ControllerState, ControllerState) {
        // Update left controller rotation offset
        if input.left_rotate_left {
            self.left_yaw_offset -= self.controller_rotation_speed.to_radians() * dt_s;
        }
        if input.left_rotate_right {
            self.left_yaw_offset += self.controller_rotation_speed.to_radians() * dt_s;
        }

        let left = self.build_left_controller(input, head_position, head_yaw_rad);
        let right = self.build_right_controller(input, head_position, head_yaw_rad);

        (left, right)
    }

    /// Build left controller state from input.
    fn build_left_controller(
        &self,
        input: &ControllerInput,
        head_position: [f32; 3],
        head_yaw_rad: f32,
    ) -> ControllerState {
        let position = compute_controller_position(
            head_position,
            head_yaw_rad,
            LEFT_HAND_OFFSET,
        );

        let controller_yaw = head_yaw_rad + self.left_yaw_offset;
        let rotation = yaw_pitch_to_quaternion(controller_yaw, 0.0);

        let grip_pose = Pose3 {
            position,
            rotation,
            linear_velocity: [0.0, 0.0, 0.0],
            angular_velocity: [0.0, 0.0, 0.0],
        };
        let aim_pose = grip_pose;

        let analog = compute_left_thumbstick(input);

        let buttons = ControllerButtons {
            trigger_click: input.left_trigger,
            grip_click: input.left_grip,
            primary_button: input.button_x,
            secondary_button: input.button_y,
            thumbstick_click: input.left_thumbstick_click,
            thumbstick_touch: input.left_stick_up
                || input.left_stick_down
                || input.left_stick_left
                || input.left_stick_right,
            menu_button: input.button_menu,
        };

        ControllerState {
            hand: Hand::Left,
            tracking_confidence: TrackingConfidence::High,
            connected: true,
            grip_pose,
            aim_pose,
            analog,
            buttons,
        }
    }

    /// Build right controller state from input.
    fn build_right_controller(
        &self,
        input: &ControllerInput,
        head_position: [f32; 3],
        head_yaw_rad: f32,
    ) -> ControllerState {
        let position = compute_controller_position(
            head_position,
            head_yaw_rad,
            RIGHT_HAND_OFFSET,
        );

        // Right controller aims where mouse points
        let aim_pitch = -input.right_aim_y * std::f32::consts::FRAC_PI_4;
        let aim_yaw = head_yaw_rad - input.right_aim_x * std::f32::consts::FRAC_PI_4;
        let rotation = yaw_pitch_to_quaternion(aim_yaw, aim_pitch);

        let grip_pose = Pose3 {
            position,
            rotation,
            linear_velocity: [0.0, 0.0, 0.0],
            angular_velocity: [0.0, 0.0, 0.0],
        };
        let aim_pose = grip_pose;

        let buttons = ControllerButtons {
            trigger_click: input.right_trigger,
            grip_click: input.right_grip,
            primary_button: input.button_a,
            secondary_button: input.button_b,
            thumbstick_click: input.right_thumbstick_click,
            thumbstick_touch: false,
            menu_button: false,
        };

        ControllerState {
            hand: Hand::Right,
            tracking_confidence: TrackingConfidence::High,
            connected: true,
            grip_pose,
            aim_pose,
            analog: ControllerAnalog::default(),
            buttons,
        }
    }

    /// Get the current left controller yaw offset.
    pub fn left_yaw_offset(&self) -> f32 {
        self.left_yaw_offset
    }

    /// Reset the left controller rotation offset to zero.
    pub fn reset_rotation(&mut self) {
        self.left_yaw_offset = 0.0;
    }
}

/// Compute a controller position relative to head, rotated by head yaw.
pub fn compute_controller_position(
    head_position: [f32; 3],
    head_yaw_rad: f32,
    offset: [f32; 3],
) -> [f32; 3] {
    let cos_y = head_yaw_rad.cos();
    let sin_y = head_yaw_rad.sin();

    // Rotate offset by head yaw (around Y axis)
    let rotated_x = offset[0] * cos_y + offset[2] * sin_y;
    let rotated_z = -offset[0] * sin_y + offset[2] * cos_y;

    [
        head_position[0] + rotated_x,
        head_position[1] + offset[1],
        head_position[2] + rotated_z,
    ]
}

/// Compute left thumbstick analog values from WASD keys.
pub fn compute_left_thumbstick(input: &ControllerInput) -> ControllerAnalog {
    let mut x = 0.0f32;
    let mut y = 0.0f32;

    if input.left_stick_up {
        y += THUMBSTICK_MAX;
    }
    if input.left_stick_down {
        y -= THUMBSTICK_MAX;
    }
    if input.left_stick_left {
        x -= THUMBSTICK_MAX;
    }
    if input.left_stick_right {
        x += THUMBSTICK_MAX;
    }

    // Normalize diagonal to unit circle
    let mag = (x * x + y * y).sqrt();
    if mag > THUMBSTICK_MAX {
        let inv = THUMBSTICK_MAX / mag;
        x *= inv;
        y *= inv;
    }

    ControllerAnalog {
        trigger: if input.left_trigger { 1.0 } else { 0.0 },
        grip: if input.left_grip { 1.0 } else { 0.0 },
        thumbstick_x: x,
        thumbstick_y: y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-4;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn default_controllers() -> EmulatedControllers {
        EmulatedControllers::new(&InputSensitivity::default())
    }

    fn empty_input() -> ControllerInput {
        ControllerInput::default()
    }

    // ---- Controller position ----

    #[test]
    fn left_controller_offset_from_head() {
        let pos = compute_controller_position(
            [0.0, 1.7, 0.0],
            0.0, // yaw = 0 (facing -Z)
            LEFT_HAND_OFFSET,
        );
        assert!(approx_eq(pos[0], -0.2), "x={}", pos[0]);
        assert!(approx_eq(pos[1], 1.4), "y={}", pos[1]);
        assert!(approx_eq(pos[2], -0.4), "z={}", pos[2]);
    }

    #[test]
    fn right_controller_offset_from_head() {
        let pos = compute_controller_position(
            [0.0, 1.7, 0.0],
            0.0,
            RIGHT_HAND_OFFSET,
        );
        assert!(approx_eq(pos[0], 0.2), "x={}", pos[0]);
        assert!(approx_eq(pos[1], 1.4), "y={}", pos[1]);
        assert!(approx_eq(pos[2], -0.4), "z={}", pos[2]);
    }

    #[test]
    fn controller_position_follows_head() {
        let pos1 = compute_controller_position([0.0, 1.7, 0.0], 0.0, LEFT_HAND_OFFSET);
        let pos2 = compute_controller_position([5.0, 1.7, 3.0], 0.0, LEFT_HAND_OFFSET);
        // Controller moves with head
        assert!(approx_eq(pos2[0] - pos1[0], 5.0));
        assert!(approx_eq(pos2[2] - pos1[2], 3.0));
    }

    #[test]
    fn controller_position_rotates_with_head_yaw() {
        // At 90 degree yaw, left controller (normally at -0.2, -, -0.4) should be rotated
        let pos = compute_controller_position(
            [0.0, 1.7, 0.0],
            std::f32::consts::FRAC_PI_2,
            LEFT_HAND_OFFSET,
        );
        // After 90 degree rotation: x' = x*cos(90) + z*sin(90) = 0 + (-0.4)*1 = -0.4
        //                           z' = -x*sin(90) + z*cos(90) = 0.2 + 0 = 0.2
        assert!(approx_eq(pos[0], -0.4), "x={}", pos[0]);
        assert!(approx_eq(pos[2], 0.2), "z={}", pos[2]);
    }

    // ---- Thumbstick ----

    #[test]
    fn thumbstick_no_input_is_zero() {
        let input = empty_input();
        let analog = compute_left_thumbstick(&input);
        assert_eq!(analog.thumbstick_x, 0.0);
        assert_eq!(analog.thumbstick_y, 0.0);
    }

    #[test]
    fn thumbstick_forward_is_positive_y() {
        let mut input = empty_input();
        input.left_stick_up = true;
        let analog = compute_left_thumbstick(&input);
        assert_eq!(analog.thumbstick_y, 1.0);
        assert_eq!(analog.thumbstick_x, 0.0);
    }

    #[test]
    fn thumbstick_backward_is_negative_y() {
        let mut input = empty_input();
        input.left_stick_down = true;
        let analog = compute_left_thumbstick(&input);
        assert_eq!(analog.thumbstick_y, -1.0);
    }

    #[test]
    fn thumbstick_left_is_negative_x() {
        let mut input = empty_input();
        input.left_stick_left = true;
        let analog = compute_left_thumbstick(&input);
        assert_eq!(analog.thumbstick_x, -1.0);
    }

    #[test]
    fn thumbstick_right_is_positive_x() {
        let mut input = empty_input();
        input.left_stick_right = true;
        let analog = compute_left_thumbstick(&input);
        assert_eq!(analog.thumbstick_x, 1.0);
    }

    #[test]
    fn thumbstick_diagonal_is_normalized() {
        let mut input = empty_input();
        input.left_stick_up = true;
        input.left_stick_right = true;
        let analog = compute_left_thumbstick(&input);
        let mag = (analog.thumbstick_x.powi(2) + analog.thumbstick_y.powi(2)).sqrt();
        assert!(approx_eq(mag, 1.0), "diagonal mag = {mag}");
    }

    #[test]
    fn thumbstick_opposing_cancels() {
        let mut input = empty_input();
        input.left_stick_up = true;
        input.left_stick_down = true;
        let analog = compute_left_thumbstick(&input);
        assert_eq!(analog.thumbstick_y, 0.0);
    }

    #[test]
    fn trigger_analog_maps_to_one() {
        let mut input = empty_input();
        input.left_trigger = true;
        let analog = compute_left_thumbstick(&input);
        assert_eq!(analog.trigger, 1.0);
    }

    #[test]
    fn trigger_analog_maps_to_zero_when_released() {
        let input = empty_input();
        let analog = compute_left_thumbstick(&input);
        assert_eq!(analog.trigger, 0.0);
    }

    #[test]
    fn grip_analog_maps_to_one() {
        let mut input = empty_input();
        input.left_grip = true;
        let analog = compute_left_thumbstick(&input);
        assert_eq!(analog.grip, 1.0);
    }

    // ---- EmulatedControllers update ----

    #[test]
    fn update_returns_both_controllers() {
        let mut controllers = default_controllers();
        let input = empty_input();
        let (left, right) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        assert_eq!(left.hand, Hand::Left);
        assert_eq!(right.hand, Hand::Right);
    }

    #[test]
    fn both_controllers_connected() {
        let mut controllers = default_controllers();
        let input = empty_input();
        let (left, right) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        assert!(left.connected);
        assert!(right.connected);
    }

    #[test]
    fn both_controllers_high_confidence() {
        let mut controllers = default_controllers();
        let input = empty_input();
        let (left, right) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        assert_eq!(left.tracking_confidence, TrackingConfidence::High);
        assert_eq!(right.tracking_confidence, TrackingConfidence::High);
    }

    #[test]
    fn left_rotate_changes_yaw_offset() {
        let mut controllers = default_controllers();
        let mut input = empty_input();
        input.left_rotate_left = true;
        controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 1.0);
        assert!(controllers.left_yaw_offset() < 0.0);
    }

    #[test]
    fn right_rotate_changes_yaw_offset() {
        let mut controllers = default_controllers();
        let mut input = empty_input();
        input.left_rotate_right = true;
        controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 1.0);
        assert!(controllers.left_yaw_offset() > 0.0);
    }

    #[test]
    fn reset_rotation_clears_offset() {
        let mut controllers = default_controllers();
        let mut input = empty_input();
        input.left_rotate_left = true;
        controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 1.0);
        controllers.reset_rotation();
        assert_eq!(controllers.left_yaw_offset(), 0.0);
    }

    // ---- Button mapping ----

    #[test]
    fn left_trigger_button() {
        let mut controllers = default_controllers();
        let mut input = empty_input();
        input.left_trigger = true;
        let (left, _) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        assert!(left.buttons.trigger_click);
        assert_eq!(left.analog.trigger, 1.0);
    }

    #[test]
    fn right_trigger_button() {
        let mut controllers = default_controllers();
        let mut input = empty_input();
        input.right_trigger = true;
        let (_, right) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        assert!(right.buttons.trigger_click);
    }

    #[test]
    fn left_grip_button() {
        let mut controllers = default_controllers();
        let mut input = empty_input();
        input.left_grip = true;
        let (left, _) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        assert!(left.buttons.grip_click);
    }

    #[test]
    fn right_grip_button() {
        let mut controllers = default_controllers();
        let mut input = empty_input();
        input.right_grip = true;
        let (_, right) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        assert!(right.buttons.grip_click);
    }

    #[test]
    fn button_a_maps_to_right_primary() {
        let mut controllers = default_controllers();
        let mut input = empty_input();
        input.button_a = true;
        let (_, right) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        assert!(right.buttons.primary_button);
    }

    #[test]
    fn button_b_maps_to_right_secondary() {
        let mut controllers = default_controllers();
        let mut input = empty_input();
        input.button_b = true;
        let (_, right) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        assert!(right.buttons.secondary_button);
    }

    #[test]
    fn button_x_maps_to_left_primary() {
        let mut controllers = default_controllers();
        let mut input = empty_input();
        input.button_x = true;
        let (left, _) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        assert!(left.buttons.primary_button);
    }

    #[test]
    fn button_y_maps_to_left_secondary() {
        let mut controllers = default_controllers();
        let mut input = empty_input();
        input.button_y = true;
        let (left, _) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        assert!(left.buttons.secondary_button);
    }

    #[test]
    fn button_menu_maps_to_left_menu() {
        let mut controllers = default_controllers();
        let mut input = empty_input();
        input.button_menu = true;
        let (left, _) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        assert!(left.buttons.menu_button);
    }

    #[test]
    fn thumbstick_touch_active_when_stick_used() {
        let mut controllers = default_controllers();
        let mut input = empty_input();
        input.left_stick_up = true;
        let (left, _) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        assert!(left.buttons.thumbstick_touch);
    }

    #[test]
    fn thumbstick_touch_inactive_when_not_used() {
        let mut controllers = default_controllers();
        let input = empty_input();
        let (left, _) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        assert!(!left.buttons.thumbstick_touch);
    }

    // ---- Right controller aim ----

    #[test]
    fn right_aim_center_matches_head_yaw() {
        let mut controllers = default_controllers();
        let mut input = empty_input();
        input.right_aim_x = 0.0;
        input.right_aim_y = 0.0;
        let (_, right) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        // At zero aim offset and zero head yaw, rotation should be near identity
        assert!(approx_eq(right.grip_pose.rotation[3], 1.0));
    }

    #[test]
    fn right_aim_offset_changes_rotation() {
        let mut controllers = default_controllers();
        let mut input = empty_input();
        input.right_aim_x = 1.0; // Full right
        let (_, right) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        // Rotation should differ from identity when aiming to the side
        assert!(right.grip_pose.rotation[1].abs() > 0.01);
    }

    // ---- No input produces default states ----

    #[test]
    fn no_input_buttons_all_false() {
        let mut controllers = default_controllers();
        let input = empty_input();
        let (left, right) = controllers.update(&input, [0.0, 1.7, 0.0], 0.0, 0.016);
        assert!(!left.buttons.trigger_click);
        assert!(!left.buttons.grip_click);
        assert!(!left.buttons.primary_button);
        assert!(!left.buttons.secondary_button);
        assert!(!left.buttons.menu_button);
        assert!(!right.buttons.trigger_click);
        assert!(!right.buttons.grip_click);
        assert!(!right.buttons.primary_button);
        assert!(!right.buttons.secondary_button);
    }
}
