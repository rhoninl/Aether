//! Emulated VR display with stereo and mono rendering.
//!
//! Computes eye-specific view matrices and viewport rectangles for
//! rendering a VR scene preview in a desktop window.

use crate::config::{DisplayConfig, ViewMode};

/// Which eye to render for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Eye {
    Left,
    Right,
}

/// A rectangular viewport in pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// View parameters for one eye.
#[derive(Debug, Clone, Copy)]
pub struct EyeView {
    /// Eye position in world space.
    pub position: [f32; 3],
    /// View direction (forward vector).
    pub forward: [f32; 3],
    /// Up vector.
    pub up: [f32; 3],
    /// Right vector.
    pub right: [f32; 3],
    /// Horizontal FOV in radians.
    pub h_fov_rad: f32,
    /// Vertical FOV in radians.
    pub v_fov_rad: f32,
    /// Viewport rectangle in the output framebuffer.
    pub viewport: Viewport,
}

/// Stereo display manager that computes eye views from head pose and configuration.
#[derive(Debug, Clone)]
pub struct StereoDisplay {
    /// Half IPD in meters (each eye offset from center by this amount).
    half_ipd_m: f32,
    /// Horizontal FOV in radians.
    h_fov_rad: f32,
    /// Vertical FOV in radians.
    v_fov_rad: f32,
    /// View mode (stereo or mono).
    view_mode: ViewMode,
    /// Output window width.
    window_width: usize,
    /// Output window height.
    window_height: usize,
}

impl StereoDisplay {
    /// Create a new stereo display from configuration.
    pub fn new(display_config: &DisplayConfig, window_width: usize, window_height: usize) -> Self {
        Self {
            half_ipd_m: (display_config.ipd_mm / 1000.0) * 0.5,
            h_fov_rad: display_config.h_fov_deg.to_radians(),
            v_fov_rad: display_config.v_fov_deg.to_radians(),
            view_mode: display_config.view_mode,
            window_width,
            window_height,
        }
    }

    /// Get the view mode.
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    /// Get the half IPD in meters.
    pub fn half_ipd_m(&self) -> f32 {
        self.half_ipd_m
    }

    /// Get the horizontal FOV in degrees.
    pub fn h_fov_deg(&self) -> f32 {
        self.h_fov_rad.to_degrees()
    }

    /// Get the vertical FOV in degrees.
    pub fn v_fov_deg(&self) -> f32 {
        self.v_fov_rad.to_degrees()
    }

    /// Compute the viewport rectangle for a given eye.
    pub fn viewport(&self, eye: Eye) -> Viewport {
        match self.view_mode {
            ViewMode::Stereo => {
                let half_width = self.window_width / 2;
                match eye {
                    Eye::Left => Viewport {
                        x: 0,
                        y: 0,
                        width: half_width,
                        height: self.window_height,
                    },
                    Eye::Right => Viewport {
                        x: half_width,
                        y: 0,
                        width: self.window_width - half_width,
                        height: self.window_height,
                    },
                }
            }
            ViewMode::Mono => Viewport {
                x: 0,
                y: 0,
                width: self.window_width,
                height: self.window_height,
            },
        }
    }

    /// Compute the full eye view for rendering, given head position and rotation.
    pub fn eye_view(
        &self,
        eye: Eye,
        head_position: [f32; 3],
        head_yaw_rad: f32,
        head_pitch_rad: f32,
    ) -> EyeView {
        let (forward, up, right) = compute_basis_vectors(head_yaw_rad, head_pitch_rad);

        // Offset eye position by half IPD along the right vector
        let ipd_offset = match eye {
            Eye::Left => -self.half_ipd_m,
            Eye::Right => self.half_ipd_m,
        };

        let position = [
            head_position[0] + right[0] * ipd_offset,
            head_position[1] + right[1] * ipd_offset,
            head_position[2] + right[2] * ipd_offset,
        ];

        let viewport = self.viewport(eye);

        EyeView {
            position,
            forward,
            up,
            right,
            h_fov_rad: self.h_fov_rad,
            v_fov_rad: self.v_fov_rad,
            viewport,
        }
    }

