//! Mini-map: small overview of the entire graph with viewport indicator.

use aether_creator_studio::visual_script::NodeGraph;
use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

use crate::canvas::ViewTransform;
use crate::node_renderer::node_bounds;

/// Width of the minimap in screen pixels.
const MINIMAP_WIDTH: f32 = 200.0;

/// Height of the minimap in screen pixels.
const MINIMAP_HEIGHT: f32 = 150.0;

/// Padding around the minimap content.
const MINIMAP_PADDING: f32 = 10.0;

/// Compute the bounding rectangle of all nodes in the graph (in canvas space).
pub fn graph_bounds(graph: &NodeGraph) -> Option<Rect> {
    let mut nodes_iter = graph.nodes();
    let first = nodes_iter.next()?;
    let mut bounds = node_bounds(first);

    for node in nodes_iter {
        bounds = bounds.union(node_bounds(node));
    }

    // Add some padding
    Some(bounds.expand(50.0))
}

/// Compute where the current viewport is within the graph bounds.
/// Returns the viewport rectangle in normalized coordinates (0..1, 0..1).
pub fn viewport_rect_normalized(
    view: &ViewTransform,
    canvas_viewport: Rect,
    graph_rect: Rect,
) -> Rect {
    let visible = view.visible_canvas_rect(canvas_viewport);

    let x_start = (visible.min.x - graph_rect.min.x) / graph_rect.width();
    let y_start = (visible.min.y - graph_rect.min.y) / graph_rect.height();
    let x_end = (visible.max.x - graph_rect.min.x) / graph_rect.width();
    let y_end = (visible.max.y - graph_rect.min.y) / graph_rect.height();

    Rect::from_min_max(
        Pos2::new(x_start.clamp(0.0, 1.0), y_start.clamp(0.0, 1.0)),
        Pos2::new(x_end.clamp(0.0, 1.0), y_end.clamp(0.0, 1.0)),
    )
}

