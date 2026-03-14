//! Property panel: shows and edits properties of the selected node.

use aether_creator_studio::visual_script::{DataType, NodeKind, Value};
use egui::Ui;

use crate::state::EditorState;

/// Draw the property panel for the currently selected node.
pub fn draw_properties(ui: &mut Ui, state: &mut EditorState) {
    ui.heading("Properties");
    ui.separator();

    let selected_id = match state.selection.single() {
        Some(id) => id,
        None => {
            if state.selection.count() == 0 {
                ui.label("No node selected.");
            } else {
                ui.label(format!("{} nodes selected.", state.selection.count()));
            }
            return;
        }
    };

    let node = match state.graph.get_node(selected_id) {
        Some(n) => n,
        None => {
            ui.label("Selected node not found.");
            return;
        }
    };

    // Node info
    ui.label(format!("Node: {}", node.display_name()));
    ui.label(format!("ID: {}", node.id));
    ui.label(format!(
        "Position: ({:.0}, {:.0})",
        node.position.0, node.position.1
    ));
    ui.separator();

    // Node-specific settings
    let kind = node.kind.clone();
    match &kind {
        NodeKind::OnTimer { interval_ms } => {
            ui.label("Timer Settings:");
            let mut ms = *interval_ms as i64;
            if ui
                .add(egui::DragValue::new(&mut ms).prefix("Interval: ").suffix(" ms").range(100..=60000))
                .changed()
            {
                let new_kind = NodeKind::OnTimer {
                    interval_ms: ms.max(100) as u64,
                };
                update_node_kind(state, selected_id, new_kind);
            }
        }
        NodeKind::Delay { delay_ms } => {
            ui.label("Delay Settings:");
            let mut ms = *delay_ms as i64;
            if ui
                .add(egui::DragValue::new(&mut ms).prefix("Delay: ").suffix(" ms").range(0..=60000))
                .changed()
            {
                let new_kind = NodeKind::Delay {
                    delay_ms: ms.max(0) as u64,
                };
                update_node_kind(state, selected_id, new_kind);
            }
        }
        NodeKind::Sequence { output_count } => {
            ui.label("Sequence Settings:");
            let mut count = *output_count as i32;
            if ui
                .add(egui::DragValue::new(&mut count).prefix("Outputs: ").range(2..=10))
                .changed()
            {
                let new_kind = NodeKind::Sequence {
                    output_count: count.max(2) as u32,
                };
                update_node_kind(state, selected_id, new_kind);
            }
        }
        NodeKind::GetVariable { var_name } => {
            ui.label("Variable Settings:");
            let mut name = var_name.clone();
            if ui
                .add(egui::TextEdit::singleline(&mut name).hint_text("Variable name"))
                .changed()
            {
                let new_kind = NodeKind::GetVariable { var_name: name };
                update_node_kind(state, selected_id, new_kind);
            }
        }
        NodeKind::SetVariable { var_name } => {
            ui.label("Variable Settings:");
            let mut name = var_name.clone();
            if ui
                .add(egui::TextEdit::singleline(&mut name).hint_text("Variable name"))
                .changed()
            {
                let new_kind = NodeKind::SetVariable { var_name: name };
                update_node_kind(state, selected_id, new_kind);
            }
        }
        _ => {}
    }

    ui.separator();

    // Input ports with default values
    let node = match state.graph.get_node(selected_id) {
        Some(n) => n,
        None => return,
    };

    if !node.inputs.is_empty() {
        ui.label("Input Ports:");
        for port in &node.inputs {
            ui.horizontal(|ui| {
                let color = EditorState::data_type_color(port.data_type);
                ui.colored_label(color, &port.name);
                ui.label(format!("({})", port.data_type));
            });

            // Show editable default value if port has one and is not connected
            let is_connected = !state.graph.connections_to_port(port.id).is_empty();
            if !is_connected && port.data_type != DataType::Flow {
                let current_value = port.default_value.clone().unwrap_or(
                    EditorState::default_value_for_type(port.data_type),
                );
                draw_value_editor(ui, &current_value, port.data_type);
            }
        }
    }

    if !node.outputs.is_empty() {
        ui.separator();
        ui.label("Output Ports:");
        for port in &node.outputs {
            ui.horizontal(|ui| {
                let color = EditorState::data_type_color(port.data_type);
                ui.colored_label(color, &port.name);
                ui.label(format!("({})", port.data_type));
            });
        }
    }

    // Comment
    ui.separator();
    let comment = state
        .graph
        .get_node(selected_id)
        .and_then(|n| n.comment.clone())
        .unwrap_or_default();
    let mut comment_buf = comment;
    ui.label("Comment:");
    if ui
        .add(egui::TextEdit::multiline(&mut comment_buf).hint_text("Add a note..."))
        .changed()
    {
        if let Some(node) = state.graph.get_node_mut(selected_id) {
            node.comment = if comment_buf.is_empty() {
                None
            } else {
                Some(comment_buf)
            };
        }
    }
}

/// Draw a value editor widget for a given value type.
fn draw_value_editor(ui: &mut Ui, value: &Value, _data_type: DataType) {
    ui.indent("value_editor", |ui| {
        match value {
            Value::Bool(v) => {
                let mut val = *v;
                ui.checkbox(&mut val, "");
            }
            Value::Int(v) => {
                let mut val = *v;
                ui.add(egui::DragValue::new(&mut val));
            }
            Value::Float(v) => {
                let mut val = *v;
                ui.add(egui::DragValue::new(&mut val).speed(0.1));
            }
            Value::String(v) => {
                let mut val = v.clone();
                ui.text_edit_singleline(&mut val);
            }
            Value::Vec3 { x, y, z } => {
                let mut vx = *x;
                let mut vy = *y;
                let mut vz = *z;
                ui.horizontal(|ui| {
                    ui.label("X:");
                    ui.add(egui::DragValue::new(&mut vx).speed(0.1));
                    ui.label("Y:");
                    ui.add(egui::DragValue::new(&mut vy).speed(0.1));
                    ui.label("Z:");
                    ui.add(egui::DragValue::new(&mut vz).speed(0.1));
                });
            }
            Value::Entity(id) => {
                let mut val = *id as i64;
                ui.add(egui::DragValue::new(&mut val).prefix("Entity: "));
            }
            Value::None => {
                ui.label("(no value)");
            }
        }
    });
}

/// Update the node kind (for nodes with configurable parameters).
/// This rebuilds the node's ports.
fn update_node_kind(state: &mut EditorState, node_id: u64, new_kind: NodeKind) {
    state.push_undo("Edit node property");
    // Remove old node and its connections, then recreate
    if let Some(old_node) = state.graph.get_node(node_id) {
        let old_pos = old_node.position;
        let _ = state.graph.remove_node(node_id);
        if let Ok(new_id) = state.graph.add_node_at(new_kind, old_pos.0, old_pos.1) {
            state.selection.select_single(new_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_node_kind_preserves_position() {
        let mut state = EditorState::new();
        let id = state
            .add_node_at(NodeKind::OnTimer { interval_ms: 1000 }, 100.0, 200.0)
            .unwrap();

        update_node_kind(&mut state, id, NodeKind::OnTimer { interval_ms: 2000 });

        // The new node should be at the same position
        let nodes: Vec<_> = state.graph.nodes().collect();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].position, (100.0, 200.0));
    }
}
