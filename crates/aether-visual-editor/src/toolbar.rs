//! Toolbar: top bar with compile, validate, layout, undo/redo, zoom, save/load.

use aether_creator_studio::visual_script::{
    apply_layout, compile, compute_layout, validate_graph, LayoutConfig,
};
use egui::Ui;

use crate::state::EditorState;

/// Draw the editor toolbar. Returns true if the graph was modified.
pub fn draw_toolbar(ui: &mut Ui, state: &mut EditorState) -> bool {
    let mut modified = false;

    ui.horizontal(|ui| {
        // Compile
        if ui
            .button("Compile")
            .on_hover_text("Compile graph to WASM bytecode (validates first)")
            .clicked()
        {
            handle_compile(state);
        }

        // Validate
        if ui
            .button("Validate")
            .on_hover_text("Run validation checks on the graph")
            .clicked()
        {
            handle_validate(state);
        }

        ui.separator();

        // Auto-layout
        if ui
            .button("Auto Layout")
            .on_hover_text("Automatically arrange nodes")
            .clicked()
        {
            handle_auto_layout(state);
            modified = true;
        }

        ui.separator();

        // Undo/Redo
        let undo_enabled = state.can_undo();
        if ui
            .add_enabled(undo_enabled, egui::Button::new("Undo"))
            .on_hover_text("Undo last action (Ctrl+Z)")
            .clicked()
        {
            state.undo();
        }

        let redo_enabled = state.can_redo();
        if ui
            .add_enabled(redo_enabled, egui::Button::new("Redo"))
            .on_hover_text("Redo last undone action (Ctrl+Y)")
            .clicked()
        {
            state.redo();
        }

        ui.separator();

        // Zoom controls
        if ui.button("-").on_hover_text("Zoom out").clicked() {
            state.view.zoom = (state.view.zoom - 0.1).max(0.1);
        }
        ui.label(format!("{:.0}%", state.view.zoom * 100.0));
        if ui.button("+").on_hover_text("Zoom in").clicked() {
            state.view.zoom = (state.view.zoom + 0.1).min(5.0);
        }
        if ui.button("100%").on_hover_text("Reset zoom").clicked() {
            state.view.zoom = 1.0;
        }

        ui.separator();

        // Save/Load
        if ui
            .button("Save")
            .on_hover_text("Save graph to JSON")
            .clicked()
        {
            handle_save(state);
        }
        if ui
            .button("Load")
            .on_hover_text("Load graph from JSON")
            .clicked()
        {
            handle_load(state);
        }

        ui.separator();

        // Toggle panels
        ui.checkbox(&mut state.show_properties, "Properties");
        ui.checkbox(&mut state.show_minimap, "Minimap");

        // Right-align info
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!(
                "Nodes: {} | Connections: {}",
                state.graph.node_count(),
                state.graph.connection_count(),
            ));
        });
    });

    modified
}

fn handle_compile(state: &mut EditorState) {
    if state.graph.node_count() == 0 {
        state.set_status("Cannot compile empty graph", true);
        return;
    }

    let validation = validate_graph(&state.graph);
    if !validation.is_valid() {
        let error_count = validation.errors().len();
        let first_error = validation
            .errors()
            .first()
            .map(|d| d.message.clone())
            .unwrap_or_default();
        state.set_status(
            &format!("Validation failed: {} error(s). First: {}", error_count, first_error),
            true,
        );
        return;
    }

    match compile(&state.graph) {
        Ok(result) => {
            state.set_status(
                &format!(
                    "Compiled successfully: {} IR instructions, {} registers, {} WASM bytes",
                    result.instructions.len(),
                    result.register_count,
                    result.wasm_bytes.len(),
                ),
                false,
            );
        }
        Err(e) => {
            state.set_status(&format!("Compilation failed: {}", e), true);
        }
    }
}

fn handle_validate(state: &mut EditorState) {
    if state.graph.node_count() == 0 {
        state.set_status("Graph is empty", false);
        return;
    }

    let result = validate_graph(&state.graph);
    let error_count = result.errors().len();
    let warning_count = result.warnings().len();

    if result.is_valid() {
        if warning_count > 0 {
            state.set_status(
                &format!("Valid with {} warning(s)", warning_count),
                false,
            );
        } else {
            state.set_status("Graph is valid", false);
        }
    } else {
        let first_error = result
            .errors()
            .first()
            .map(|d| d.message.clone())
            .unwrap_or_default();
        state.set_status(
            &format!(
                "{} error(s), {} warning(s). First: {}",
                error_count, warning_count, first_error
            ),
            true,
        );
    }
}

fn handle_auto_layout(state: &mut EditorState) {
    state.push_undo("Auto layout");
    let config = LayoutConfig::default();
    let layout = compute_layout(&state.graph, &config);
    apply_layout(&mut state.graph, &layout);
    state.set_status("Layout applied", false);
}

