//! Emulator window management and debug overlay rendering.
//!
//! Wraps `minifb` to provide a desktop window that displays the VR emulator
//! preview and a debug overlay with tracking information.

use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};

use crate::config::EmulatorConfig;
use crate::controller::ControllerInput;

/// Colors for the debug overlay.
const OVERLAY_TEXT_COLOR: u32 = 0x00ff00;
const STEREO_DIVIDER_COLOR: u32 = 0x333333;

/// Framebuffer for the emulator window.
#[derive(Debug)]
pub struct EmulatorFrameBuffer {
    /// Pixel data in 0xRRGGBB format.
    pub pixels: Vec<u32>,
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
}

impl EmulatorFrameBuffer {
    /// Create a new framebuffer with the given dimensions.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            pixels: vec![0; width * height],
            width,
            height,
        }
    }

    /// Clear the framebuffer to a sky gradient.
    pub fn clear_sky(&mut self) {
        let sky_top: u32 = 0x1a1a2e;
        let sky_bot: u32 = 0x16213e;
        for y in 0..self.height {
            let t = y as f32 / self.height as f32;
            let r = lerp_u8((sky_top >> 16) as u8, (sky_bot >> 16) as u8, t);
            let g = lerp_u8((sky_top >> 8) as u8, (sky_bot >> 8) as u8, t);
            let b = lerp_u8(sky_top as u8, sky_bot as u8, t);
            let color = (r as u32) << 16 | (g as u32) << 8 | b as u32;
            for x in 0..self.width {
                self.pixels[y * self.width + x] = color;
            }
        }
    }

    /// Clear the framebuffer to a solid color.
    pub fn clear_color(&mut self, color: u32) {
        self.pixels.fill(color);
    }

    /// Set a pixel at (x, y) if within bounds.
    pub fn set_pixel(&mut self, x: i32, y: i32, color: u32) {
        if x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32 {
            self.pixels[y as usize * self.width + x as usize] = color;
        }
    }

    /// Draw the stereo divider line (vertical line in the center).
    pub fn draw_stereo_divider(&mut self) {
        let mid_x = self.width / 2;
        for y in 0..self.height {
            self.pixels[y * self.width + mid_x] = STEREO_DIVIDER_COLOR;
        }
    }
}

/// Debug overlay information to display.
#[derive(Debug, Clone)]
pub struct DebugOverlayInfo {
    pub fps: f32,
    pub frame_time_ms: f32,
    pub head_position: [f32; 3],
    pub head_yaw_deg: f32,
    pub head_pitch_deg: f32,
    pub left_controller_pos: [f32; 3],
    pub right_controller_pos: [f32; 3],
    pub session_state: String,
    pub frame_count: u64,
}

/// Draw the debug overlay onto the framebuffer.
pub fn draw_debug_overlay(fb: &mut EmulatorFrameBuffer, info: &DebugOverlayInfo) {
    // Background strip at the top
    let strip_height = 56;
    for y in 0..strip_height.min(fb.height) {
        for x in 0..fb.width {
            let idx = y * fb.width + x;
            let c = fb.pixels[idx];
            let r = ((c >> 16) & 0xff) / 3;
            let g = ((c >> 8) & 0xff) / 3;
            let b = (c & 0xff) / 3;
            fb.pixels[idx] = (r << 16) | (g << 8) | b;
        }
    }

    let lines = [
        format!(
            "FPS: {:.0}  Frame: {:.1}ms  State: {}  Frame#: {}",
            info.fps, info.frame_time_ms, info.session_state, info.frame_count
        ),
        format!(
            "Head: ({:.2}, {:.2}, {:.2})  Yaw: {:.1}  Pitch: {:.1}",
            info.head_position[0],
            info.head_position[1],
            info.head_position[2],
            info.head_yaw_deg,
            info.head_pitch_deg
        ),
        format!(
            "L-Ctrl: ({:.2}, {:.2}, {:.2})  R-Ctrl: ({:.2}, {:.2}, {:.2})",
            info.left_controller_pos[0],
            info.left_controller_pos[1],
            info.left_controller_pos[2],
            info.right_controller_pos[0],
            info.right_controller_pos[1],
            info.right_controller_pos[2],
        ),
    ];

    for (i, line) in lines.iter().enumerate() {
        aether_vr_overlay::font::draw_text_u32(
            &mut fb.pixels,
            fb.width,
            fb.height,
            line,
            8,
            4 + i as i32 * 16,
            OVERLAY_TEXT_COLOR,
        );
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).clamp(0.0, 255.0) as u8
}

