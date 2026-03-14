//! Interaction handler: processes mouse/keyboard input for the visual editor.

use aether_creator_studio::visual_script::{NodeId, PortDirection, PortId};
use egui::{Key, Pos2, Rect, Response, Vec2};

use crate::node_renderer::{hit_test_node, hit_test_port, PORT_RADIUS};
use crate::state::{EditorMode, EditorState};

/// Port hit-test radius in canvas units.
const PORT_HIT_RADIUS: f32 = PORT_RADIUS + 6.0;

/// Data carried during a port connection drag.
#[derive(Debug, Clone)]
pub struct PendingConnection {
    pub from_node: NodeId,
    pub from_port: PortId,
    pub from_direction: PortDirection,
    pub current_screen_pos: Pos2,
}

/// Tracks ongoing interaction state (not stored in EditorState to keep it serializable).
#[derive(Debug, Clone, Default)]
pub struct InteractionState {
    /// True if currently dragging node(s).
    pub dragging_nodes: bool,
    /// Starting canvas position of a drag.
    pub drag_start_canvas: Option<Pos2>,
    /// Pending connection being dragged.
    pub pending_connection: Option<PendingConnection>,
    /// Box selection start and current in screen coords.
    pub box_select_start: Option<Pos2>,
    pub box_select_current: Option<Pos2>,
    /// Whether space is held (for panning).
    pub space_held: bool,
}

/// Process canvas interactions. Returns true if the graph was modified.
pub fn handle_canvas_interaction(
    response: &Response,
    state: &mut EditorState,
    interaction: &mut InteractionState,
    viewport: Rect,
) -> bool {
    let mut modified = false;

    // Handle keyboard shortcuts
    modified |= handle_keyboard(response, state, interaction);

    // Handle scroll zoom
    if let Some(hover_pos) = response.hover_pos() {
        let scroll_delta = response.ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta.abs() > 0.1 {
            let zoom_factor = scroll_delta * 0.002;
            state.view.zoom_at(zoom_factor, hover_pos, viewport.min);
        }
    }

    // Handle panning with middle mouse button or space+drag
    let middle_button_down = response.ctx.input(|i| i.pointer.middle_down());
    interaction.space_held = response.ctx.input(|i| i.key_down(Key::Space));

    if (middle_button_down || interaction.space_held) && response.dragged() {
        let drag_delta = response.drag_delta();
        state.view.pan_by(drag_delta);
        return modified;
    }

    // Handle primary button interactions
    handle_primary_button(response, state, interaction, viewport, &mut modified);

    modified
}

fn handle_primary_button(
    response: &Response,
    state: &mut EditorState,
    interaction: &mut InteractionState,
    viewport: Rect,
    modified: &mut bool,
) {
    let pointer_pos = match response.hover_pos().or(response.interact_pointer_pos()) {
        Some(p) => p,
        None => return,
    };

    let canvas_pos = state.view.screen_to_canvas(pointer_pos, viewport.min);
    let shift_held = response.ctx.input(|i| i.modifiers.shift);

    // Click down: start interaction
    if response.drag_started() {
        // Check for port hit first
        if let Some(port_hit) = find_port_at(state, canvas_pos) {
            // Start connection drag
            interaction.pending_connection = Some(PendingConnection {
                from_node: port_hit.0,
                from_port: port_hit.1,
                from_direction: port_hit.2,
                current_screen_pos: pointer_pos,
            });
            state.mode = EditorMode::Connecting {
                from_node: port_hit.0,
                from_port: port_hit.1,
                from_direction: port_hit.2,
            };
            return;
        }

        // Check for node hit
        if let Some(node_id) = hit_test_node(state.graph.nodes(), canvas_pos) {
            if shift_held {
                state.selection.toggle(node_id);
            } else if !state.selection.is_selected(node_id) {
                state.selection.select_single(node_id);
            }
            interaction.dragging_nodes = true;
            interaction.drag_start_canvas = Some(canvas_pos);
            return;
        }

        // Empty space: start box selection or clear selection
        if !shift_held {
            state.selection.clear();
        }
        interaction.box_select_start = Some(pointer_pos);
        interaction.box_select_current = Some(pointer_pos);
        state.mode = EditorMode::BoxSelecting;
    }

    // Dragging
    if response.dragged() {
        // Connection drag
        if let Some(pending) = &mut interaction.pending_connection {
            pending.current_screen_pos = pointer_pos;
            return;
        }

        // Node drag
        if interaction.dragging_nodes {
            let delta = response.drag_delta();
            let canvas_delta = Vec2::new(
                delta.x / state.view.zoom,
                delta.y / state.view.zoom,
            );
            let selected: Vec<NodeId> = state.selection.nodes.iter().copied().collect();
            for node_id in selected {
                if let Some(node) = state.graph.get_node_mut(node_id) {
                    node.position.0 += canvas_delta.x;
                    node.position.1 += canvas_delta.y;
                }
            }
            return;
        }

        // Box selection
        if interaction.box_select_start.is_some() {
            interaction.box_select_current = Some(pointer_pos);
        }
    }

    // Release
    if response.drag_stopped() {
        // Complete connection
        if let Some(pending) = interaction.pending_connection.take() {
            if let Some(target_hit) = find_port_at(state, canvas_pos) {
                let (to_node, to_port, to_dir) = target_hit;
                // Determine from/to based on direction
                if pending.from_direction == PortDirection::Output
                    && to_dir == PortDirection::Input
                {
                    if state.can_connect_ports(
                        pending.from_node,
                        pending.from_port,
                        to_node,
                        to_port,
                    ) {
                        let _ = state.connect(
                            pending.from_node,
                            pending.from_port,
                            to_node,
                            to_port,
                        );
                        *modified = true;
                    }
                } else if pending.from_direction == PortDirection::Input
                    && to_dir == PortDirection::Output
                {
                    if state.can_connect_ports(
                        to_node,
                        to_port,
                        pending.from_node,
                        pending.from_port,
                    ) {
                        let _ = state.connect(
                            to_node,
                            to_port,
                            pending.from_node,
                            pending.from_port,
                        );
                        *modified = true;
                    }
                }
            }
            state.mode = EditorMode::Normal;
        }

        // Complete box selection
        if let (Some(start), Some(end)) = (
            interaction.box_select_start.take(),
            interaction.box_select_current.take(),
        ) {
            let screen_rect = Rect::from_two_pos(start, end);
            let canvas_min = state.view.screen_to_canvas(screen_rect.min, viewport.min);
            let canvas_max = state.view.screen_to_canvas(screen_rect.max, viewport.min);
            let select_rect = Rect::from_min_max(canvas_min, canvas_max);

            for node in state.graph.nodes() {
                let node_center = Pos2::new(
                    node.position.0 + 90.0, // Approximate center
                    node.position.1 + 40.0,
                );
                if select_rect.contains(node_center) {
                    state.selection.add(node.id);
                }
            }
            state.mode = EditorMode::Normal;
        }

        interaction.dragging_nodes = false;
        interaction.drag_start_canvas = None;
    }
}