    /// Compute the aspect ratio of a single eye viewport.
    pub fn eye_aspect_ratio(&self) -> f32 {
        let vp = self.viewport(Eye::Left);
        vp.width as f32 / vp.height as f32
    }

    /// Compute the perspective projection FOV scale (tangent of half vertical FOV).
    pub fn fov_scale(&self) -> f32 {
        (self.v_fov_rad * 0.5).tan()
    }

    /// Project a world-space point to a screen-space position for a given eye view.
    /// Returns (screen_x, screen_y, depth) or None if behind the near plane.
    pub fn project_point(
        &self,
        eye_view: &EyeView,
        world_point: [f32; 3],
    ) -> Option<(f32, f32, f32)> {
        let rel = [
            world_point[0] - eye_view.position[0],
            world_point[1] - eye_view.position[1],
            world_point[2] - eye_view.position[2],
        ];

        let z = dot(rel, eye_view.forward);
        if z < 0.1 {
            return None;
        }

        let x = dot(rel, eye_view.right);
        let y = dot(rel, eye_view.up);

        let aspect = eye_view.viewport.width as f32 / eye_view.viewport.height as f32;
        let fov_scale = (eye_view.v_fov_rad * 0.5).tan();

        let px = x / (z * fov_scale * aspect);
        let py = -y / (z * fov_scale);

        let sx = eye_view.viewport.x as f32 + (px * 0.5 + 0.5) * eye_view.viewport.width as f32;
        let sy = eye_view.viewport.y as f32 + (py * 0.5 + 0.5) * eye_view.viewport.height as f32;

        Some((sx, sy, z))
    }
}

