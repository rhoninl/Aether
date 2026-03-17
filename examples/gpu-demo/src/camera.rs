use std::env;

use aether_renderer::gpu::pass::CameraUniforms;

/// Default camera field of view in degrees.
const DEFAULT_FOV_DEGREES: f32 = 60.0;
/// Near clipping plane distance.
const NEAR_PLANE: f32 = 0.1;
/// Far clipping plane distance.
const FAR_PLANE: f32 = 500.0;
/// Maximum pitch angle in radians (just under 90 degrees).
const MAX_PITCH: f32 = 1.5;
/// Minimum pitch angle in radians.
const MIN_PITCH: f32 = -1.5;
/// Default camera movement speed (units per second).
const DEFAULT_MOVE_SPEED: f32 = 5.0;
/// Default camera rotation speed (radians per second).
const DEFAULT_ROTATE_SPEED: f32 = 2.0;

/// Read camera FOV from `AETHER_CAMERA_FOV` env var, defaulting to 60 degrees.
fn fov_from_env() -> f32 {
    env::var("AETHER_CAMERA_FOV")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(DEFAULT_FOV_DEGREES)
}

/// FPS-style camera with position, yaw, and pitch.
pub struct Camera {
    pub position: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub fov_degrees: f32,
    pub aspect_ratio: f32,
    pub move_speed: f32,
    pub rotate_speed: f32,
}

impl Camera {
    /// Create a new camera at the given position.
    pub fn new(position: [f32; 3]) -> Self {
        Self {
            position,
            yaw: 0.0,
            pitch: 0.0,
            fov_degrees: fov_from_env(),
            aspect_ratio: 16.0 / 9.0,
            move_speed: DEFAULT_MOVE_SPEED,
            rotate_speed: DEFAULT_ROTATE_SPEED,
        }
    }

    /// Update aspect ratio (call on window resize).
    pub fn set_aspect_ratio(&mut self, width: u32, height: u32) {
        if height > 0 {
            self.aspect_ratio = width as f32 / height as f32;
        }
    }

    /// Compute the forward direction vector (normalized, in XZ plane for movement).
    pub fn forward(&self) -> [f32; 3] {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        [sin_yaw, 0.0, -cos_yaw]
    }

    /// Compute the right direction vector (normalized).
    pub fn right(&self) -> [f32; 3] {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        [cos_yaw, 0.0, sin_yaw]
    }

    /// Compute the look direction (includes pitch).
    pub fn look_direction(&self) -> [f32; 3] {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        [
            sin_yaw * cos_pitch,
            sin_pitch,
            -cos_yaw * cos_pitch,
        ]
    }

    /// Move the camera forward/backward and left/right.
    /// `forward_amount` is positive for forward, negative for backward.
    /// `right_amount` is positive for right, negative for left.
    pub fn translate(&mut self, forward_amount: f32, right_amount: f32, up_amount: f32) {
        let fwd = self.forward();
        let right = self.right();

        self.position[0] += fwd[0] * forward_amount + right[0] * right_amount;
        self.position[1] += up_amount;
        self.position[2] += fwd[2] * forward_amount + right[2] * right_amount;
    }

    /// Rotate the camera by yaw and pitch deltas (in radians).
    pub fn rotate(&mut self, yaw_delta: f32, pitch_delta: f32) {
        self.yaw += yaw_delta;
        self.pitch = (self.pitch + pitch_delta).clamp(MIN_PITCH, MAX_PITCH);
    }

    /// Build the view matrix (column-major 4x4).
    pub fn view_matrix(&self) -> [[f32; 4]; 4] {
        let look = self.look_direction();
        let target = [
            self.position[0] + look[0],
            self.position[1] + look[1],
            self.position[2] + look[2],
        ];
        look_at(self.position, target, [0.0, 1.0, 0.0])
    }