fn handle_keyboard(
    response: &Response,
    state: &mut EditorState,
    _interaction: &mut InteractionState,
) -> bool {
    let mut modified = false;

    response.ctx.input(|input| {
        // Delete selected nodes
        if input.key_pressed(Key::Delete) || input.key_pressed(Key::Backspace) {
            if !state.selection.nodes.is_empty() {
                state.delete_selected();
                modified = true;
            }
        }

        // Undo: Ctrl+Z (or Cmd+Z on macOS)
        if input.modifiers.command && input.key_pressed(Key::Z) && !input.modifiers.shift {
            state.undo();
        }

        // Redo: Ctrl+Y or Ctrl+Shift+Z
        if input.modifiers.command && input.key_pressed(Key::Y) {
            state.redo();
        }
        if input.modifiers.command && input.modifiers.shift && input.key_pressed(Key::Z) {
            state.redo();
        }

        // Copy: Ctrl+C
        if input.modifiers.command && input.key_pressed(Key::C) {
            state.copy_selected();
        }

        // Select all: Ctrl+A
        if input.modifiers.command && input.key_pressed(Key::A) {
            for node in state.graph.nodes() {
                state.selection.add(node.id);
            }
        }
    });

    modified
}

/// Find a port at the given canvas position.
fn find_port_at(
    state: &EditorState,
    canvas_pos: Pos2,
) -> Option<(NodeId, PortId, PortDirection)> {
    for node in state.graph.nodes() {
        if let Some(hit) = hit_test_port(node, canvas_pos, PORT_HIT_RADIUS) {
            return Some(hit);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::EditorState;
    use aether_creator_studio::visual_script::NodeKind;

    #[test]
    fn test_interaction_state_default() {
        let is = InteractionState::default();
        assert!(!is.dragging_nodes);
        assert!(is.drag_start_canvas.is_none());
        assert!(is.pending_connection.is_none());
        assert!(is.box_select_start.is_none());
        assert!(!is.space_held);
    }

    #[test]
    fn test_find_port_at_input() {
        let mut state = EditorState::new();
        let id = state.add_node_at(NodeKind::Add, 0.0, 0.0).unwrap();

        // Input port 0 is at node x + 0 (PORT_INSET), y offset from title
        let port_pos = crate::node_renderer::input_port_canvas_pos(
            state.graph.get_node(id).unwrap(),
            0,
        );
        let result = find_port_at(&state, port_pos);
        assert!(result.is_some());
        let (nid, _pid, dir) = result.unwrap();
        assert_eq!(nid, id);
        assert_eq!(dir, PortDirection::Input);
    }

    #[test]
    fn test_find_port_at_output() {
        let mut state = EditorState::new();
        let id = state.add_node_at(NodeKind::Add, 0.0, 0.0).unwrap();

        let port_pos = crate::node_renderer::output_port_canvas_pos(
            state.graph.get_node(id).unwrap(),
            0,
        );
        let result = find_port_at(&state, port_pos);
        assert!(result.is_some());
        let (nid, _pid, dir) = result.unwrap();
        assert_eq!(nid, id);
        assert_eq!(dir, PortDirection::Output);
    }

    #[test]
    fn test_find_port_at_miss() {
        let mut state = EditorState::new();
        state.add_node_at(NodeKind::Add, 0.0, 0.0).unwrap();

        let far = Pos2::new(1000.0, 1000.0);
        let result = find_port_at(&state, far);
        assert!(result.is_none());
    }

    #[test]
    fn test_pending_connection() {
        let pending = PendingConnection {
            from_node: 1,
            from_port: 2,
            from_direction: PortDirection::Output,
            current_screen_pos: Pos2::new(100.0, 100.0),
        };
        assert_eq!(pending.from_node, 1);
        assert_eq!(pending.from_direction, PortDirection::Output);
    }
}