/// Emulator window wrapper around minifb.
pub struct EmulatorWindow {
    window: Window,
    width: usize,
    height: usize,
    mouse_x: f32,
    mouse_y: f32,
    prev_mouse_x: f32,
    prev_mouse_y: f32,
}

impl EmulatorWindow {
    /// Create a new emulator window.
    pub fn create(config: &EmulatorConfig) -> Result<Self, String> {
        let title = format!(
            "{} (VR Emulator - WASD=move, Mouse=look, 1-5=buttons, ESC=quit)",
            config.application_name
        );
        let window = Window::new(
            &title,
            config.window_width,
            config.window_height,
            WindowOptions {
                resize: false,
                ..WindowOptions::default()
            },
        )
        .map_err(|e| format!("failed to create window: {e}"))?;

        Ok(Self {
            window,
            width: config.window_width,
            height: config.window_height,
            mouse_x: 0.0,
            mouse_y: 0.0,
            prev_mouse_x: 0.0,
            prev_mouse_y: 0.0,
        })
    }

    /// Check if the window is still open (not closed by user).
    pub fn is_open(&self) -> bool {
        self.window.is_open() && !self.window.is_key_down(Key::Escape)
    }

    /// Set the target FPS for the window update loop.
    pub fn set_target_fps(&mut self, fps: usize) {
        self.window.set_target_fps(fps);
    }

    /// Poll input state and return a ControllerInput.
    pub fn poll_input(&mut self) -> (ControllerInput, f32, f32) {
        // Track mouse position for delta computation
        self.prev_mouse_x = self.mouse_x;
        self.prev_mouse_y = self.mouse_y;

        if let Some((mx, my)) = self.window.get_mouse_pos(MouseMode::Clamp) {
            self.mouse_x = mx;
            self.mouse_y = my;
        }

        let mouse_dx = self.mouse_x - self.prev_mouse_x;
        let mouse_dy = self.mouse_y - self.prev_mouse_y;

        // Normalize mouse aim to [-1, 1] range
        let aim_x = (self.mouse_x / self.width as f32) * 2.0 - 1.0;
        let aim_y = (self.mouse_y / self.height as f32) * 2.0 - 1.0;

        let input = ControllerInput {
            left_stick_up: self.window.is_key_down(Key::W),
            left_stick_down: self.window.is_key_down(Key::S),
            left_stick_left: self.window.is_key_down(Key::A),
            left_stick_right: self.window.is_key_down(Key::D),
            left_rotate_left: self.window.is_key_down(Key::Q),
            left_rotate_right: self.window.is_key_down(Key::E),
            right_aim_x: aim_x,
            right_aim_y: aim_y,
            left_trigger: self.window.is_key_down(Key::Space),
            right_trigger: self.window.get_mouse_down(MouseButton::Left),
            left_grip: self.window.is_key_down(Key::LeftShift),
            right_grip: self.window.get_mouse_down(MouseButton::Right),
            button_a: self.window.is_key_down(Key::Key1),
            button_b: self.window.is_key_down(Key::Key2),
            button_x: self.window.is_key_down(Key::Key3),
            button_y: self.window.is_key_down(Key::Key4),
            button_menu: self.window.is_key_down(Key::Key5),
            left_thumbstick_click: self.window.is_key_down(Key::Tab),
            right_thumbstick_click: self.window.get_mouse_down(MouseButton::Middle),
        };

        (input, mouse_dx, mouse_dy)
    }