/// Compute forward, up, right basis vectors from yaw and pitch.
pub fn compute_basis_vectors(yaw_rad: f32, pitch_rad: f32) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let cos_p = pitch_rad.cos();
    let sin_p = pitch_rad.sin();
    let cos_y = yaw_rad.cos();
    let sin_y = yaw_rad.sin();

    // Forward direction (where the head is looking)
    let forward = [-sin_y * cos_p, sin_p, -cos_y * cos_p];

    // World up
    let world_up = [0.0f32, 1.0, 0.0];

    // Right = forward x world_up (then normalize)
    let right = normalize(cross(forward, world_up));

    // Recompute up = right x forward
    let up = cross(right, forward);

    (forward, up, right)
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-8 {
        return [0.0, 0.0, 0.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{preset_display, HeadsetPreset};

    const EPSILON: f32 = 1e-4;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn default_display() -> StereoDisplay {
        let config = preset_display(HeadsetPreset::Quest2);
        StereoDisplay::new(&config, 1280, 720)
    }

    fn mono_display() -> StereoDisplay {
        let mut config = preset_display(HeadsetPreset::Quest2);
        config.view_mode = ViewMode::Mono;
        StereoDisplay::new(&config, 1280, 720)
    }

    // ---- Viewport tests ----

    #[test]
    fn stereo_left_viewport() {
        let display = default_display();
        let vp = display.viewport(Eye::Left);
        assert_eq!(vp.x, 0);
        assert_eq!(vp.y, 0);
        assert_eq!(vp.width, 640);
        assert_eq!(vp.height, 720);
    }

    #[test]
    fn stereo_right_viewport() {
        let display = default_display();
        let vp = display.viewport(Eye::Right);
        assert_eq!(vp.x, 640);
        assert_eq!(vp.y, 0);
        assert_eq!(vp.width, 640);
        assert_eq!(vp.height, 720);
    }

    #[test]
    fn stereo_viewports_cover_full_width() {
        let display = default_display();
        let left = display.viewport(Eye::Left);
        let right = display.viewport(Eye::Right);
        assert_eq!(left.width + right.width, 1280);
        assert_eq!(left.x + left.width, right.x);
    }

    #[test]
    fn mono_viewport_full_width() {
        let display = mono_display();
        let vp = display.viewport(Eye::Left);
        assert_eq!(vp.x, 0);
        assert_eq!(vp.width, 1280);
        assert_eq!(vp.height, 720);
    }

    #[test]
    fn mono_both_eyes_same_viewport() {
        let display = mono_display();
        let left = display.viewport(Eye::Left);
        let right = display.viewport(Eye::Right);
        assert_eq!(left, right);
    }

    // ---- Odd width handling ----

    #[test]
    fn stereo_odd_width_no_gap() {
        let config = preset_display(HeadsetPreset::Quest2);
        let display = StereoDisplay::new(&config, 1281, 720);
        let left = display.viewport(Eye::Left);
        let right = display.viewport(Eye::Right);
        // left.width = 640, right starts at 640, right.width = 641
        assert_eq!(left.x + left.width, right.x);
        assert_eq!(right.x + right.width, 1281);
    }

    // ---- Eye position offset ----

    #[test]
    fn left_eye_offset_is_negative_x() {
        let display = default_display();
        let view = display.eye_view(Eye::Left, [0.0, 1.7, 0.0], 0.0, 0.0);
        // At zero yaw, right vector is roughly [1, 0, 0]
        // Left eye should be offset in -X direction
        assert!(view.position[0] < 0.0, "left eye x={}", view.position[0]);
    }

    #[test]
    fn right_eye_offset_is_positive_x() {
        let display = default_display();
        let view = display.eye_view(Eye::Right, [0.0, 1.7, 0.0], 0.0, 0.0);
        assert!(view.position[0] > 0.0, "right eye x={}", view.position[0]);
    }

    #[test]
    fn eye_offset_equals_half_ipd() {
        let display = default_display();
        let left = display.eye_view(Eye::Left, [0.0, 1.7, 0.0], 0.0, 0.0);
        let right = display.eye_view(Eye::Right, [0.0, 1.7, 0.0], 0.0, 0.0);
        let dx = right.position[0] - left.position[0];
        let expected_ipd = 63.0 / 1000.0;
        assert!(approx_eq(dx, expected_ipd), "IPD distance = {dx}");
    }

    #[test]
    fn mono_both_eyes_same_position() {
        let display = mono_display();
        let left = display.eye_view(Eye::Left, [0.0, 1.7, 0.0], 0.0, 0.0);
        let right = display.eye_view(Eye::Right, [0.0, 1.7, 0.0], 0.0, 0.0);
        // In mono mode, both still compute separate positions, but they are symmetric
        // (The display doesn't suppress the offset - the app would just not render the right eye)
        assert!(approx_eq(left.position[0], -right.position[0]));
    }

    // ---- FOV ----

    #[test]
    fn fov_matches_config() {
        let display = default_display();
        assert!(approx_eq(display.h_fov_deg(), 97.0));
        assert!(approx_eq(display.v_fov_deg(), 93.0));
    }

    #[test]
    fn fov_scale_is_tan_half_vfov() {
        let display = default_display();
        let expected = (93.0f32.to_radians() * 0.5).tan();
        assert!(approx_eq(display.fov_scale(), expected));
    }

    #[test]
    fn eye_view_fov_matches_display() {
        let display = default_display();
        let view = display.eye_view(Eye::Left, [0.0, 1.7, 0.0], 0.0, 0.0);
        assert!(approx_eq(view.h_fov_rad, 97.0f32.to_radians()));
        assert!(approx_eq(view.v_fov_rad, 93.0f32.to_radians()));
    }

    // ---- Aspect ratio ----

    #[test]
    fn stereo_aspect_ratio() {
        let display = default_display();
        let aspect = display.eye_aspect_ratio();
        // Each eye: 640x720
        let expected = 640.0 / 720.0;
        assert!(approx_eq(aspect, expected), "aspect = {aspect}");
    }

    #[test]
    fn mono_aspect_ratio() {
        let display = mono_display();
        let aspect = display.eye_aspect_ratio();
        let expected = 1280.0 / 720.0;
        assert!(approx_eq(aspect, expected), "aspect = {aspect}");
    }

    // ---- Basis vectors ----

    #[test]
    fn basis_at_zero_rotation() {
        let (forward, up, right) = compute_basis_vectors(0.0, 0.0);
        // At zero rotation: forward = [0, 0, -1], up = [0, 1, 0], right = [1, 0, 0]
        assert!(approx_eq(forward[0], 0.0));
        assert!(approx_eq(forward[1], 0.0));
        assert!(approx_eq(forward[2], -1.0));

        assert!(approx_eq(up[0], 0.0));
        assert!(approx_eq(up[1], 1.0));
        assert!(approx_eq(up[2], 0.0));

        assert!(approx_eq(right[0], 1.0));
        assert!(approx_eq(right[1], 0.0));
        assert!(approx_eq(right[2], 0.0));
    }

    #[test]
    fn basis_vectors_orthogonal() {
        let (forward, up, right) = compute_basis_vectors(0.7, 0.3);
        assert!(approx_eq(dot(forward, up), 0.0));
        assert!(approx_eq(dot(forward, right), 0.0));
        assert!(approx_eq(dot(up, right), 0.0));
    }

    #[test]
    fn basis_forward_is_unit() {
        let (forward, _, _) = compute_basis_vectors(1.2, -0.4);
        let len = (forward[0].powi(2) + forward[1].powi(2) + forward[2].powi(2)).sqrt();
        assert!(approx_eq(len, 1.0), "forward len = {len}");
    }

    // ---- Projection ----

    #[test]
    fn project_point_in_front_returns_some() {
        let display = default_display();
        let view = display.eye_view(Eye::Left, [0.0, 1.7, 0.0], 0.0, 0.0);
        // Point directly in front of the eye
        let result = display.project_point(&view, [0.0, 1.7, -5.0]);
        assert!(result.is_some());
    }

    #[test]
    fn project_point_behind_returns_none() {
        let display = default_display();
        let view = display.eye_view(Eye::Left, [0.0, 1.7, 0.0], 0.0, 0.0);
        // Point behind the eye
        let result = display.project_point(&view, [0.0, 1.7, 5.0]);
        assert!(result.is_none());
    }

    #[test]
    fn project_center_point_in_viewport_center() {
        let display = default_display();
        let view = display.eye_view(Eye::Left, [0.0, 1.7, 0.0], 0.0, 0.0);
        // Point directly ahead at eye level
        let pos = [view.position[0], view.position[1], view.position[2] - 10.0];
        if let Some((sx, sy, _)) = display.project_point(&view, pos) {
            // Should be roughly in the center of the left viewport
            let vp = view.viewport;
            let cx = vp.x as f32 + vp.width as f32 * 0.5;
            let cy = vp.y as f32 + vp.height as f32 * 0.5;
            assert!((sx - cx).abs() < 10.0, "sx={sx}, expected ~{cx}");
            assert!((sy - cy).abs() < 10.0, "sy={sy}, expected ~{cy}");
        }
    }

    #[test]
    fn project_depth_increases_with_distance() {
        let display = default_display();
        let view = display.eye_view(Eye::Left, [0.0, 1.7, 0.0], 0.0, 0.0);
        let near = display.project_point(&view, [0.0, 1.7, -2.0]).unwrap().2;
        let far = display.project_point(&view, [0.0, 1.7, -20.0]).unwrap().2;
        assert!(far > near, "near_z={near}, far_z={far}");
    }

    // ---- View mode ----

    #[test]
    fn view_mode_returns_config_mode() {
        assert_eq!(default_display().view_mode(), ViewMode::Stereo);
        assert_eq!(mono_display().view_mode(), ViewMode::Mono);
    }

    // ---- Half IPD ----

    #[test]
    fn half_ipd_is_half_of_full() {
        let display = default_display();
        let expected = 63.0 / 1000.0 / 2.0;
        assert!(approx_eq(display.half_ipd_m(), expected));
    }
}
