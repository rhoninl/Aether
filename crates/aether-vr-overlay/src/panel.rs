//! Overlay panel that renders debug data to an RGBA pixel buffer.

use crate::config::OverlayConfig;
use crate::font;
use crate::layout::DebugOverlayData;

pub const DEFAULT_PANEL_WIDTH: usize = 512;
pub const DEFAULT_PANEL_HEIGHT: usize = 256;
pub const DEFAULT_TEXT_COLOR: [u8; 4] = [0, 255, 0, 255];
pub const DEFAULT_BG_COLOR: [u8; 4] = [10, 10, 30, 200];
pub const DEFAULT_TEXT_SCALE: usize = 2;
pub const DEFAULT_PADDING: usize = 8;

/// A renderable debug overlay panel.
pub struct OverlayPanel {
    width: usize,
    height: usize,
    buffer: Vec<u8>,
    text_color: [u8; 4],
    bg_color: [u8; 4],
    text_scale: usize,
    padding: usize,
    visible: bool,
}

impl OverlayPanel {
    /// Create a new overlay panel with default settings.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            buffer: vec![0u8; width * height * 4],
            text_color: DEFAULT_TEXT_COLOR,
            bg_color: DEFAULT_BG_COLOR,
            text_scale: DEFAULT_TEXT_SCALE,
            padding: DEFAULT_PADDING,
            visible: true,
        }
    }

    /// Create from configuration.
    pub fn from_config(config: &OverlayConfig) -> Self {
        let mut panel = Self::new(config.width, config.height);
        panel.text_scale = config.text_scale;
        panel.visible = config.initially_visible;
        panel
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Render debug data into the RGBA buffer.
    pub fn render(&mut self, data: &DebugOverlayData) {
        if !self.visible {
            return;
        }

        // Fill background
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = (y * self.width + x) * 4;
                self.buffer[idx] = self.bg_color[0];
                self.buffer[idx + 1] = self.bg_color[1];
                self.buffer[idx + 2] = self.bg_color[2];
                self.buffer[idx + 3] = self.bg_color[3];
            }
        }

        // Render text lines
        let lines = data.format_lines();
        let line_height = font::LINE_HEIGHT * self.text_scale;

        let mut params = font::DrawParams {
            buffer: &mut self.buffer,
            width: self.width,
            height: self.height,
            color: self.text_color,
            scale: self.text_scale,
        };

        for (i, line) in lines.iter().enumerate() {
            let y = self.padding as i32 + (i * line_height) as i32;
            font::draw_text_rgba(&mut params, line, self.padding as i32, y);
        }
    }

    /// Get the RGBA buffer contents.
    pub fn rgba_buffer(&self) -> &[u8] {
        &self.buffer
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_panel_dimensions() {
        let panel = OverlayPanel::new(512, 256);
        assert_eq!(panel.width(), 512);
        assert_eq!(panel.height(), 256);
    }

    #[test]
    fn new_panel_buffer_size() {
        let panel = OverlayPanel::new(512, 256);
        assert_eq!(panel.rgba_buffer().len(), 512 * 256 * 4);
    }

    #[test]
    fn new_panel_visible_by_default() {
        let panel = OverlayPanel::new(64, 64);
        assert!(panel.is_visible());
    }

    #[test]
    fn toggle_visibility() {
        let mut panel = OverlayPanel::new(64, 64);
        assert!(panel.is_visible());
        panel.toggle();
        assert!(!panel.is_visible());
        panel.toggle();
        assert!(panel.is_visible());
    }

    #[test]
    fn set_visible() {
        let mut panel = OverlayPanel::new(64, 64);
        panel.set_visible(false);
        assert!(!panel.is_visible());
        panel.set_visible(true);
        assert!(panel.is_visible());
    }

    #[test]
    fn render_fills_background() {
        let mut panel = OverlayPanel::new(64, 64);
        let data = DebugOverlayData::default();
        panel.render(&data);
        // Check that background color is present
        let buf = panel.rgba_buffer();
        // First pixel should be bg color
        assert_eq!(buf[0], DEFAULT_BG_COLOR[0]);
        assert_eq!(buf[1], DEFAULT_BG_COLOR[1]);
        assert_eq!(buf[2], DEFAULT_BG_COLOR[2]);
        assert_eq!(buf[3], DEFAULT_BG_COLOR[3]);
    }

    #[test]
    fn render_produces_text_pixels() {
        let mut panel = OverlayPanel::new(512, 256);
        let mut data = DebugOverlayData::default();
        data.fps = 90.0;
        panel.render(&data);
        let buf = panel.rgba_buffer();
        // Some pixels should be the text color (green)
        let has_green = buf
            .chunks(4)
            .any(|p| p[0] == DEFAULT_TEXT_COLOR[0] && p[1] == DEFAULT_TEXT_COLOR[1]);
        assert!(has_green, "no text pixels found after render");
    }

    #[test]
    fn render_when_invisible_does_not_modify() {
        let mut panel = OverlayPanel::new(64, 64);
        panel.set_visible(false);
        let before = panel.rgba_buffer().to_vec();
        let data = DebugOverlayData::default();
        panel.render(&data);
        assert_eq!(panel.rgba_buffer(), before.as_slice());
    }

    #[test]
    fn render_multiple_times_no_panic() {
        let mut panel = OverlayPanel::new(256, 128);
        let data = DebugOverlayData::default();
        for _ in 0..10 {
            panel.render(&data);
        }
    }

    #[test]
    fn from_config() {
        let config = OverlayConfig {
            width: 256,
            height: 128,
            text_scale: 3,
            initially_visible: false,
        };
        let panel = OverlayPanel::from_config(&config);
        assert_eq!(panel.width(), 256);
        assert_eq!(panel.height(), 128);
        assert!(!panel.is_visible());
    }

    #[test]
    fn small_panel_no_panic() {
        let mut panel = OverlayPanel::new(16, 16);
        let data = DebugOverlayData::default();
        panel.render(&data);
    }
}
