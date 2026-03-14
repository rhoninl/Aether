//! Node rendering: draws node boxes, title bars, and ports on the canvas.

use aether_creator_studio::visual_script::{Node, NodeId, PortDirection, PortId};
use egui::{Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

use crate::canvas::ViewTransform;
use crate::state::EditorState;

/// Width of a node box in canvas units.
pub const NODE_WIDTH: f32 = 180.0;

/// Height of the title bar in canvas units.
pub const TITLE_BAR_HEIGHT: f32 = 24.0;

/// Height of each port row in canvas units.
pub const PORT_ROW_HEIGHT: f32 = 22.0;

/// Radius of port circles in canvas units.
pub const PORT_RADIUS: f32 = 5.0;

/// Horizontal inset for port circles from node edge.
const PORT_INSET: f32 = 0.0;

/// Padding inside the node box.
const NODE_PADDING: f32 = 6.0;

/// Calculate the bounding rectangle of a node in canvas space.
pub fn node_bounds(node: &Node) -> Rect {
    let port_count = node.inputs.len().max(node.outputs.len());
    let height = TITLE_BAR_HEIGHT + port_count as f32 * PORT_ROW_HEIGHT + NODE_PADDING;
    Rect::from_min_size(
        Pos2::new(node.position.0, node.position.1),
        Vec2::new(NODE_WIDTH, height),
    )
}

/// Calculate the screen-space position of a port circle center.
pub fn port_screen_pos(
    node: &Node,
    port_id: PortId,
    view: &ViewTransform,
    viewport_origin: Pos2,
) -> Option<Pos2> {
    // Find port and its index
    if let Some((index, _port)) = node
        .inputs
        .iter()
        .enumerate()
        .find(|(_, p)| p.id == port_id)
    {
        let canvas_pos = input_port_canvas_pos(node, index);
        return Some(view.canvas_to_screen(canvas_pos, viewport_origin));
    }
    if let Some((index, _port)) = node
        .outputs
        .iter()
        .enumerate()
        .find(|(_, p)| p.id == port_id)
    {
        let canvas_pos = output_port_canvas_pos(node, index);
        return Some(view.canvas_to_screen(canvas_pos, viewport_origin));
    }
    None
}

/// Canvas position of an input port by index.
pub fn input_port_canvas_pos(node: &Node, index: usize) -> Pos2 {
    Pos2::new(
        node.position.0 + PORT_INSET,
        node.position.1 + TITLE_BAR_HEIGHT + (index as f32 + 0.5) * PORT_ROW_HEIGHT,
    )
}

/// Canvas position of an output port by index.
pub fn output_port_canvas_pos(node: &Node, index: usize) -> Pos2 {
    Pos2::new(
        node.position.0 + NODE_WIDTH - PORT_INSET,
        node.position.1 + TITLE_BAR_HEIGHT + (index as f32 + 0.5) * PORT_ROW_HEIGHT,
    )
}

/// Hit-test: find which node (if any) is at the given canvas position.
pub fn hit_test_node<'a>(
    nodes: impl Iterator<Item = &'a Node>,
    canvas_pos: Pos2,
) -> Option<NodeId> {
    for node in nodes {
        if node_bounds(node).contains(canvas_pos) {
            return Some(node.id);
        }
    }
    None
}

/// Hit-test: find which port (if any) is near the given canvas position.
/// Returns (node_id, port_id, direction).
pub fn hit_test_port(
    node: &Node,
    canvas_pos: Pos2,
    hit_radius: f32,
) -> Option<(NodeId, PortId, PortDirection)> {
    for (i, port) in node.inputs.iter().enumerate() {
        let port_pos = input_port_canvas_pos(node, i);
        if (port_pos - canvas_pos).length() <= hit_radius {
            return Some((node.id, port.id, PortDirection::Input));
        }
    }
    for (i, port) in node.outputs.iter().enumerate() {
        let port_pos = output_port_canvas_pos(node, i);
        if (port_pos - canvas_pos).length() <= hit_radius {
            return Some((node.id, port.id, PortDirection::Output));
        }
    }
    None
}