    /// Check if a specific key is pressed.
    pub fn is_key_down(&self, key: Key) -> bool {
        self.window.is_key_down(key)
    }

    /// Present the framebuffer to the window.
    pub fn present(&mut self, fb: &EmulatorFrameBuffer) -> Result<(), String> {
        self.window
            .update_with_buffer(&fb.pixels, fb.width, fb.height)
            .map_err(|e| format!("failed to update window: {e}"))
    }

    /// Get the window dimensions.
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- EmulatorFrameBuffer tests ----

    #[test]
    fn framebuffer_new_correct_size() {
        let fb = EmulatorFrameBuffer::new(800, 600);
        assert_eq!(fb.pixels.len(), 800 * 600);
        assert_eq!(fb.width, 800);
        assert_eq!(fb.height, 600);
    }

    #[test]
    fn framebuffer_new_initialized_to_zero() {
        let fb = EmulatorFrameBuffer::new(100, 100);
        assert!(fb.pixels.iter().all(|&p| p == 0));
    }

    #[test]
    fn framebuffer_clear_sky() {
        let mut fb = EmulatorFrameBuffer::new(100, 100);
        fb.clear_sky();
        // Top and bottom should be different colors (gradient)
        let top = fb.pixels[0];
        let bot = fb.pixels[99 * 100];
        assert_ne!(top, bot);
    }

    #[test]
    fn framebuffer_clear_color() {
        let mut fb = EmulatorFrameBuffer::new(100, 100);
        fb.clear_color(0xff0000);
        assert!(fb.pixels.iter().all(|&p| p == 0xff0000));
    }

    #[test]
    fn framebuffer_set_pixel_in_bounds() {
        let mut fb = EmulatorFrameBuffer::new(100, 100);
        fb.set_pixel(50, 50, 0xffffff);
        assert_eq!(fb.pixels[50 * 100 + 50], 0xffffff);
    }

    #[test]
    fn framebuffer_set_pixel_out_of_bounds_no_panic() {
        let mut fb = EmulatorFrameBuffer::new(100, 100);
        fb.set_pixel(-1, 0, 0xffffff);
        fb.set_pixel(0, -1, 0xffffff);
        fb.set_pixel(100, 0, 0xffffff);
        fb.set_pixel(0, 100, 0xffffff);
    }

    #[test]
    fn framebuffer_draw_stereo_divider() {
        let mut fb = EmulatorFrameBuffer::new(100, 100);
        fb.clear_color(0x000000);
        fb.draw_stereo_divider();
        // Middle column should be the divider color
        let mid = 50;
        for y in 0..100 {
            assert_eq!(fb.pixels[y * 100 + mid], STEREO_DIVIDER_COLOR);
        }
    }

    // ---- Character bitmap tests (delegated to aether-vr-overlay) ----

    #[test]
    fn char_bitmap_space_is_empty() {
        let bm = aether_vr_overlay::font::char_bitmap(' ');
        assert!(bm.iter().all(|&row| row == 0));
    }

    #[test]
    fn char_bitmap_a_is_nonempty() {
        let bm = aether_vr_overlay::font::char_bitmap('A');
        assert!(bm.iter().any(|&row| row != 0));
    }

    #[test]
    fn char_bitmap_digits_nonempty() {
        for digit in '0'..='9' {
            let bm = aether_vr_overlay::font::char_bitmap(digit);
            assert!(bm.iter().any(|&row| row != 0), "digit {digit} is empty");
        }
    }

    #[test]
    fn char_bitmap_unknown_char_fallback() {
        let bm = aether_vr_overlay::font::char_bitmap('\u{FFFF}');
        assert!(bm.iter().any(|&row| row != 0));
    }

