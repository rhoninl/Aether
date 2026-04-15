//! 2D overlay primitives: rect, border, bar, panel. All coordinates are
//! clipped to the framebuffer. `value` parameters are clamped to [0, 1].

pub use crate::bitmap_font::{draw_text, draw_text_scaled};

pub fn draw_rect(fb: &mut [u32], fb_w: u32, fb_h: u32, x: i32, y: i32, w: u32, h: u32, color: u32) {
    if fb.len() != (fb_w * fb_h) as usize || fb_w == 0 || fb_h == 0 || w == 0 || h == 0 {
        return;
    }
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w as i32).min(fb_w as i32);
    let y1 = (y + h as i32).min(fb_h as i32);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    for py in y0..y1 {
        for px in x0..x1 {
            let idx = (py as u32 * fb_w + px as u32) as usize;
            fb[idx] = color;
        }
    }
}

pub fn draw_border(
    fb: &mut [u32],
    fb_w: u32,
    fb_h: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    color: u32,
    thickness: u32,
) {
    if thickness == 0 || w == 0 || h == 0 {
        return;
    }
    let t = thickness.min(w.min(h) / 2 + 1);
    draw_rect(fb, fb_w, fb_h, x, y, w, t, color);
    draw_rect(fb, fb_w, fb_h, x, y + h as i32 - t as i32, w, t, color);
    draw_rect(fb, fb_w, fb_h, x, y, t, h, color);
    draw_rect(fb, fb_w, fb_h, x + w as i32 - t as i32, y, t, h, color);
}

pub fn draw_bar(
    fb: &mut [u32],
    fb_w: u32,
    fb_h: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    value: f32,
    fg: u32,
    bg: u32,
) {
    let v = value.clamp(0.0, 1.0);
    draw_rect(fb, fb_w, fb_h, x, y, w, h, bg);
    let fg_w = (w as f32 * v).round() as u32;
    if fg_w > 0 {
        draw_rect(fb, fb_w, fb_h, x, y, fg_w, h, fg);
    }
}

pub fn draw_panel(
    fb: &mut [u32],
    fb_w: u32,
    fb_h: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    bg: u32,
    border: u32,
) {
    draw_rect(fb, fb_w, fb_h, x, y, w, h, bg);
    draw_border(fb, fb_w, fb_h, x, y, w, h, border, 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    const BG: u32 = 0xff000000;
    const FG: u32 = 0xffffffff;

    fn empty(w: u32, h: u32) -> Vec<u32> {
        vec![BG; (w * h) as usize]
    }

    fn count(fb: &[u32], color: u32) -> usize {
        fb.iter().filter(|px| **px == color).count()
    }

    #[test]
    fn draw_rect_fills_expected_pixels() {
        let mut fb = empty(10, 10);
        draw_rect(&mut fb, 10, 10, 2, 3, 4, 5, FG);
        assert_eq!(count(&fb, FG), 20);
    }

    #[test]
    fn draw_rect_clips_negative_origin() {
        let mut fb = empty(10, 10);
        draw_rect(&mut fb, 10, 10, -2, -3, 4, 5, FG);
        // Visible part is 2 wide, 2 tall = 4 pixels.
        assert_eq!(count(&fb, FG), 4);
    }

    #[test]
    fn draw_rect_clips_past_edge() {
        let mut fb = empty(10, 10);
        draw_rect(&mut fb, 10, 10, 8, 8, 5, 5, FG);
        assert_eq!(count(&fb, FG), 4);
    }

    #[test]
    fn draw_rect_fully_outside_is_noop() {
        let mut fb = empty(10, 10);
        draw_rect(&mut fb, 10, 10, 100, 100, 5, 5, FG);
        assert_eq!(count(&fb, FG), 0);
        draw_rect(&mut fb, 10, 10, -100, -100, 5, 5, FG);
        assert_eq!(count(&fb, FG), 0);
    }

    #[test]
    fn draw_rect_zero_size_is_noop() {
        let mut fb = empty(10, 10);
        draw_rect(&mut fb, 10, 10, 0, 0, 0, 5, FG);
        draw_rect(&mut fb, 10, 10, 0, 0, 5, 0, FG);
        assert_eq!(count(&fb, FG), 0);
    }

    #[test]
    fn draw_bar_half_fills_half() {
        let mut fb = empty(20, 10);
        draw_bar(&mut fb, 20, 10, 0, 0, 10, 2, 0.5, FG, 0xff808080);
        let fg = count(&fb, FG);
        let bg = count(&fb, 0xff808080);
        assert_eq!(fg, 10);
        assert_eq!(bg, 10);
    }

    #[test]
    fn draw_bar_clamps_value() {
        let mut fb = empty(10, 4);
        draw_bar(&mut fb, 10, 4, 0, 0, 10, 2, 2.5, FG, 0xff222222);
        assert_eq!(count(&fb, FG), 20);
        draw_bar(&mut fb, 10, 4, 0, 0, 10, 2, -0.5, FG, 0xff222222);
        assert_eq!(count(&fb, FG), 0);
        assert_eq!(count(&fb, 0xff222222), 20);
    }

    #[test]
    fn draw_border_thickness_one() {
        let mut fb = empty(10, 10);
        draw_border(&mut fb, 10, 10, 0, 0, 10, 10, FG, 1);
        // Perimeter of 10x10 rect = 4*10 - 4 = 36 pixels.
        assert_eq!(count(&fb, FG), 36);
    }

    #[test]
    fn draw_panel_has_bg_and_border() {
        let mut fb = empty(10, 10);
        draw_panel(&mut fb, 10, 10, 0, 0, 10, 10, 0xff222222, FG);
        assert_eq!(count(&fb, FG), 36);
        assert_eq!(count(&fb, 0xff222222), 64);
    }

    #[test]
    fn draw_rect_wraps_around_out_of_bounds_coords() {
        // Sanity: very large negative + positive origin should still not panic.
        let mut fb = empty(10, 10);
        draw_rect(&mut fb, 10, 10, -1_000, 0, 5, 5, FG);
        assert_eq!(count(&fb, FG), 0);
    }
}
