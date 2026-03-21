//! Overlay quad positioning in VR space.
//!
//! Computes the model matrix for the debug overlay panel,
//! positioned as a billboard in front of the user.
//! Used by the GLES renderer when rendering on Quest hardware.

/// Distance from the user's head to the overlay panel in meters.
pub const OVERLAY_DISTANCE_M: f32 = 1.5;

/// Width of the overlay panel in world space meters.
pub const OVERLAY_QUAD_WIDTH_M: f32 = 0.8;

/// Height of the overlay panel in world space meters.
pub const OVERLAY_QUAD_HEIGHT_M: f32 = 0.4;

/// Vertical offset below eye level in meters.
pub const OVERLAY_VERTICAL_OFFSET_M: f32 = -0.2;

/// Compute the world-space position of the overlay panel center.
///
/// Places the panel at `OVERLAY_DISTANCE_M` in front of the head,
/// offset slightly below eye level.
pub fn overlay_position(head_position: [f32; 3], head_forward: [f32; 3]) -> [f32; 3] {
    [
        head_position[0] + head_forward[0] * OVERLAY_DISTANCE_M,
        head_position[1] + OVERLAY_VERTICAL_OFFSET_M,
        head_position[2] + head_forward[2] * OVERLAY_DISTANCE_M,
    ]
}

/// Extract forward direction from a quaternion [x, y, z, w].
/// Returns the -Z direction (OpenXR convention: forward is -Z).
pub fn quaternion_forward(q: [f32; 4]) -> [f32; 3] {
    let [x, y, z, w] = q;
    [
        2.0 * (x * z + w * y),
        2.0 * (y * z - w * x),
        -(1.0 - 2.0 * (x * x + y * y)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_position_straight_ahead() {
        let head = [0.0, 1.7, 0.0];
        let forward = [0.0, 0.0, -1.0];
        let pos = overlay_position(head, forward);
        assert!((pos[0] - 0.0).abs() < 0.001);
        assert!((pos[1] - (1.7 + OVERLAY_VERTICAL_OFFSET_M)).abs() < 0.001);
        assert!((pos[2] - (-OVERLAY_DISTANCE_M)).abs() < 0.001);
    }

    #[test]
    fn overlay_position_rotated() {
        let head = [0.0, 1.7, 0.0];
        let forward = [1.0, 0.0, 0.0]; // looking right
        let pos = overlay_position(head, forward);
        assert!((pos[0] - OVERLAY_DISTANCE_M).abs() < 0.001);
        assert!((pos[2] - 0.0).abs() < 0.001);
    }

    #[test]
    fn quaternion_forward_identity() {
        let fwd = quaternion_forward([0.0, 0.0, 0.0, 1.0]);
        // Identity quaternion: forward is -Z -> [0, 0, -1]
        assert!((fwd[0]).abs() < 0.001);
        assert!((fwd[1]).abs() < 0.001);
        assert!((fwd[2] - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn quaternion_forward_90_yaw() {
        // 90 degrees around Y: forward should be roughly [-1, 0, 0] or [1, 0, 0]
        let s = std::f32::consts::FRAC_PI_4.sin();
        let c = std::f32::consts::FRAC_PI_4.cos();
        let fwd = quaternion_forward([0.0, s, 0.0, c]);
        // Should be approximately [1, 0, 0] (looking right)
        assert!(fwd[0].abs() > 0.9);
        assert!(fwd[2].abs() < 0.1);
    }

    #[test]
    fn overlay_constants_reasonable() {
        assert!(OVERLAY_DISTANCE_M > 0.5);
        assert!(OVERLAY_DISTANCE_M < 5.0);
        assert!(OVERLAY_QUAD_WIDTH_M > 0.1);
        assert!(OVERLAY_QUAD_HEIGHT_M > 0.1);
    }
}
