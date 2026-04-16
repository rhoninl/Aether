//! 3D camera with view/projection matrices and world->screen projection.
//!
//! Coordinate convention: right-handed. +X right, +Y up, +Z out of screen.
//! `yaw` rotates around +Y (positive = look left), `pitch` rotates around +X
//! (positive = look up). The camera looks down -Z when yaw=0, pitch=0.

const DEFAULT_FOV_Y_RAD: f32 = 1.2217; // ~70 degrees
const DEFAULT_NEAR: f32 = 0.1;
const DEFAULT_FAR: f32 = 500.0;

pub struct Camera {
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub fov_y_rad: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn new(pos: [f32; 3], yaw: f32, pitch: f32) -> Self {
        Self {
            pos,
            yaw,
            pitch,
            fov_y_rad: DEFAULT_FOV_Y_RAD,
            near: DEFAULT_NEAR,
            far: DEFAULT_FAR,
        }
    }

    /// Row-major 4x4 view matrix: rotates world into camera space, then
    /// translates by -pos. `m[row][col]`.
    pub fn view_matrix(&self) -> [[f32; 4]; 4] {
        let (sy, cy) = (self.yaw.sin(), self.yaw.cos());
        let (sp, cp) = (self.pitch.sin(), self.pitch.cos());

        // Rotation about Y (yaw), then about X (pitch). Inverse of camera
        // orientation; we rotate the world by the negative of the camera's
        // yaw/pitch.
        let ry: [[f32; 4]; 4] = [
            [cy, 0.0, -sy, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [sy, 0.0, cy, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let rx: [[f32; 4]; 4] = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, cp, sp, 0.0],
            [0.0, -sp, cp, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let t: [[f32; 4]; 4] = [
            [1.0, 0.0, 0.0, -self.pos[0]],
            [0.0, 1.0, 0.0, -self.pos[1]],
            [0.0, 0.0, 1.0, -self.pos[2]],
            [0.0, 0.0, 0.0, 1.0],
        ];
        mat4_mul(rx, mat4_mul(ry, t))
    }

    /// Row-major perspective projection to clip space. Maps z in [near, far]
    /// to clip-z in [-1, 1].
    pub fn proj_matrix(&self, aspect: f32) -> [[f32; 4]; 4] {
        let f = 1.0 / (self.fov_y_rad * 0.5).tan();
        let nf = 1.0 / (self.near - self.far);
        [
            [f / aspect, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [
                0.0,
                0.0,
                (self.far + self.near) * nf,
                2.0 * self.far * self.near * nf,
            ],
            [0.0, 0.0, -1.0, 0.0],
        ]
    }

    /// Transforms a world-space point to framebuffer pixel coords + depth.
    /// Returns None if behind camera or outside the frustum. Depth is the
    /// camera-space distance along -Z (positive for points in front).
    pub fn world_to_screen(
        &self,
        point: [f32; 3],
        fb_w: u32,
        fb_h: u32,
    ) -> Option<(i32, i32, f32)> {
        if fb_w == 0 || fb_h == 0 {
            return None;
        }
        let aspect = fb_w as f32 / fb_h as f32;
        let view = self.view_matrix();
        let proj = self.proj_matrix(aspect);

        let world = [point[0], point[1], point[2], 1.0];
        let v = mat4_mul_vec(view, world);
        // Depth along -Z (points in front of camera have v[2] < 0).
        let depth = -v[2];
        if depth <= self.near || depth >= self.far {
            return None;
        }
        let c = mat4_mul_vec(proj, v);
        if c[3].abs() < f32::EPSILON {
            return None;
        }
        let ndc_x = c[0] / c[3];
        let ndc_y = c[1] / c[3];
        if !(-1.0..=1.0).contains(&ndc_x) || !(-1.0..=1.0).contains(&ndc_y) {
            return None;
        }
        let sx = ((ndc_x * 0.5 + 0.5) * fb_w as f32).round() as i32;
        let sy = ((1.0 - (ndc_y * 0.5 + 0.5)) * fb_h as f32).round() as i32;
        Some((sx, sy, depth))
    }
}

/// Row-major 4x4 matrix multiplication: result[i][j] = sum_k a[i][k]*b[k][j].
pub fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += a[i][k] * b[k][j];
            }
            out[i][j] = s;
        }
    }
    out
}

