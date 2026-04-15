//! `aether-renderer-soft` — a minimal, zero-dependency software rasterizer
//! for unit tests, headless demos, and tooling.
//!
//! # Pixel format
//! All public drawing APIs operate on `&mut [u32]` framebuffers in
//! **ARGB packed** layout: bits 31..24 = alpha (MSB), 23..16 = red,
//! 15..8 = green, 7..0 = blue. Framebuffers are row-major,
//! `fb.len() == fb_w * fb_h`.
//!
//! # Module map
//! - [`camera`] — 3D camera, view/projection, world-to-screen projection.
//! - [`zbuffer`] — per-pixel depth buffer.
//! - [`voxel_draw`] — single AABB cube rasterizer with Lambert shading.
//! - [`voxel_model`] — batches of cubes with a simple transform.
//! - [`bitmap_font`] — baked 5x7 ASCII font + text drawing.
//! - [`fx`] — particles, floating text, hit rings, screen shake.
//! - [`ui_overlay`] — 2D overlay primitives (rect, border, bar, panel).

pub mod bitmap_font;
pub mod camera;
pub mod fx;
pub mod ui_overlay;
pub mod voxel_draw;
pub mod voxel_model;
pub mod zbuffer;

pub use bitmap_font::{draw_text, draw_text_scaled, GLYPH_HEIGHT, GLYPH_WIDTH};
pub use camera::{mat4_mul, Camera};
pub use fx::{FloaterText, FxState, HitRing, Particle, ScreenShake};
pub use ui_overlay::{draw_bar, draw_border, draw_panel, draw_rect};
pub use voxel_draw::draw_voxel_cube;
pub use voxel_model::{draw_voxel_model, Transform3, VoxelBox, VoxelModel};
pub use zbuffer::ZBuffer;

#[cfg(test)]
mod integration_tests {
    use super::*;

    const FB_W: u32 = 64;
    const FB_H: u32 = 48;
    const BG: u32 = 0xff000000;

    #[test]
    fn end_to_end_scene_writes_real_pixels() {
        let mut fb = vec![BG; (FB_W * FB_H) as usize];
        let mut zb = ZBuffer::new(FB_W, FB_H);

        let cam = Camera::new([0.0, 0.0, 0.0], 0.0, 0.0);

        let mut model = VoxelModel::new();
        model.push(VoxelBox {
            offset: [0.0, 0.0, 0.0],
            size: 0.8,
            color: 0xff3380ff,
        });
        let xform = Transform3 {
            position: [0.0, 0.0, -5.0],
            rotation_y: 0.0,
            scale: 1.0,
        };

        draw_voxel_model(&mut fb, FB_W, FB_H, &mut zb, &cam, &model, &xform, [0.0, 0.0, 1.0]);
        draw_text(&mut fb, FB_W, FB_H, 2, 2, "HI", 0xffffffff);

        let lit = fb.iter().filter(|px| **px != BG).count();
        assert!(lit > 0, "expected the integration scene to render real pixels");
    }
}