fn handle_save(state: &mut EditorState) {
    match serde_json::to_string_pretty(&state.graph) {
        Ok(json) => {
            // In a real application, this would write to a file via a dialog.
            // For now, copy to clipboard content.
            state.clipboard.content = Some(json);
            state.set_status("Graph saved to clipboard (copy it)", false);
        }
        Err(e) => {
            state.set_status(&format!("Save failed: {}", e), true);
        }
    }
}

fn handle_load(state: &mut EditorState) {
    if let Some(json) = &state.clipboard.content {
        match serde_json::from_str(json) {
            Ok(graph) => {
                state.push_undo("Load graph");
                state.graph = graph;
                state.selection.clear();
                state.set_status("Graph loaded from clipboard", false);
            }
            Err(e) => {
                state.set_status(&format!("Load failed: {}", e), true);
            }
        }
    } else {
        state.set_status("No graph data in clipboard", true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_creator_studio::visual_script::NodeKind;

    #[test]
    fn test_compile_empty_graph() {
        let mut state = EditorState::new();
        handle_compile(&mut state);
        assert!(state.status.as_ref().unwrap().is_error);
        assert!(state.status.as_ref().unwrap().text.contains("empty"));
    }

    #[test]
    fn test_compile_valid_graph() {
        let mut state = EditorState::new();
        let event_id = state.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
        let log_id = state.add_node_at(NodeKind::Log, 100.0, 0.0).unwrap();

        let exec_out = state.graph.get_node(event_id).unwrap().find_output("exec").unwrap().id;
        let exec_in = state.graph.get_node(log_id).unwrap().find_input("exec").unwrap().id;
        state.connect(event_id, exec_out, log_id, exec_in).unwrap();

        handle_compile(&mut state);
        assert!(
            !state.status.as_ref().unwrap().is_error,
            "status: {:?}",
            state.status,
        );
        assert!(state.status.as_ref().unwrap().text.contains("Compiled"));
    }

    #[test]
    fn test_validate_empty_graph() {
        let mut state = EditorState::new();
        handle_validate(&mut state);
        assert!(!state.status.as_ref().unwrap().is_error);
    }

    #[test]
    fn test_validate_valid_graph() {
        let mut state = EditorState::new();
        let event_id = state.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
        let log_id = state.add_node_at(NodeKind::Log, 100.0, 0.0).unwrap();

        let exec_out = state.graph.get_node(event_id).unwrap().find_output("exec").unwrap().id;
        let exec_in = state.graph.get_node(log_id).unwrap().find_input("exec").unwrap().id;
        state.connect(event_id, exec_out, log_id, exec_in).unwrap();

        handle_validate(&mut state);
        assert!(!state.status.as_ref().unwrap().is_error);
    }

    #[test]
    fn test_validate_invalid_graph() {
        let mut state = EditorState::new();
        // Only an action node with no events -> invalid
        state.add_node_at(NodeKind::Log, 0.0, 0.0).unwrap();
        handle_validate(&mut state);
        assert!(state.status.as_ref().unwrap().is_error);
    }

    #[test]
    fn test_auto_layout() {
        let mut state = EditorState::new();
        let event_id = state.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
        let log_id = state.add_node_at(NodeKind::Log, 0.0, 0.0).unwrap();

        let exec_out = state.graph.get_node(event_id).unwrap().find_output("exec").unwrap().id;
        let exec_in = state.graph.get_node(log_id).unwrap().find_input("exec").unwrap().id;
        state.connect(event_id, exec_out, log_id, exec_in).unwrap();

        handle_auto_layout(&mut state);
        // After layout, the log node should be to the right of the event node
        let event_x = state.graph.get_node(event_id).unwrap().position.0;
        let log_x = state.graph.get_node(log_id).unwrap().position.0;
        assert!(log_x > event_x, "log should be right of event after layout");
    }

    #[test]
    fn test_save_and_load() {
        let mut state = EditorState::new();
        state.add_node_at(NodeKind::OnStart, 50.0, 60.0).unwrap();

        handle_save(&mut state);
        assert!(state.clipboard.content.is_some());

        // Modify graph
        state.graph.clear();
        assert_eq!(state.graph.node_count(), 0);

        // Load back
        handle_load(&mut state);
        assert_eq!(state.graph.node_count(), 1);
    }

    #[test]
    fn test_load_no_clipboard() {
        let mut state = EditorState::new();
        handle_load(&mut state);
        assert!(state.status.as_ref().unwrap().is_error);
    }

    #[test]
    fn test_load_invalid_json() {
        let mut state = EditorState::new();
        state.clipboard.content = Some("not valid json".to_string());
        handle_load(&mut state);
        assert!(state.status.as_ref().unwrap().is_error);
    }
}
