//! View geometry returned by `xrLocateViews` (design doc §5.4, P2-B).

use crate::tracking::Pose3;

/// Asymmetric field of view in radians. All four values are non-negative;
/// signs encode direction relative to the view's forward axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fov {
    pub angle_left: f32,
    pub angle_right: f32,
    pub angle_up: f32,
    pub angle_down: f32,
}

impl Default for Fov {
    fn default() -> Self {
        // 90° symmetric default — fine as a placeholder; real backends always
        // overwrite this from `xrLocateViews`.
        Self {
            angle_left: -std::f32::consts::FRAC_PI_4,
            angle_right: std::f32::consts::FRAC_PI_4,
            angle_up: std::f32::consts::FRAC_PI_4,
            angle_down: -std::f32::consts::FRAC_PI_4,
        }
    }
}

/// One eye-view location: pose + FOV. Backends produce a Vec of these per
/// frame; the array length matches the active `ViewConfigType` (1 for Mono,
/// 2 for Stereo).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub pose: Pose3,
    pub fov: Fov,
}

impl Default for View {
    fn default() -> Self {
        Self {
            pose: Pose3::default(),
            fov: Fov::default(),
        }
    }
}