/// Row-major 4x4 * column-vec4 multiply.
pub fn mat4_mul_vec(a: [[f32; 4]; 4], v: [f32; 4]) -> [f32; 4] {
    let mut out = [0.0f32; 4];
    for i in 0..4 {
        out[i] = a[i][0] * v[0] + a[i][1] * v[1] + a[i][2] * v[2] + a[i][3] * v[3];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> [[f32; 4]; 4] {
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    #[test]
    fn mat4_mul_identity_is_self() {
        let m: [[f32; 4]; 4] = [
            [1.0, 2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
            [13.0, 14.0, 15.0, 16.0],
        ];
        let out = mat4_mul(m, identity());
        assert_eq!(out, m);
        let out2 = mat4_mul(identity(), m);
        assert_eq!(out2, m);
    }

    #[test]
    fn mat4_mul_associativity_spot_check() {
        let a: [[f32; 4]; 4] = [
            [1.0, 0.0, 2.0, 0.0],
            [0.0, 1.0, 0.0, 3.0],
            [4.0, 0.0, 5.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let b: [[f32; 4]; 4] = [
            [0.0, 1.0, 0.0, 2.0],
            [1.0, 0.0, 3.0, 0.0],
            [0.0, 2.0, 0.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let c: [[f32; 4]; 4] = [
            [2.0, 0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let left = mat4_mul(mat4_mul(a, b), c);
        let right = mat4_mul(a, mat4_mul(b, c));
        for i in 0..4 {
            for j in 0..4 {
                assert!((left[i][j] - right[i][j]).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn identity_view_for_origin_camera() {
        let cam = Camera::new([0.0, 0.0, 0.0], 0.0, 0.0);
        let v = cam.view_matrix();
        let expected = identity();
        for i in 0..4 {
            for j in 0..4 {
                assert!((v[i][j] - expected[i][j]).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn point_in_front_projects_to_sensible_pixel() {
        let cam = Camera::new([0.0, 0.0, 0.0], 0.0, 0.0);
        let result = cam.world_to_screen([0.0, 0.0, -5.0], 100, 100);
        let (sx, sy, depth) = result.expect("should project");
        assert!((sx - 50).abs() <= 1, "x should be near center, got {}", sx);
        assert!((sy - 50).abs() <= 1, "y should be near center, got {}", sy);
        assert!((depth - 5.0).abs() < 0.001);
    }

    #[test]
    fn point_behind_camera_returns_none() {
        let cam = Camera::new([0.0, 0.0, 0.0], 0.0, 0.0);
        assert!(cam.world_to_screen([0.0, 0.0, 5.0], 100, 100).is_none());
    }

    #[test]
    fn point_beyond_far_plane_returns_none() {
        let cam = Camera::new([0.0, 0.0, 0.0], 0.0, 0.0);
        assert!(cam
            .world_to_screen([0.0, 0.0, -10_000.0], 100, 100)
            .is_none());
    }

    #[test]
    fn offscreen_point_returns_none() {
        let cam = Camera::new([0.0, 0.0, 0.0], 0.0, 0.0);
        assert!(cam.world_to_screen([100.0, 0.0, -5.0], 100, 100).is_none());
    }

    #[test]
    fn proj_matrix_uses_aspect() {
        let cam = Camera::new([0.0, 0.0, 0.0], 0.0, 0.0);
        let wide = cam.proj_matrix(2.0);
        let square = cam.proj_matrix(1.0);
        assert!(wide[0][0] < square[0][0]);
    }
}