/// Draw a single node on the canvas.
pub fn draw_node(
    painter: &Painter,
    node: &Node,
    view: &ViewTransform,
    viewport_origin: Pos2,
    is_selected: bool,
) {
    let bounds = node_bounds(node);
    let screen_min = view.canvas_to_screen(bounds.min, viewport_origin);
    let screen_max = view.canvas_to_screen(bounds.max, viewport_origin);
    let screen_rect = Rect::from_min_max(screen_min, screen_max);

    // Skip if entirely off screen
    let clip_rect = painter.clip_rect();
    if !clip_rect.intersects(screen_rect) {
        return;
    }

    let rounding = CornerRadius::same(view.canvas_to_screen_dist(4.0) as u8);

    // Node background
    painter.rect_filled(screen_rect, rounding, Color32::from_rgb(40, 40, 45));

    // Title bar
    let title_bar_screen_bottom =
        view.canvas_to_screen(Pos2::new(bounds.min.x, bounds.min.y + TITLE_BAR_HEIGHT), viewport_origin);
    let title_rect = Rect::from_min_max(screen_min, Pos2::new(screen_max.x, title_bar_screen_bottom.y));
    let category_color = EditorState::node_category_color(&node.kind);
    painter.rect_filled(title_rect, rounding, category_color);
    // Flatten bottom corners of title bar
    let title_bottom_rect = Rect::from_min_max(
        Pos2::new(screen_min.x, title_bar_screen_bottom.y - view.canvas_to_screen_dist(4.0)),
        Pos2::new(screen_max.x, title_bar_screen_bottom.y),
    );
    painter.rect_filled(title_bottom_rect, CornerRadius::ZERO, category_color);

    // Title text
    let font_size = view.canvas_to_screen_dist(12.0).max(6.0);
    let title_pos = Pos2::new(
        screen_min.x + view.canvas_to_screen_dist(8.0),
        screen_min.y + view.canvas_to_screen_dist(5.0),
    );
    painter.text(
        title_pos,
        egui::Align2::LEFT_TOP,
        node.display_name(),
        FontId::proportional(font_size),
        Color32::WHITE,
    );

    // Ports
    let port_font_size = view.canvas_to_screen_dist(10.0).max(5.0);
    let port_radius_screen = view.canvas_to_screen_dist(PORT_RADIUS);

    // Input ports
    for (i, port) in node.inputs.iter().enumerate() {
        let canvas_pos = input_port_canvas_pos(node, i);
        let screen_pos = view.canvas_to_screen(canvas_pos, viewport_origin);
        let color = EditorState::data_type_color(port.data_type);

        // Port circle
        if port.data_type == aether_creator_studio::visual_script::DataType::Flow {
            // Flow ports as triangles
            draw_flow_port(painter, screen_pos, port_radius_screen, color);
        } else {
            painter.circle_filled(screen_pos, port_radius_screen, color);
        }

        // Port label
        let label_pos = Pos2::new(
            screen_pos.x + view.canvas_to_screen_dist(10.0),
            screen_pos.y,
        );
        painter.text(
            label_pos,
            egui::Align2::LEFT_CENTER,
            &port.name,
            FontId::proportional(port_font_size),
            Color32::from_gray(200),
        );
    }

    // Output ports
    for (i, port) in node.outputs.iter().enumerate() {
        let canvas_pos = output_port_canvas_pos(node, i);
        let screen_pos = view.canvas_to_screen(canvas_pos, viewport_origin);
        let color = EditorState::data_type_color(port.data_type);

        if port.data_type == aether_creator_studio::visual_script::DataType::Flow {
            draw_flow_port(painter, screen_pos, port_radius_screen, color);
        } else {
            painter.circle_filled(screen_pos, port_radius_screen, color);
        }

        // Port label (right-aligned)
        let label_pos = Pos2::new(
            screen_pos.x - view.canvas_to_screen_dist(10.0),
            screen_pos.y,
        );
        painter.text(
            label_pos,
            egui::Align2::RIGHT_CENTER,
            &port.name,
            FontId::proportional(port_font_size),
            Color32::from_gray(200),
        );
    }

    // Selection outline
    if is_selected {
        painter.rect_stroke(
            screen_rect,
            rounding,
            Stroke::new(view.canvas_to_screen_dist(2.0), Color32::from_rgb(100, 180, 255)),
            StrokeKind::Outside,
        );
    }

    // Border
    painter.rect_stroke(
        screen_rect,
        rounding,
        Stroke::new(1.0, Color32::from_gray(70)),
        StrokeKind::Middle,
    );
}