    /// Build the projection matrix (column-major 4x4).
    pub fn projection_matrix(&self) -> [[f32; 4]; 4] {
        perspective(
            self.fov_degrees.to_radians(),
            self.aspect_ratio,
            NEAR_PLANE,
            FAR_PLANE,
        )
    }

    /// Build the CameraUniforms struct for the GPU.
    pub fn to_uniforms(&self) -> CameraUniforms {
        CameraUniforms {
            view: self.view_matrix(),
            projection: self.projection_matrix(),
            view_position: [self.position[0], self.position[1], self.position[2], 1.0],
        }
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new([0.0, 5.0, 10.0])
    }
}

/// Compute a look-at view matrix (column-major).
pub fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let f = normalize([
        target[0] - eye[0],
        target[1] - eye[1],
        target[2] - eye[2],
    ]);
    let s = normalize(cross(f, up));
    let u = cross(s, f);

    [
        [s[0], u[0], -f[0], 0.0],
        [s[1], u[1], -f[1], 0.0],
        [s[2], u[2], -f[2], 0.0],
        [-dot(s, eye), -dot(u, eye), dot(f, eye), 1.0],
    ]
}

/// Compute a perspective projection matrix (column-major, Vulkan/wgpu clip space: depth 0..1).
pub fn perspective(fov_rad: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (fov_rad / 2.0).tan();
    let range_inv = 1.0 / (near - far);

    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far * range_inv, -1.0],
        [0.0, 0.0, near * far * range_inv, 0.0],
    ]
}

/// Build a translation matrix.
pub fn translation_matrix(x: f32, y: f32, z: f32) -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [x, y, z, 1.0],
    ]
}

/// Build a uniform scale matrix.
pub fn scale_matrix(sx: f32, sy: f32, sz: f32) -> [[f32; 4]; 4] {
    [
        [sx, 0.0, 0.0, 0.0],
        [0.0, sy, 0.0, 0.0],
        [0.0, 0.0, sz, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// Multiply two 4x4 column-major matrices.
pub fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0f32; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            result[col][row] = a[0][row] * b[col][0]
                + a[1][row] * b[col][1]
                + a[2][row] * b[col][2]
                + a[3][row] * b[col][3];
        }
    }
    result
}

