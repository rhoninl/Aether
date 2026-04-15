//! A VoxelModel is a collection of axis-aligned boxes transformed by a
//! simple Transform3 (translation + Y-axis rotation + uniform scale).

use crate::camera::Camera;
use crate::voxel_draw::draw_voxel_cube;
use crate::zbuffer::ZBuffer;

pub struct Transform3 {
    pub position: [f32; 3],
    pub rotation_y: f32,
    pub scale: f32,
}

impl Transform3 {
    pub fn identity() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation_y: 0.0,
            scale: 1.0,
        }
    }

    /// Transforms a local-space offset into world space.
    pub fn apply(&self, offset: [f32; 3]) -> [f32; 3] {
        let (s, c) = (self.rotation_y.sin(), self.rotation_y.cos());
        let scaled = [offset[0] * self.scale, offset[1] * self.scale, offset[2] * self.scale];
        let rx = c * scaled[0] + s * scaled[2];
        let rz = -s * scaled[0] + c * scaled[2];
        [
            rx + self.position[0],
            scaled[1] + self.position[1],
            rz + self.position[2],
        ]
    }
}

pub struct VoxelBox {
    pub offset: [f32; 3],
    pub size: f32,
    pub color: u32,
}

pub struct VoxelModel {
    pub boxes: Vec<VoxelBox>,
}

impl VoxelModel {
    pub fn new() -> Self {
        Self { boxes: Vec::new() }
    }

    pub fn push(&mut self, b: VoxelBox) {
        self.boxes.push(b);
    }
}

impl Default for VoxelModel {
    fn default() -> Self {
        Self::new()
    }
}

pub fn draw_voxel_model(
    fb: &mut [u32],
    fb_w: u32,
    fb_h: u32,
    zb: &mut ZBuffer,
    cam: &Camera,
    model: &VoxelModel,
    xform: &Transform3,
    light_dir: [f32; 3],
) {
    for b in model.boxes.iter() {
        let world_center = xform.apply(b.offset);
        draw_voxel_cube(
            fb,
            fb_w,
            fb_h,
            zb,
            cam,
            world_center,
            b.size * xform.scale,
            b.color,
            light_dir,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BG: u32 = 0xff000000;

    fn empty_fb(w: u32, h: u32) -> Vec<u32> {
        vec![BG; (w * h) as usize]
    }

    #[test]
    fn transform_identity_preserves_offset() {
        let t = Transform3::identity();
        let p = t.apply([1.0, 2.0, 3.0]);
        assert!((p[0] - 1.0).abs() < 1e-5);
        assert!((p[1] - 2.0).abs() < 1e-5);
        assert!((p[2] - 3.0).abs() < 1e-5);
    }

    #[test]
    fn transform_translates() {
        let t = Transform3 {
            position: [10.0, 20.0, 30.0],
            rotation_y: 0.0,
            scale: 1.0,
        };
        let p = t.apply([1.0, 2.0, 3.0]);
        assert!((p[0] - 11.0).abs() < 1e-5);
        assert!((p[1] - 22.0).abs() < 1e-5);
        assert!((p[2] - 33.0).abs() < 1e-5);
    }

    #[test]
    fn transform_scales() {
        let t = Transform3 {
            position: [0.0, 0.0, 0.0],
            rotation_y: 0.0,
            scale: 2.0,
        };
        let p = t.apply([1.0, 2.0, 3.0]);
        assert!((p[0] - 2.0).abs() < 1e-5);
        assert!((p[1] - 4.0).abs() < 1e-5);
        assert!((p[2] - 6.0).abs() < 1e-5);
    }

    #[test]
    fn two_box_model_draws_pixels() {
        let mut fb = empty_fb(64, 48);
        let mut zb = ZBuffer::new(64, 48);
        let cam = Camera::new([0.0, 0.0, 0.0], 0.0, 0.0);
        let mut model = VoxelModel::new();
        model.push(VoxelBox {
            offset: [-0.5, 0.0, 0.0],
            size: 0.4,
            color: 0xffff0000,
        });
        model.push(VoxelBox {
            offset: [0.5, 0.0, 0.0],
            size: 0.4,
            color: 0xff00ff00,
        });
        let xform = Transform3 {
            position: [0.0, 0.0, -5.0],
            rotation_y: 0.0,
            scale: 1.0,
        };
        draw_voxel_model(&mut fb, 64, 48, &mut zb, &cam, &model, &xform, [0.0, 0.0, 1.0]);
        let red = fb.iter().filter(|px| (**px >> 16) & 0xff > 0 && (**px >> 8) & 0xff == 0).count();
        let green = fb.iter().filter(|px| (**px >> 8) & 0xff > 0 && (**px >> 16) & 0xff == 0).count();
        assert!(red > 0, "expected some red pixels for left box");
        assert!(green > 0, "expected some green pixels for right box");
    }

    #[test]
    fn translation_moves_pixels() {
        let cam = Camera::new([0.0, 0.0, 0.0], 0.0, 0.0);
        let mut model = VoxelModel::new();
        model.push(VoxelBox {
            offset: [0.0, 0.0, 0.0],
            size: 0.6,
            color: 0xffffffff,
        });

        let mut fb_a = empty_fb(64, 48);
        let mut zb_a = ZBuffer::new(64, 48);
        let xform_a = Transform3 {
            position: [0.0, 0.0, -5.0],
            rotation_y: 0.0,
            scale: 1.0,
        };
        draw_voxel_model(&mut fb_a, 64, 48, &mut zb_a, &cam, &model, &xform_a, [0.0, 0.0, 1.0]);

        let mut fb_b = empty_fb(64, 48);
        let mut zb_b = ZBuffer::new(64, 48);
        let xform_b = Transform3 {
            position: [1.0, 0.0, -5.0],
            rotation_y: 0.0,
            scale: 1.0,
        };
        draw_voxel_model(&mut fb_b, 64, 48, &mut zb_b, &cam, &model, &xform_b, [0.0, 0.0, 1.0]);

        assert_ne!(fb_a, fb_b, "translation should produce different framebuffers");
    }
}
