#[derive(Debug, Clone, Copy)]
pub struct Quantization {
    pub pos_mm_step: f32,
    pub rot_bits_per_component: u8,
    pub rot_bits_total: u16,
}

impl Default for Quantization {
    fn default() -> Self {
        Self {
            pos_mm_step: 0.001,
            rot_bits_per_component: 10,
            rot_bits_total: 30,
        }
    }
}

#[derive(Debug)]
pub struct QuantizedFrame {
    pub entity_id: u64,
    pub x_mm: i32,
    pub y_mm: i32,
    pub z_mm: i32,
    pub rot_pitch: u16,
    pub rot_yaw: u16,
    pub rot_roll: u16,
}

impl QuantizedFrame {
    pub fn from_floats(entity_id: u64, x: f32, y: f32, z: f32, pitch: f32, yaw: f32, roll: f32) -> Self {
        let cfg = Quantization::default();
        Self {
            entity_id,
            x_mm: (x / cfg.pos_mm_step).round() as i32,
            y_mm: (y / cfg.pos_mm_step).round() as i32,
            z_mm: (z / cfg.pos_mm_step).round() as i32,
            rot_pitch: encode_smallest_three(pitch, cfg.rot_bits_per_component),
            rot_yaw: encode_smallest_three(yaw, cfg.rot_bits_per_component),
            rot_roll: encode_smallest_three(roll, cfg.rot_bits_per_component),
        }
    }
}

pub fn encode_smallest_three(angle_deg: f32, bits: u8) -> u16 {
    let max_steps = ((1u32 << bits) as f32) - 1.0;
    let normalized = (angle_deg / 360.0).fract().abs();
    (normalized * max_steps).round() as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantization_uses_1mm_pos_resolution() {
        let q = QuantizedFrame::from_floats(1, 1.234, -0.2, 0.8, 45.0, 90.0, 180.0);
        assert_eq!(q.x_mm, 1234);
        assert_eq!(q.entity_id, 1);
    }
}