    // ---- Debug overlay tests ----

    #[test]
    fn draw_debug_overlay_no_panic() {
        let mut fb = EmulatorFrameBuffer::new(400, 300);
        fb.clear_color(0x000000);
        let info = DebugOverlayInfo {
            fps: 90.0,
            frame_time_ms: 11.1,
            head_position: [0.0, 1.7, 0.0],
            head_yaw_deg: 45.0,
            head_pitch_deg: 10.0,
            left_controller_pos: [-0.2, 1.4, -0.4],
            right_controller_pos: [0.2, 1.4, -0.4],
            session_state: "Running".to_string(),
            frame_count: 1234,
        };
        draw_debug_overlay(&mut fb, &info);
        // Verify some pixels were modified (overlay drawn)
        assert!(fb.pixels.iter().any(|&p| p == OVERLAY_TEXT_COLOR));
    }

    #[test]
    fn draw_debug_overlay_small_framebuffer() {
        // Should not panic even with a tiny framebuffer
        let mut fb = EmulatorFrameBuffer::new(10, 10);
        fb.clear_color(0x000000);
        let info = DebugOverlayInfo {
            fps: 60.0,
            frame_time_ms: 16.6,
            head_position: [0.0, 1.7, 0.0],
            head_yaw_deg: 0.0,
            head_pitch_deg: 0.0,
            left_controller_pos: [0.0, 0.0, 0.0],
            right_controller_pos: [0.0, 0.0, 0.0],
            session_state: "Idle".to_string(),
            frame_count: 0,
        };
        draw_debug_overlay(&mut fb, &info);
    }

    // ---- Text rendering ----

    #[test]
    fn draw_text_renders_pixels() {
        let mut fb = EmulatorFrameBuffer::new(200, 50);
        fb.clear_color(0x000000);
        aether_vr_overlay::font::draw_text_u32(
            &mut fb.pixels,
            fb.width,
            fb.height,
            "Hello",
            10,
            10,
            0xffffff,
        );
        assert!(fb.pixels.iter().any(|&p| p == 0xffffff));
    }

    #[test]
    fn draw_text_empty_string_no_change() {
        let mut fb = EmulatorFrameBuffer::new(100, 50);
        fb.clear_color(0x000000);
        let before: Vec<u32> = fb.pixels.clone();
        aether_vr_overlay::font::draw_text_u32(
            &mut fb.pixels,
            fb.width,
            fb.height,
            "",
            10,
            10,
            0xffffff,
        );
        assert_eq!(fb.pixels, before);
    }

    // ---- ControllerInput default ----

    #[test]
    fn controller_input_default_all_false() {
        let input = ControllerInput::default();
        assert!(!input.left_stick_up);
        assert!(!input.left_stick_down);
        assert!(!input.left_stick_left);
        assert!(!input.left_stick_right);
        assert!(!input.left_trigger);
        assert!(!input.right_trigger);
        assert!(!input.left_grip);
        assert!(!input.right_grip);
        assert!(!input.button_a);
        assert!(!input.button_b);
        assert!(!input.button_x);
        assert!(!input.button_y);
        assert!(!input.button_menu);
        assert_eq!(input.right_aim_x, 0.0);
        assert_eq!(input.right_aim_y, 0.0);
    }

    // ---- Lerp ----

    #[test]
    fn lerp_u8_endpoints() {
        assert_eq!(lerp_u8(0, 255, 0.0), 0);
        assert_eq!(lerp_u8(0, 255, 1.0), 255);
    }

    #[test]
    fn lerp_u8_midpoint() {
        let result = lerp_u8(0, 200, 0.5);
        assert!((result as i32 - 100).abs() <= 1);
    }

    #[test]
    fn lerp_u8_clamped() {
        assert_eq!(lerp_u8(0, 255, 2.0), 255);
        assert_eq!(lerp_u8(255, 0, 2.0), 0);
    }
}