/// Draw a flow port as a right-pointing triangle.
fn draw_flow_port(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    let r = radius * 1.2;
    let points = vec![
        Pos2::new(center.x - r * 0.7, center.y - r),
        Pos2::new(center.x + r, center.y),
        Pos2::new(center.x - r * 0.7, center.y + r),
    ];
    painter.add(egui::Shape::convex_polygon(
        points,
        color,
        Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_creator_studio::visual_script::{Node, NodeKind};

    fn make_add_node() -> Node {
        Node::new(1, NodeKind::Add, 100).with_position(50.0, 50.0)
    }

    fn make_branch_node() -> Node {
        Node::new(2, NodeKind::Branch, 200).with_position(300.0, 100.0)
    }

    // Node bounds tests

    #[test]
    fn test_node_bounds_position() {
        let node = make_add_node();
        let bounds = node_bounds(&node);
        assert_eq!(bounds.min.x, 50.0);
        assert_eq!(bounds.min.y, 50.0);
        assert_eq!(bounds.width(), NODE_WIDTH);
    }

    #[test]
    fn test_node_bounds_height() {
        let node = make_add_node();
        // Add has 2 inputs, 1 output -> max(2, 1) = 2 ports
        let bounds = node_bounds(&node);
        let expected_height = TITLE_BAR_HEIGHT + 2.0 * PORT_ROW_HEIGHT + NODE_PADDING;
        assert!((bounds.height() - expected_height).abs() < 1e-5);
    }

    #[test]
    fn test_node_bounds_branch() {
        let node = make_branch_node();
        // Branch has 2 inputs, 2 outputs -> max(2, 2) = 2 ports
        let bounds = node_bounds(&node);
        let expected_height = TITLE_BAR_HEIGHT + 2.0 * PORT_ROW_HEIGHT + NODE_PADDING;
        assert!((bounds.height() - expected_height).abs() < 1e-5);
    }

    // Port position tests

    #[test]
    fn test_input_port_canvas_pos() {
        let node = make_add_node();
        let pos = input_port_canvas_pos(&node, 0);
        assert_eq!(pos.x, node.position.0 + PORT_INSET);
        assert!(pos.y > node.position.1 + TITLE_BAR_HEIGHT);
    }

    #[test]
    fn test_output_port_canvas_pos() {
        let node = make_add_node();
        let pos = output_port_canvas_pos(&node, 0);
        assert_eq!(pos.x, node.position.0 + NODE_WIDTH - PORT_INSET);
    }

    #[test]
    fn test_port_positions_ordered() {
        let node = make_branch_node();
        let pos0 = input_port_canvas_pos(&node, 0);
        let pos1 = input_port_canvas_pos(&node, 1);
        assert!(pos1.y > pos0.y, "second port should be below first");
    }

    // Hit testing tests

    #[test]
    fn test_hit_test_node_inside() {
        let node = make_add_node();
        let nodes = vec![node];
        let center = Pos2::new(50.0 + NODE_WIDTH / 2.0, 50.0 + TITLE_BAR_HEIGHT + 10.0);
        let result = hit_test_node(nodes.iter(), center);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn test_hit_test_node_outside() {
        let node = make_add_node();
        let nodes = vec![node];
        let outside = Pos2::new(0.0, 0.0);
        let result = hit_test_node(nodes.iter(), outside);
        assert!(result.is_none());
    }

    #[test]
    fn test_hit_test_node_on_edge() {
        let node = make_add_node();
        let nodes = vec![node];
        let on_edge = Pos2::new(50.0, 50.0);
        let result = hit_test_node(nodes.iter(), on_edge);
        assert_eq!(result, Some(1), "top-left corner should be inside");
    }

    #[test]
    fn test_hit_test_port_input() {
        let node = make_add_node();
        let port_pos = input_port_canvas_pos(&node, 0);
        let result = hit_test_port(&node, port_pos, PORT_RADIUS + 2.0);
        assert!(result.is_some());
        let (nid, _pid, dir) = result.unwrap();
        assert_eq!(nid, 1);
        assert_eq!(dir, PortDirection::Input);
    }

    #[test]
    fn test_hit_test_port_output() {
        let node = make_add_node();
        let port_pos = output_port_canvas_pos(&node, 0);
        let result = hit_test_port(&node, port_pos, PORT_RADIUS + 2.0);
        assert!(result.is_some());
        let (nid, _pid, dir) = result.unwrap();
        assert_eq!(nid, 1);
        assert_eq!(dir, PortDirection::Output);
    }

    #[test]
    fn test_hit_test_port_miss() {
        let node = make_add_node();
        let far_away = Pos2::new(1000.0, 1000.0);
        let result = hit_test_port(&node, far_away, PORT_RADIUS + 2.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_hit_test_port_near_but_outside_radius() {
        let node = make_add_node();
        let port_pos = input_port_canvas_pos(&node, 0);
        let offset = Pos2::new(port_pos.x + PORT_RADIUS + 10.0, port_pos.y);
        let result = hit_test_port(&node, offset, PORT_RADIUS);
        assert!(result.is_none());
    }

    // Port screen position

    #[test]
    fn test_port_screen_pos_identity() {
        let node = make_add_node();
        let view = ViewTransform::default();
        let origin = Pos2::ZERO;

        // First input port
        let port_id = node.inputs[0].id;
        let screen = port_screen_pos(&node, port_id, &view, origin).unwrap();
        let canvas = input_port_canvas_pos(&node, 0);
        // With identity transform, screen == canvas
        assert!((screen.x - canvas.x).abs() < 1e-3);
        assert!((screen.y - canvas.y).abs() < 1e-3);
    }

    #[test]
    fn test_port_screen_pos_invalid_id() {
        let node = make_add_node();
        let view = ViewTransform::default();
        let result = port_screen_pos(&node, 99999, &view, Pos2::ZERO);
        assert!(result.is_none());
    }

    // Multiple nodes hit test

    #[test]
    fn test_hit_test_multiple_nodes() {
        let node1 = Node::new(1, NodeKind::Add, 100).with_position(0.0, 0.0);
        let node2 = Node::new(2, NodeKind::Add, 200).with_position(300.0, 0.0);
        let nodes = vec![node1, node2];

        let pos1 = Pos2::new(NODE_WIDTH / 2.0, TITLE_BAR_HEIGHT + 10.0);
        let result = hit_test_node(nodes.iter(), pos1);
        assert_eq!(result, Some(1));

        let pos2 = Pos2::new(300.0 + NODE_WIDTH / 2.0, TITLE_BAR_HEIGHT + 10.0);
        let result = hit_test_node(nodes.iter(), pos2);
        assert_eq!(result, Some(2));
    }
}
