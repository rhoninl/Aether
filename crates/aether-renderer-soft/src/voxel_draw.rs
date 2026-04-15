//! Dead-simple voxel cube rasterizer.
//!
//! Each cube is projected via `Camera::world_to_screen`, back-facing cubes
//! are culled, the screen-space axis-aligned bounding box of the visible
//! corners is filled with a Lambert-shaded color, and the z-buffer rejects
//! pixels covered by something closer.
//!
//! This is intentionally the simplest approach that produces a recognizable
//! block: no perspective-correct interpolation, no per-pixel normals. Good
//! enough for unit tests and toy demos.

use crate::camera::Camera;
use crate::zbuffer::ZBuffer;

const CUBE_CORNERS: [[f32; 3]; 8] = [
    [-0.5, -0.5, -0.5],
    [0.5, -0.5, -0.5],
    [-0.5, 0.5, -0.5],
    [0.5, 0.5, -0.5],
    [-0.5, -0.5, 0.5],
    [0.5, -0.5, 0.5],
    [-0.5, 0.5, 0.5],
    [0.5, 0.5, 0.5],
];

const FACE_NORMALS: [[f32; 3]; 6] = [
    [1.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, -1.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, -1.0],
];

const AMBIENT: f32 = 0.25;
const DIFFUSE: f32 = 0.75;

/// Draws a single axis-aligned cube centered at `center` with side length `size`.
///
/// Framebuffer pixel format: ARGB packed u32, MSB = alpha. Writes opaque pixels.
pub fn draw_voxel_cube(
    fb: &mut [u32],
    fb_w: u32,
    fb_h: u32,
    zb: &mut ZBuffer,
    cam: &Camera,
    center: [f32; 3],
    size: f32,
    base_color: u32,
    light_dir: [f32; 3],
) {
    if fb.len() != (fb_w * fb_h) as usize || fb_w == 0 || fb_h == 0 {
        return;
    }

    // Project 8 corners; track screen-space AABB and the minimum depth.
    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    let mut min_depth = f32::INFINITY;
    let mut any_visible = false;

    for corner in CUBE_CORNERS.iter() {
        let world = [
            center[0] + corner[0] * size,
            center[1] + corner[1] * size,
            center[2] + corner[2] * size,
        ];
        if let Some((sx, sy, depth)) = cam.world_to_screen(world, fb_w, fb_h) {
            any_visible = true;
            if sx < min_x {
                min_x = sx;
            }
            if sx > max_x {
                max_x = sx;
            }
            if sy < min_y {
                min_y = sy;
            }
            if sy > max_y {
                max_y = sy;
            }
            if depth < min_depth {
                min_depth = depth;
            }
        }
    }

    if !any_visible {
        return;
    }

    // Facing-aware shading: pick the strongest-lit visible face.
    // For a simple AABB cube we average the unit normals of the three faces
    // that face the camera; back-face culling is applied by checking the
    // vector from cube center to camera.
    let light = normalize(light_dir);
    let to_cam = normalize([
        cam.pos[0] - center[0],
        cam.pos[1] - center[1],
        cam.pos[2] - center[2],
    ]);

    let mut best_intensity: f32 = 0.0;
    let mut any_front_facing = false;
    for n in FACE_NORMALS.iter() {
        if dot(*n, to_cam) > 0.0 {
            any_front_facing = true;
            let lambert = dot(*n, light).max(0.0);
            let shade = AMBIENT + DIFFUSE * lambert;
            if shade > best_intensity {
                best_intensity = shade;
            }
        }
    }
    if !any_front_facing {
        return;
    }

    let shaded = shade_color(base_color, best_intensity);

    // Clip the bounding box to the framebuffer.
    let x0 = min_x.max(0);
    let y0 = min_y.max(0);
    let x1 = max_x.min(fb_w as i32 - 1);
    let y1 = max_y.min(fb_h as i32 - 1);
    if x1 < x0 || y1 < y0 {
        return;
    }

    for y in y0..=y1 {
        for x in x0..=x1 {
            if zb.test_and_set(x, y, min_depth) {
                let idx = (y as u32 * fb_w + x as u32) as usize;
                fb[idx] = shaded;
            }
        }
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < f32::EPSILON {
        [0.0, 0.0, 0.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

fn shade_color(color: u32, intensity: f32) -> u32 {
    let clamped = intensity.clamp(0.0, 1.0);
    let a = (color >> 24) & 0xff;
    let r = ((color >> 16) & 0xff) as f32 * clamped;
    let g = ((color >> 8) & 0xff) as f32 * clamped;
    let b = (color & 0xff) as f32 * clamped;
    (a << 24)
        | ((r.round() as u32 & 0xff) << 16)
        | ((g.round() as u32 & 0xff) << 8)
        | (b.round() as u32 & 0xff)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BG: u32 = 0xff000000;

    fn empty_fb(w: u32, h: u32) -> Vec<u32> {
        vec![BG; (w * h) as usize]
    }

    #[test]
    fn draws_pixels_for_visible_cube() {
        let mut fb = empty_fb(64, 48);
        let mut zb = ZBuffer::new(64, 48);
        let cam = Camera::new([0.0, 0.0, 0.0], 0.0, 0.0);
        draw_voxel_cube(
            &mut fb,
            64,
            48,
            &mut zb,
            &cam,
            [0.0, 0.0, -5.0],
            1.0,
            0xffff0000,
            [0.0, 0.0, 1.0],
        );
        let lit = fb.iter().filter(|px| **px != BG).count();
        assert!(lit > 0, "expected at least one lit pixel");
    }

    #[test]
    fn cube_behind_camera_writes_nothing() {
        let mut fb = empty_fb(64, 48);
        let mut zb = ZBuffer::new(64, 48);
        let cam = Camera::new([0.0, 0.0, 0.0], 0.0, 0.0);
        draw_voxel_cube(
            &mut fb,
            64,
            48,
            &mut zb,
            &cam,
            [0.0, 0.0, 5.0],
            1.0,
            0xffff0000,
            [0.0, 0.0, 1.0],
        );
        assert!(fb.iter().all(|px| *px == BG));
    }

    #[test]
    fn near_cube_occludes_far_cube() {
        let mut fb = empty_fb(64, 48);
        let mut zb = ZBuffer::new(64, 48);
        let cam = Camera::new([0.0, 0.0, 0.0], 0.0, 0.0);
        // Draw far first in red, then near in green.
        draw_voxel_cube(
            &mut fb,
            64,
            48,
            &mut zb,
            &cam,
            [0.0, 0.0, -20.0],
            1.0,
            0xffff0000,
            [0.0, 0.0, 1.0],
        );
        draw_voxel_cube(
            &mut fb,
            64,
            48,
            &mut zb,
            &cam,
            [0.0, 0.0, -5.0],
            1.0,
            0xff00ff00,
            [0.0, 0.0, 1.0],
        );
        // Center pixel should be shaded green (near cube).
        let idx = (24_u32 * 64 + 32) as usize;
        let r = (fb[idx] >> 16) & 0xff;
        let g = (fb[idx] >> 8) & 0xff;
        assert!(g > 0, "green channel should be lit, got {:08x}", fb[idx]);
        assert_eq!(r, 0, "red from far cube should be occluded");
    }

    #[test]
    fn shade_color_at_zero_intensity_is_black() {
        let c = shade_color(0xffff00ff, 0.0);
        assert_eq!(c, 0xff000000);
    }

    #[test]
    fn shade_color_at_one_is_unchanged() {
        let c = shade_color(0xff804020, 1.0);
        assert_eq!(c, 0xff804020);
    }
}