/// Draw the minimap in the bottom-right corner of the canvas.
pub fn draw_minimap(
    painter: &Painter,
    graph: &NodeGraph,
    view: &ViewTransform,
    canvas_viewport: Rect,
) {
    let g_bounds = match graph_bounds(graph) {
        Some(b) => b,
        None => return,
    };

    // Minimap screen position (bottom-right corner)
    let minimap_rect = Rect::from_min_size(
        Pos2::new(
            canvas_viewport.max.x - MINIMAP_WIDTH - 10.0,
            canvas_viewport.max.y - MINIMAP_HEIGHT - 10.0,
        ),
        Vec2::new(MINIMAP_WIDTH, MINIMAP_HEIGHT),
    );

    // Background
    painter.rect_filled(
        minimap_rect,
        CornerRadius::same(4),
        Color32::from_rgba_premultiplied(30, 30, 35, 220),
    );
    painter.rect_stroke(
        minimap_rect,
        CornerRadius::same(4),
        Stroke::new(1.0, Color32::from_gray(80)),
        StrokeKind::Middle,
    );

    let content_rect = minimap_rect.shrink(MINIMAP_PADDING);

    // Draw node dots
    for node in graph.nodes() {
        let nx = (node.position.0 - g_bounds.min.x) / g_bounds.width();
        let ny = (node.position.1 - g_bounds.min.y) / g_bounds.height();

        let screen_x = content_rect.min.x + nx * content_rect.width();
        let screen_y = content_rect.min.y + ny * content_rect.height();

        let color = crate::state::EditorState::node_category_color(&node.kind);
        painter.circle_filled(Pos2::new(screen_x, screen_y), 3.0, color);
    }

    // Draw viewport indicator
    let vp_norm = viewport_rect_normalized(view, canvas_viewport, g_bounds);
    let vp_screen = Rect::from_min_max(
        Pos2::new(
            content_rect.min.x + vp_norm.min.x * content_rect.width(),
            content_rect.min.y + vp_norm.min.y * content_rect.height(),
        ),
        Pos2::new(
            content_rect.min.x + vp_norm.max.x * content_rect.width(),
            content_rect.min.y + vp_norm.max.y * content_rect.height(),
        ),
    );

    painter.rect_stroke(
        vp_screen,
        CornerRadius::ZERO,
        Stroke::new(1.5, Color32::from_rgb(100, 180, 255)),
        StrokeKind::Middle,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_creator_studio::visual_script::{NodeGraph, NodeKind};

    #[test]
    fn test_graph_bounds_empty() {
        let graph = NodeGraph::new("test", "Empty");
        assert!(graph_bounds(&graph).is_none());
    }

    #[test]
    fn test_graph_bounds_single_node() {
        let mut graph = NodeGraph::new("test", "Single");
        graph.add_node_at(NodeKind::OnStart, 100.0, 200.0).unwrap();
        let bounds = graph_bounds(&graph).unwrap();
        // Should contain the node position
        assert!(bounds.contains(Pos2::new(100.0, 200.0)));
    }

    #[test]
    fn test_graph_bounds_multiple_nodes() {
        let mut graph = NodeGraph::new("test", "Multi");
        graph.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
        graph.add_node_at(NodeKind::Log, 500.0, 300.0).unwrap();
        let bounds = graph_bounds(&graph).unwrap();
        assert!(bounds.contains(Pos2::new(0.0, 0.0)));
        assert!(bounds.contains(Pos2::new(500.0, 300.0)));
    }

    #[test]
    fn test_graph_bounds_includes_padding() {
        let mut graph = NodeGraph::new("test", "Padded");
        graph.add_node_at(NodeKind::OnStart, 100.0, 100.0).unwrap();
        let bounds = graph_bounds(&graph).unwrap();
        // With 50px padding, bounds should extend beyond node position
        assert!(bounds.min.x < 100.0);
        assert!(bounds.min.y < 100.0);
    }

    #[test]
    fn test_viewport_rect_normalized_full() {
        let mut graph = NodeGraph::new("test", "Test");
        graph.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
        graph.add_node_at(NodeKind::Log, 400.0, 300.0).unwrap();
        let g_bounds = graph_bounds(&graph).unwrap();

        // Viewport that encompasses everything
        let view = ViewTransform::new(
            egui::Vec2::new(g_bounds.min.x, g_bounds.min.y),
            1.0,
        );
        let canvas_vp = Rect::from_min_size(
            Pos2::new(0.0, 0.0),
            egui::Vec2::new(g_bounds.width(), g_bounds.height()),
        );

        let vp = viewport_rect_normalized(&view, canvas_vp, g_bounds);
        // Should cover roughly 0..1 in both dimensions
        assert!(vp.min.x <= 0.01);
        assert!(vp.min.y <= 0.01);
        assert!(vp.max.x >= 0.99);
        assert!(vp.max.y >= 0.99);
    }

    #[test]
    fn test_viewport_rect_normalized_zoomed_in() {
        let g_bounds = Rect::from_min_max(
            Pos2::new(0.0, 0.0),
            Pos2::new(1000.0, 800.0),
        );

        // Zoomed in: viewing only center quarter
        let view = ViewTransform::new(
            egui::Vec2::new(250.0, 200.0),
            2.0, // 2x zoom
        );
        let canvas_vp = Rect::from_min_size(
            Pos2::new(0.0, 0.0),
            egui::Vec2::new(1000.0, 800.0),
        );

        let vp = viewport_rect_normalized(&view, canvas_vp, g_bounds);
        // At 2x zoom, visible area is 500x400 canvas units, starting at (250, 200)
        // So normalized: (0.25, 0.25) to (0.75, 0.75)
        assert!((vp.min.x - 0.25).abs() < 0.01);
        assert!((vp.min.y - 0.25).abs() < 0.01);
        assert!((vp.max.x - 0.75).abs() < 0.01);
        assert!((vp.max.y - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_viewport_rect_clamped() {
        let g_bounds = Rect::from_min_max(
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 100.0),
        );

        // Pan way outside
        let view = ViewTransform::new(
            egui::Vec2::new(-500.0, -500.0),
            1.0,
        );
        let canvas_vp = Rect::from_min_size(
            Pos2::new(0.0, 0.0),
            egui::Vec2::new(200.0, 200.0),
        );

        let vp = viewport_rect_normalized(&view, canvas_vp, g_bounds);
        // Values should be clamped to 0..1
        assert!(vp.min.x >= 0.0);
        assert!(vp.min.y >= 0.0);
        assert!(vp.max.x <= 1.0);
        assert!(vp.max.y <= 1.0);
    }
}