/// Compute the inverse-transpose of the upper-left 3x3, stored in a 4x4.
/// Used for the normal matrix.
pub fn normal_matrix(model: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    // For uniform/non-uniform scale + translation, the normal matrix is
    // the inverse-transpose of the upper-left 3x3.
    let a = model[0][0];
    let b = model[1][0];
    let c = model[2][0];
    let d = model[0][1];
    let e = model[1][1];
    let f = model[2][1];
    let g = model[0][2];
    let h = model[1][2];
    let i = model[2][2];

    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);

    if det.abs() < 1e-10 {
        return identity();
    }

    let inv_det = 1.0 / det;

    // Inverse of 3x3, then transpose => cofactor matrix / det
    [
        [
            (e * i - f * h) * inv_det,
            (c * h - b * i) * inv_det,
            (b * f - c * e) * inv_det,
            0.0,
        ],
        [
            (f * g - d * i) * inv_det,
            (a * i - c * g) * inv_det,
            (c * d - a * f) * inv_det,
            0.0,
        ],
        [
            (d * h - e * g) * inv_det,
            (b * g - a * h) * inv_det,
            (a * e - b * d) * inv_det,
            0.0,
        ],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// Identity 4x4 matrix.
pub fn identity() -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-10 {
        return [0.0, 0.0, 0.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    // ---- Vector math tests ----

    #[test]
    fn test_normalize() {
        let v = normalize([3.0, 4.0, 0.0]);
        assert!(approx_eq(v[0], 0.6));
        assert!(approx_eq(v[1], 0.8));
        assert!(approx_eq(v[2], 0.0));
    }

    #[test]
    fn test_normalize_zero_vector() {
        let v = normalize([0.0, 0.0, 0.0]);
        assert_eq!(v, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_cross_product() {
        let result = cross([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!(approx_eq(result[0], 0.0));
        assert!(approx_eq(result[1], 0.0));
        assert!(approx_eq(result[2], 1.0));
    }

    #[test]
    fn test_dot_product() {
        assert!(approx_eq(dot([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]), 32.0));
    }

    // ---- Camera tests ----

    #[test]
    fn camera_default_position() {
        let cam = Camera::default();
        assert_eq!(cam.position, [0.0, 5.0, 10.0]);
    }

    #[test]
    fn camera_forward_at_zero_yaw() {
        let cam = Camera::new([0.0, 0.0, 0.0]);
        let fwd = cam.forward();
        // At yaw=0, forward should be -Z
        assert!(approx_eq(fwd[0], 0.0));
        assert!(approx_eq(fwd[1], 0.0));
        assert!(approx_eq(fwd[2], -1.0));
    }

    #[test]
    fn camera_right_at_zero_yaw() {
        let cam = Camera::new([0.0, 0.0, 0.0]);
        let right = cam.right();
        // At yaw=0, right should be +X
        assert!(approx_eq(right[0], 1.0));
        assert!(approx_eq(right[1], 0.0));
        assert!(approx_eq(right[2], 0.0));
    }

    #[test]
    fn camera_look_direction_with_pitch() {
        let mut cam = Camera::new([0.0, 0.0, 0.0]);
        cam.pitch = std::f32::consts::FRAC_PI_4; // 45 degrees up
        let look = cam.look_direction();
        // Should look partly up and partly -Z
        assert!(look[1] > 0.0);
        assert!(look[2] < 0.0);
        let len = (look[0].powi(2) + look[1].powi(2) + look[2].powi(2)).sqrt();
        assert!(approx_eq(len, 1.0));
    }

    #[test]
    fn camera_translate_forward() {
        let mut cam = Camera::new([0.0, 0.0, 0.0]);
        cam.translate(1.0, 0.0, 0.0);
        // At yaw=0, forward is -Z
        assert!(approx_eq(cam.position[0], 0.0));
        assert!(approx_eq(cam.position[2], -1.0));
    }

    #[test]
    fn camera_translate_right() {
        let mut cam = Camera::new([0.0, 0.0, 0.0]);
        cam.translate(0.0, 1.0, 0.0);
        // At yaw=0, right is +X
        assert!(approx_eq(cam.position[0], 1.0));
        assert!(approx_eq(cam.position[2], 0.0));
    }

    #[test]
    fn camera_translate_up() {
        let mut cam = Camera::new([0.0, 0.0, 0.0]);
        cam.translate(0.0, 0.0, 3.0);
        assert!(approx_eq(cam.position[1], 3.0));
    }

    #[test]
    fn camera_pitch_clamped() {
        let mut cam = Camera::new([0.0, 0.0, 0.0]);
        cam.rotate(0.0, 100.0);
        assert!(cam.pitch <= MAX_PITCH);
        cam.rotate(0.0, -200.0);
        assert!(cam.pitch >= MIN_PITCH);
    }

    #[test]
    fn camera_aspect_ratio() {
        let mut cam = Camera::default();
        cam.set_aspect_ratio(1920, 1080);
        assert!(approx_eq(cam.aspect_ratio, 1920.0 / 1080.0));
    }

    #[test]
    fn camera_aspect_ratio_zero_height() {
        let mut cam = Camera::default();
        let original = cam.aspect_ratio;
        cam.set_aspect_ratio(1920, 0);
        // Should not change
        assert!(approx_eq(cam.aspect_ratio, original));
    }

    // ---- Matrix tests ----

    #[test]
    fn identity_matrix_is_correct() {
        let m = identity();
        for i in 0..4 {
            for j in 0..4 {
                if i == j {
                    assert!(approx_eq(m[i][j], 1.0));
                } else {
                    assert!(approx_eq(m[i][j], 0.0));
                }
            }
        }
    }

    #[test]
    fn translation_matrix_correct() {
        let m = translation_matrix(1.0, 2.0, 3.0);
        assert!(approx_eq(m[3][0], 1.0));
        assert!(approx_eq(m[3][1], 2.0));
        assert!(approx_eq(m[3][2], 3.0));
        assert!(approx_eq(m[0][0], 1.0));
        assert!(approx_eq(m[1][1], 1.0));
        assert!(approx_eq(m[2][2], 1.0));
    }

    #[test]
    fn scale_matrix_correct() {
        let m = scale_matrix(2.0, 3.0, 4.0);
        assert!(approx_eq(m[0][0], 2.0));
        assert!(approx_eq(m[1][1], 3.0));
        assert!(approx_eq(m[2][2], 4.0));
        assert!(approx_eq(m[3][3], 1.0));
    }

    #[test]
    fn mat4_mul_identity() {
        let a = translation_matrix(1.0, 2.0, 3.0);
        let result = mat4_mul(a, identity());
        for i in 0..4 {
            for j in 0..4 {
                assert!(approx_eq(result[i][j], a[i][j]));
            }
        }
    }

    #[test]
    fn mat4_mul_translation_composition() {
        let a = translation_matrix(1.0, 0.0, 0.0);
        let b = translation_matrix(0.0, 2.0, 0.0);
        let result = mat4_mul(a, b);
        // Combined should translate by (1, 2, 0)
        assert!(approx_eq(result[3][0], 1.0));
        assert!(approx_eq(result[3][1], 2.0));
        assert!(approx_eq(result[3][2], 0.0));
    }

    #[test]
    fn normal_matrix_identity() {
        let nm = normal_matrix(identity());
        for i in 0..3 {
            for j in 0..3 {
                if i == j {
                    assert!(approx_eq(nm[i][j], 1.0));
                } else {
                    assert!(approx_eq(nm[i][j], 0.0));
                }
            }
        }
    }

    #[test]
    fn normal_matrix_uniform_scale() {
        let model = scale_matrix(2.0, 2.0, 2.0);
        let nm = normal_matrix(model);
        // For uniform scale of 2, inverse-transpose of 3x3 should have 0.5 on diagonal
        assert!(approx_eq(nm[0][0], 0.5));
        assert!(approx_eq(nm[1][1], 0.5));
        assert!(approx_eq(nm[2][2], 0.5));
    }

    #[test]
    fn look_at_basic() {
        let view = look_at([0.0, 0.0, 5.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        // Looking from +Z towards origin, the forward (-Z in view space) maps to -Z in world
        // The view matrix should be valid (not all zeros)
        let determinant_proxy = view[0][0] * view[1][1] - view[0][1] * view[1][0];
        assert!(determinant_proxy.abs() > 1e-6);
    }

    #[test]
    fn perspective_basic() {
        let proj = perspective(std::f32::consts::FRAC_PI_3, 16.0 / 9.0, 0.1, 100.0);
        // proj[2][3] should be -1 for standard perspective
        assert!(approx_eq(proj[2][3], -1.0));
        // proj[3][3] should be 0
        assert!(approx_eq(proj[3][3], 0.0));
    }

    #[test]
    fn camera_to_uniforms_produces_valid_data() {
        let cam = Camera::default();
        let uniforms = cam.to_uniforms();
        // View position should match camera position
        assert!(approx_eq(uniforms.view_position[0], cam.position[0]));
        assert!(approx_eq(uniforms.view_position[1], cam.position[1]));
        assert!(approx_eq(uniforms.view_position[2], cam.position[2]));
        assert!(approx_eq(uniforms.view_position[3], 1.0));
    }

    #[test]
    fn fov_from_env_returns_default() {
        // When env var is not set, should return default
        let fov = fov_from_env();
        // It will be default unless env var is set in the test environment
        assert!(fov > 0.0);
    }
}
