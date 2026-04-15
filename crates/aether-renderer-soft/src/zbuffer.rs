//! Simple per-pixel depth buffer. Lower depth == closer to camera.

const INITIAL_DEPTH: f32 = f32::INFINITY;

pub struct ZBuffer {
    w: u32,
    h: u32,
    data: Vec<f32>,
}

impl ZBuffer {
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            data: vec![INITIAL_DEPTH; (w * h) as usize],
        }
    }

    pub fn clear(&mut self) {
        for d in self.data.iter_mut() {
            *d = INITIAL_DEPTH;
        }
    }

    /// Returns true if (x, y, depth) is closer than the current depth and the
    /// new depth was stored. Out-of-bounds coordinates return false.
    pub fn test_and_set(&mut self, x: i32, y: i32, depth: f32) -> bool {
        if x < 0 || y < 0 || (x as u32) >= self.w || (y as u32) >= self.h {
            return false;
        }
        let idx = (y as u32 * self.w + x as u32) as usize;
        if depth < self.data[idx] {
            self.data[idx] = depth;
            true
        } else {
            false
        }
    }

    pub fn width(&self) -> u32 {
        self.w
    }

    pub fn height(&self) -> u32 {
        self.h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_sets_infinity() {
        let mut zb = ZBuffer::new(2, 2);
        assert!(zb.test_and_set(0, 0, 5.0));
        zb.clear();
        assert!(zb.test_and_set(0, 0, 10.0));
    }

    #[test]
    fn closer_wins() {
        let mut zb = ZBuffer::new(4, 4);
        assert!(zb.test_and_set(1, 1, 10.0));
        assert!(zb.test_and_set(1, 1, 5.0));
    }

    #[test]
    fn farther_loses() {
        let mut zb = ZBuffer::new(4, 4);
        assert!(zb.test_and_set(1, 1, 5.0));
        assert!(!zb.test_and_set(1, 1, 10.0));
    }

    #[test]
    fn equal_depth_is_not_written() {
        let mut zb = ZBuffer::new(4, 4);
        assert!(zb.test_and_set(0, 0, 5.0));
        assert!(!zb.test_and_set(0, 0, 5.0));
    }

    #[test]
    fn out_of_bounds_returns_false() {
        let mut zb = ZBuffer::new(4, 4);
        assert!(!zb.test_and_set(-1, 0, 1.0));
        assert!(!zb.test_and_set(0, -1, 1.0));
        assert!(!zb.test_and_set(4, 0, 1.0));
        assert!(!zb.test_and_set(0, 4, 1.0));
    }

    #[test]
    fn dimensions_report_correctly() {
        let zb = ZBuffer::new(8, 6);
        assert_eq!(zb.width(), 8);
        assert_eq!(zb.height(), 6);
    }
}
