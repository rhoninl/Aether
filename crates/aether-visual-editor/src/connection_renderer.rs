//! Connection rendering: draws bezier curves between connected ports.

use aether_creator_studio::visual_script::{Connection, DataType, NodeGraph};
use egui::{Color32, Painter, Pos2, Stroke};

use crate::canvas::ViewTransform;
use crate::node_renderer::port_screen_pos;
use crate::state::EditorState;

/// Thickness for flow connections in screen pixels.
const FLOW_CONNECTION_THICKNESS: f32 = 3.0;

/// Thickness for data connections in screen pixels.
const DATA_CONNECTION_THICKNESS: f32 = 2.0;

/// Number of segments for bezier curve approximation.
const BEZIER_SEGMENTS: usize = 20;

/// Minimum tangent length for bezier curves.
const MIN_TANGENT_LENGTH: f32 = 50.0;

/// Draw all connections in the graph.
pub fn draw_connections(
    painter: &Painter,
    graph: &NodeGraph,
    view: &ViewTransform,
    viewport_origin: Pos2,
) {
    for conn in graph.connections() {
        draw_connection(painter, graph, conn, view, viewport_origin);
    }
}

/// Draw a single connection as a bezier curve.
fn draw_connection(
    painter: &Painter,
    graph: &NodeGraph,
    conn: &Connection,
    view: &ViewTransform,
    viewport_origin: Pos2,
) {
    let from_node = match graph.get_node(conn.from_node) {
        Some(n) => n,
        None => return,
    };
    let to_node = match graph.get_node(conn.to_node) {
        Some(n) => n,
        None => return,
    };

    let from_screen = match port_screen_pos(from_node, conn.from_port, view, viewport_origin) {
        Some(p) => p,
        None => return,
    };
    let to_screen = match port_screen_pos(to_node, conn.to_port, view, viewport_origin) {
        Some(p) => p,
        None => return,
    };

    let data_type = graph.port_data_type(conn.from_port).unwrap_or(DataType::Any);
    let color = EditorState::data_type_color(data_type);
    let thickness = if data_type == DataType::Flow {
        FLOW_CONNECTION_THICKNESS
    } else {
        DATA_CONNECTION_THICKNESS
    };

    draw_bezier_connection(painter, from_screen, to_screen, color, thickness);
}

/// Draw a pending connection (while user is dragging from a port).
pub fn draw_pending_connection(
    painter: &Painter,
    from_screen: Pos2,
    to_screen: Pos2,
    data_type: DataType,
) {
    let color = EditorState::data_type_color(data_type);
    let thickness = if data_type == DataType::Flow {
        FLOW_CONNECTION_THICKNESS
    } else {
        DATA_CONNECTION_THICKNESS
    };
    let alpha_color = Color32::from_rgba_premultiplied(
        color.r(),
        color.g(),
        color.b(),
        180,
    );
    draw_bezier_connection(painter, from_screen, to_screen, alpha_color, thickness);
}

/// Draw a cubic bezier curve between two points.
/// The curve bulges horizontally to look like typical node editor connections.
fn draw_bezier_connection(
    painter: &Painter,
    from: Pos2,
    to: Pos2,
    color: Color32,
    thickness: f32,
) {
    let dx = (to.x - from.x).abs();
    let tangent_len = (dx * 0.5).max(MIN_TANGENT_LENGTH);

    let cp1 = Pos2::new(from.x + tangent_len, from.y);
    let cp2 = Pos2::new(to.x - tangent_len, to.y);

    let mut points = Vec::with_capacity(BEZIER_SEGMENTS + 1);
    for i in 0..=BEZIER_SEGMENTS {
        let t = i as f32 / BEZIER_SEGMENTS as f32;
        let point = cubic_bezier(from, cp1, cp2, to, t);
        points.push(point);
    }

    let stroke = Stroke::new(thickness, color);
    for i in 0..points.len() - 1 {
        painter.line_segment([points[i], points[i + 1]], stroke);
    }
}

/// Evaluate a cubic bezier curve at parameter t.
fn cubic_bezier(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, t: f32) -> Pos2 {
    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let uuu = uu * u;
    let ttt = tt * t;

    Pos2::new(
        uuu * p0.x + 3.0 * uu * t * p1.x + 3.0 * u * tt * p2.x + ttt * p3.x,
        uuu * p0.y + 3.0 * uu * t * p1.y + 3.0 * u * tt * p2.y + ttt * p3.y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cubic_bezier_start() {
        let p0 = Pos2::new(0.0, 0.0);
        let p1 = Pos2::new(50.0, 0.0);
        let p2 = Pos2::new(150.0, 100.0);
        let p3 = Pos2::new(200.0, 100.0);
        let result = cubic_bezier(p0, p1, p2, p3, 0.0);
        assert!((result.x - p0.x).abs() < 1e-5);
        assert!((result.y - p0.y).abs() < 1e-5);
    }

    #[test]
    fn test_cubic_bezier_end() {
        let p0 = Pos2::new(0.0, 0.0);
        let p1 = Pos2::new(50.0, 0.0);
        let p2 = Pos2::new(150.0, 100.0);
        let p3 = Pos2::new(200.0, 100.0);
        let result = cubic_bezier(p0, p1, p2, p3, 1.0);
        assert!((result.x - p3.x).abs() < 1e-3);
        assert!((result.y - p3.y).abs() < 1e-3);
    }

    #[test]
    fn test_cubic_bezier_midpoint() {
        let p0 = Pos2::new(0.0, 0.0);
        let p1 = Pos2::new(0.0, 0.0);
        let p2 = Pos2::new(100.0, 100.0);
        let p3 = Pos2::new(100.0, 100.0);
        let result = cubic_bezier(p0, p1, p2, p3, 0.5);
        assert!((result.x - 50.0).abs() < 1e-3);
        assert!((result.y - 50.0).abs() < 1e-3);
    }

    #[test]
    fn test_cubic_bezier_straight_line() {
        let p0 = Pos2::new(0.0, 0.0);
        let p1 = Pos2::new(33.33, 0.0);
        let p2 = Pos2::new(66.67, 0.0);
        let p3 = Pos2::new(100.0, 0.0);
        // A straight horizontal line; all y should be 0
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let pt = cubic_bezier(p0, p1, p2, p3, t);
            assert!((pt.y).abs() < 1e-3, "y should be ~0 at t={t}, got {}", pt.y);
        }
    }

    #[test]
    fn test_flow_connection_thickness() {
        assert!(FLOW_CONNECTION_THICKNESS > DATA_CONNECTION_THICKNESS);
    }
}
