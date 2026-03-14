//! Canvas viewport with pan/zoom coordinate transforms and grid rendering.

use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};

/// Minimum allowed zoom level.
const MIN_ZOOM: f32 = 0.1;

/// Maximum allowed zoom level.
const MAX_ZOOM: f32 = 5.0;

/// Small grid cell size in canvas units.
const GRID_CELL_SMALL: f32 = 25.0;

/// Large grid cell size in canvas units.
const GRID_CELL_LARGE: f32 = 100.0;

/// Default snap grid size.
const SNAP_GRID_SIZE: f32 = 25.0;

/// Manages the view transform between screen space and canvas (world) space.
#[derive(Debug, Clone)]
pub struct ViewTransform {
    /// Pan offset in canvas units (the canvas coordinate at the top-left of the viewport).
    pub pan: Vec2,
    /// Zoom level. 1.0 = 100%.
    pub zoom: f32,
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self {
            pan: Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

impl ViewTransform {
    /// Create a new view transform with given pan and zoom.
    pub fn new(pan: Vec2, zoom: f32) -> Self {
        Self {
            pan,
            zoom: zoom.clamp(MIN_ZOOM, MAX_ZOOM),
        }
    }

    /// Convert a screen-space position to canvas-space.
    /// `viewport_origin` is the top-left corner of the canvas widget in screen coords.
    pub fn screen_to_canvas(&self, screen_pos: Pos2, viewport_origin: Pos2) -> Pos2 {
        let relative = screen_pos - viewport_origin;
        Pos2::new(
            relative.x / self.zoom + self.pan.x,
            relative.y / self.zoom + self.pan.y,
        )
    }

    /// Convert a canvas-space position to screen-space.
    /// `viewport_origin` is the top-left corner of the canvas widget in screen coords.
    pub fn canvas_to_screen(&self, canvas_pos: Pos2, viewport_origin: Pos2) -> Pos2 {
        Pos2::new(
            (canvas_pos.x - self.pan.x) * self.zoom + viewport_origin.x,
            (canvas_pos.y - self.pan.y) * self.zoom + viewport_origin.y,
        )
    }

    /// Apply a zoom delta centered on a screen position.
    pub fn zoom_at(&mut self, delta: f32, screen_pos: Pos2, viewport_origin: Pos2) {
        let canvas_before = self.screen_to_canvas(screen_pos, viewport_origin);
        self.zoom = (self.zoom * (1.0 + delta)).clamp(MIN_ZOOM, MAX_ZOOM);
        let canvas_after = self.screen_to_canvas(screen_pos, viewport_origin);
        self.pan.x += canvas_before.x - canvas_after.x;
        self.pan.y += canvas_before.y - canvas_after.y;
    }

    /// Apply a pan delta in screen pixels.
    pub fn pan_by(&mut self, screen_delta: Vec2) {
        self.pan.x -= screen_delta.x / self.zoom;
        self.pan.y -= screen_delta.y / self.zoom;
    }

    /// Get the visible canvas-space rectangle for a given screen viewport.
    pub fn visible_canvas_rect(&self, viewport: Rect) -> Rect {
        let top_left = self.screen_to_canvas(viewport.min, viewport.min);
        let bottom_right = self.screen_to_canvas(viewport.max, viewport.min);
        Rect::from_min_max(top_left, bottom_right)
    }

    /// Scale a distance from canvas units to screen pixels.
    pub fn canvas_to_screen_dist(&self, canvas_dist: f32) -> f32 {
        canvas_dist * self.zoom
    }
}

/// Snap a canvas-space position to the nearest grid point.
pub fn snap_to_grid(pos: Pos2) -> Pos2 {
    Pos2::new(
        (pos.x / SNAP_GRID_SIZE).round() * SNAP_GRID_SIZE,
        (pos.y / SNAP_GRID_SIZE).round() * SNAP_GRID_SIZE,
    )
}

/// Draw the grid background on the canvas.
pub fn draw_grid(painter: &Painter, view: &ViewTransform, viewport: Rect) {
    let canvas_rect = view.visible_canvas_rect(viewport);

    // Small grid (visible at zoom > 0.5)
    if view.zoom > 0.5 {
        draw_grid_lines(
            painter,
            view,
            viewport,
            &canvas_rect,
            GRID_CELL_SMALL,
            Color32::from_gray(45),
        );
    }

    // Large grid (always visible)
    draw_grid_lines(
        painter,
        view,
        viewport,
        &canvas_rect,
        GRID_CELL_LARGE,
        Color32::from_gray(60),
    );

    // Origin axes
    let origin_screen = view.canvas_to_screen(Pos2::ZERO, viewport.min);
    if viewport.contains(Pos2::new(origin_screen.x, viewport.center().y)) {
        painter.line_segment(
            [
                Pos2::new(origin_screen.x, viewport.min.y),
                Pos2::new(origin_screen.x, viewport.max.y),
            ],
            Stroke::new(1.0, Color32::from_gray(80)),
        );
    }
    if viewport.contains(Pos2::new(viewport.center().x, origin_screen.y)) {
        painter.line_segment(
            [
                Pos2::new(viewport.min.x, origin_screen.y),
                Pos2::new(viewport.max.x, origin_screen.y),
            ],
            Stroke::new(1.0, Color32::from_gray(80)),
        );
    }
}

fn draw_grid_lines(
    painter: &Painter,
    view: &ViewTransform,
    viewport: Rect,
    canvas_rect: &Rect,
    cell_size: f32,
    color: Color32,
) {
    let stroke = Stroke::new(1.0, color);

    let start_x = (canvas_rect.min.x / cell_size).floor() as i64;
    let end_x = (canvas_rect.max.x / cell_size).ceil() as i64;
    let start_y = (canvas_rect.min.y / cell_size).floor() as i64;
    let end_y = (canvas_rect.max.y / cell_size).ceil() as i64;

    for ix in start_x..=end_x {
        let cx = ix as f32 * cell_size;
        let screen_x = view.canvas_to_screen(Pos2::new(cx, 0.0), viewport.min).x;
        painter.line_segment(
            [
                Pos2::new(screen_x, viewport.min.y),
                Pos2::new(screen_x, viewport.max.y),
            ],
            stroke,
        );
    }

    for iy in start_y..=end_y {
        let cy = iy as f32 * cell_size;
        let screen_y = view.canvas_to_screen(Pos2::new(0.0, cy), viewport.min).y;
        painter.line_segment(
            [
                Pos2::new(viewport.min.x, screen_y),
                Pos2::new(viewport.max.x, screen_y),
            ],
            stroke,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: Pos2 = Pos2::new(0.0, 0.0);

    #[test]
    fn test_default_view_transform() {
        let vt = ViewTransform::default();
        assert_eq!(vt.pan, Vec2::ZERO);
        assert_eq!(vt.zoom, 1.0);
    }

    #[test]
    fn test_screen_to_canvas_identity() {
        let vt = ViewTransform::default();
        let screen = Pos2::new(100.0, 200.0);
        let canvas = vt.screen_to_canvas(screen, ORIGIN);
        assert!((canvas.x - 100.0).abs() < 1e-5);
        assert!((canvas.y - 200.0).abs() < 1e-5);
    }

    #[test]
    fn test_canvas_to_screen_identity() {
        let vt = ViewTransform::default();
        let canvas = Pos2::new(100.0, 200.0);
        let screen = vt.canvas_to_screen(canvas, ORIGIN);
        assert!((screen.x - 100.0).abs() < 1e-5);
        assert!((screen.y - 200.0).abs() < 1e-5);
    }

    #[test]
    fn test_round_trip_screen_canvas() {
        let vt = ViewTransform::new(Vec2::new(50.0, -30.0), 1.5);
        let origin = Pos2::new(10.0, 20.0);
        let screen = Pos2::new(150.0, 250.0);

        let canvas = vt.screen_to_canvas(screen, origin);
        let back = vt.canvas_to_screen(canvas, origin);

        assert!((back.x - screen.x).abs() < 1e-3, "x: {} vs {}", back.x, screen.x);
        assert!((back.y - screen.y).abs() < 1e-3, "y: {} vs {}", back.y, screen.y);
    }

    #[test]
    fn test_round_trip_canvas_screen() {
        let vt = ViewTransform::new(Vec2::new(-100.0, 200.0), 2.0);
        let origin = Pos2::new(5.0, 5.0);
        let canvas = Pos2::new(300.0, 400.0);

        let screen = vt.canvas_to_screen(canvas, origin);
        let back = vt.screen_to_canvas(screen, origin);

        assert!((back.x - canvas.x).abs() < 1e-3);
        assert!((back.y - canvas.y).abs() < 1e-3);
    }

    #[test]
    fn test_zoom_clamped_min() {
        let vt = ViewTransform::new(Vec2::ZERO, 0.01);
        assert!((vt.zoom - MIN_ZOOM).abs() < 1e-5);
    }

    #[test]
    fn test_zoom_clamped_max() {
        let vt = ViewTransform::new(Vec2::ZERO, 100.0);
        assert!((vt.zoom - MAX_ZOOM).abs() < 1e-5);
    }

    #[test]
    fn test_pan_by() {
        let mut vt = ViewTransform::default();
        vt.pan_by(Vec2::new(100.0, 50.0));
        assert!((vt.pan.x - (-100.0)).abs() < 1e-5);
        assert!((vt.pan.y - (-50.0)).abs() < 1e-5);
    }

    #[test]
    fn test_pan_by_with_zoom() {
        let mut vt = ViewTransform::new(Vec2::ZERO, 2.0);
        vt.pan_by(Vec2::new(100.0, 0.0));
        // pan delta in canvas = -100 / 2.0 = -50
        assert!((vt.pan.x - (-50.0)).abs() < 1e-5);
    }

    #[test]
    fn test_zoom_at_preserves_point() {
        let mut vt = ViewTransform::default();
        let origin = Pos2::new(0.0, 0.0);
        let focus = Pos2::new(200.0, 300.0);

        let canvas_before = vt.screen_to_canvas(focus, origin);
        vt.zoom_at(0.5, focus, origin);
        let canvas_after = vt.screen_to_canvas(focus, origin);

        assert!(
            (canvas_before.x - canvas_after.x).abs() < 1e-2,
            "canvas x should be preserved"
        );
        assert!(
            (canvas_before.y - canvas_after.y).abs() < 1e-2,
            "canvas y should be preserved"
        );
    }

    #[test]
    fn test_visible_canvas_rect() {
        let vt = ViewTransform::new(Vec2::new(100.0, 100.0), 1.0);
        let viewport = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(800.0, 600.0));
        let canvas_rect = vt.visible_canvas_rect(viewport);

        assert!((canvas_rect.min.x - 100.0).abs() < 1e-3);
        assert!((canvas_rect.min.y - 100.0).abs() < 1e-3);
        assert!((canvas_rect.max.x - 900.0).abs() < 1e-3);
        assert!((canvas_rect.max.y - 700.0).abs() < 1e-3);
    }

    #[test]
    fn test_visible_canvas_rect_zoomed() {
        let vt = ViewTransform::new(Vec2::ZERO, 2.0);
        let viewport = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(800.0, 600.0));
        let canvas_rect = vt.visible_canvas_rect(viewport);

        // At 2x zoom, visible area is half the viewport in canvas units
        assert!((canvas_rect.width() - 400.0).abs() < 1e-3);
        assert!((canvas_rect.height() - 300.0).abs() < 1e-3);
    }

    #[test]
    fn test_canvas_to_screen_dist() {
        let vt = ViewTransform::new(Vec2::ZERO, 2.0);
        assert!((vt.canvas_to_screen_dist(50.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn test_snap_to_grid_exact() {
        let pos = Pos2::new(75.0, 100.0);
        let snapped = snap_to_grid(pos);
        assert!((snapped.x - 75.0).abs() < 1e-5);
        assert!((snapped.y - 100.0).abs() < 1e-5);
    }

    #[test]
    fn test_snap_to_grid_rounds() {
        let pos = Pos2::new(63.0, 88.0);
        let snapped = snap_to_grid(pos);
        // 63 / 25 = 2.52, rounds to 3 -> 75
        assert!((snapped.x - 75.0).abs() < 1e-5);
        // 88 / 25 = 3.52, rounds to 4 -> 100
        assert!((snapped.y - 100.0).abs() < 1e-5);
    }

    #[test]
    fn test_snap_to_grid_negative() {
        let pos = Pos2::new(-13.0, -38.0);
        let snapped = snap_to_grid(pos);
        // -13 / 25 = -0.52, rounds to -1 -> -25
        assert!((snapped.x - (-25.0)).abs() < 1e-5);
        // -38 / 25 = -1.52, rounds to -2 -> -50
        assert!((snapped.y - (-50.0)).abs() < 1e-5);
    }

    #[test]
    fn test_screen_to_canvas_with_offset_origin() {
        let vt = ViewTransform::default();
        let origin = Pos2::new(200.0, 100.0);
        let screen = Pos2::new(300.0, 250.0);
        let canvas = vt.screen_to_canvas(screen, origin);
        // (300 - 200) / 1.0 + 0 = 100
        assert!((canvas.x - 100.0).abs() < 1e-5);
        // (250 - 100) / 1.0 + 0 = 150
        assert!((canvas.y - 150.0).abs() < 1e-5);
    }

    #[test]
    fn test_canvas_to_screen_with_pan() {
        let vt = ViewTransform::new(Vec2::new(50.0, 50.0), 1.0);
        let origin = Pos2::new(0.0, 0.0);
        let canvas = Pos2::new(150.0, 200.0);
        let screen = vt.canvas_to_screen(canvas, origin);
        // (150 - 50) * 1.0 + 0 = 100
        assert!((screen.x - 100.0).abs() < 1e-5);
        // (200 - 50) * 1.0 + 0 = 150
        assert!((screen.y - 150.0).abs() < 1e-5);
    }
}
