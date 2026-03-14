//! Editor state: wraps NodeGraph with selection, undo/redo, clipboard, and view state.

use std::collections::HashSet;

use aether_creator_studio::visual_script::{
    ConnectionId, DataType, GraphError, NodeGraph, NodeId, NodeKind, PortDirection, PortId,
    Value,
};

use crate::canvas::ViewTransform;

/// Maximum number of undo steps to keep.
const MAX_UNDO_HISTORY: usize = 100;

/// The current editor interaction mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorMode {
    /// Normal mode: click to select, drag to move.
    Normal,
    /// Connecting two ports: dragging a wire from an output port.
    Connecting {
        from_node: NodeId,
        from_port: PortId,
        from_direction: PortDirection,
    },
    /// Box-selecting: dragging a rectangle to select multiple nodes.
    BoxSelecting,
}

/// Selection state.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    /// Set of currently selected node IDs.
    pub nodes: HashSet<NodeId>,
}

impl Selection {
    /// Select a single node, clearing previous selection.
    pub fn select_single(&mut self, node_id: NodeId) {
        self.nodes.clear();
        self.nodes.insert(node_id);
    }

    /// Toggle a node in the selection (for shift-click).
    pub fn toggle(&mut self, node_id: NodeId) {
        if self.nodes.contains(&node_id) {
            self.nodes.remove(&node_id);
        } else {
            self.nodes.insert(node_id);
        }
    }

    /// Add a node to the selection.
    pub fn add(&mut self, node_id: NodeId) {
        self.nodes.insert(node_id);
    }

    /// Clear all selection.
    pub fn clear(&mut self) {
        self.nodes.clear();
    }

    /// Check if a node is selected.
    pub fn is_selected(&self, node_id: NodeId) -> bool {
        self.nodes.contains(&node_id)
    }

    /// Number of selected nodes.
    pub fn count(&self) -> usize {
        self.nodes.len()
    }

    /// Get the single selected node, if exactly one is selected.
    pub fn single(&self) -> Option<NodeId> {
        if self.nodes.len() == 1 {
            self.nodes.iter().next().copied()
        } else {
            None
        }
    }
}

/// A snapshot of the graph for undo/redo.
#[derive(Clone)]
struct UndoEntry {
    graph_json: String,
    description: String,
}

/// Clipboard contents.
#[derive(Debug, Clone, Default)]
pub struct Clipboard {
    /// Serialized nodes and connections for copy/paste.
    pub content: Option<String>,
}

/// Status message shown at the bottom of the editor.
#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
}

/// The main editor state.
pub struct EditorState {
    /// The node graph being edited.
    pub graph: NodeGraph,
    /// View transform (pan, zoom).
    pub view: ViewTransform,
    /// Current selection.
    pub selection: Selection,
    /// Current editor mode.
    pub mode: EditorMode,
    /// Clipboard.
    pub clipboard: Clipboard,
    /// Status message.
    pub status: Option<StatusMessage>,
    /// Search query for the palette.
    pub palette_search: String,
    /// Whether the property panel should be visible.
    pub show_properties: bool,
    /// Whether the minimap should be visible.
    pub show_minimap: bool,
    /// Undo stack.
    undo_stack: Vec<UndoEntry>,
    /// Redo stack.
    redo_stack: Vec<UndoEntry>,
}

impl EditorState {
    /// Create a new editor state with a default empty graph.
    pub fn new() -> Self {
        Self {
            graph: NodeGraph::new("editor", "Untitled Script"),
            view: ViewTransform::default(),
            selection: Selection::default(),
            mode: EditorMode::Normal,
            clipboard: Clipboard::default(),
            status: None,
            palette_search: String::new(),
            show_properties: true,
            show_minimap: true,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Create a new editor state with the given graph.
    pub fn with_graph(graph: NodeGraph) -> Self {
        Self {
            graph,
            view: ViewTransform::default(),
            selection: Selection::default(),
            mode: EditorMode::Normal,
            clipboard: Clipboard::default(),
            status: None,
            palette_search: String::new(),
            show_properties: true,
            show_minimap: true,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Push the current graph state onto the undo stack.
    pub fn push_undo(&mut self, description: &str) {
        let json = serde_json::to_string(&self.graph).unwrap_or_default();
        self.undo_stack.push(UndoEntry {
            graph_json: json,
            description: description.to_string(),
        });
        if self.undo_stack.len() > MAX_UNDO_HISTORY {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Undo the last operation. Returns true if an undo was performed.
    pub fn undo(&mut self) -> bool {
        if let Some(entry) = self.undo_stack.pop() {
            // Save current state to redo
            let current_json = serde_json::to_string(&self.graph).unwrap_or_default();
            self.redo_stack.push(UndoEntry {
                graph_json: current_json,
                description: entry.description.clone(),
            });
            // Restore
            if let Ok(graph) = serde_json::from_str(&entry.graph_json) {
                self.graph = graph;
                self.selection.clear();
                self.set_status(&format!("Undo: {}", entry.description), false);
                return true;
            }
        }
        false
    }

    /// Redo the last undone operation. Returns true if a redo was performed.
    pub fn redo(&mut self) -> bool {
        if let Some(entry) = self.redo_stack.pop() {
            let current_json = serde_json::to_string(&self.graph).unwrap_or_default();
            self.undo_stack.push(UndoEntry {
                graph_json: current_json,
                description: entry.description.clone(),
            });
            if let Ok(graph) = serde_json::from_str(&entry.graph_json) {
                self.graph = graph;
                self.selection.clear();
                self.set_status(&format!("Redo: {}", entry.description), false);
                return true;
            }
        }
        false
    }

    /// Check if undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Number of undo steps available.
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Number of redo steps available.
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Add a node at a canvas position.
    pub fn add_node_at(
        &mut self,
        kind: NodeKind,
        x: f32,
        y: f32,
    ) -> Result<NodeId, GraphError> {
        self.push_undo("Add node");
        let id = self.graph.add_node_at(kind, x, y)?;
        self.selection.select_single(id);
        self.set_status("Node added", false);
        Ok(id)
    }

    /// Delete the currently selected nodes.
    pub fn delete_selected(&mut self) {
        if self.selection.nodes.is_empty() {
            return;
        }
        self.push_undo("Delete nodes");
        let to_delete: Vec<NodeId> = self.selection.nodes.iter().copied().collect();
        for id in to_delete {
            let _ = self.graph.remove_node(id);
        }
        self.selection.clear();
        self.set_status("Deleted selected nodes", false);
    }

    /// Connect two ports.
    pub fn connect(
        &mut self,
        from_node: NodeId,
        from_port: PortId,
        to_node: NodeId,
        to_port: PortId,
    ) -> Result<ConnectionId, GraphError> {
        self.push_undo("Connect ports");
        let result = self.graph.connect(from_node, from_port, to_node, to_port);
        match &result {
            Ok(_) => self.set_status("Connected", false),
            Err(e) => self.set_status(&format!("Connection failed: {}", e), true),
        }
        result
    }

    /// Disconnect a connection.
    pub fn disconnect(&mut self, connection_id: ConnectionId) -> Result<(), GraphError> {
        self.push_undo("Disconnect");
        self.graph.disconnect(connection_id)?;
        self.set_status("Disconnected", false);
        Ok(())
    }

    /// Copy selected nodes to clipboard.
    pub fn copy_selected(&mut self) {
        if self.selection.nodes.is_empty() {
            return;
        }
        // Serialize selected node IDs as a simple JSON list for now
        let selected: Vec<NodeId> = self.selection.nodes.iter().copied().collect();
        if let Ok(json) = serde_json::to_string(&selected) {
            self.clipboard.content = Some(json);
            self.set_status("Copied to clipboard", false);
        }
    }

    /// Set the status message.
    pub fn set_status(&mut self, text: &str, is_error: bool) {
        self.status = Some(StatusMessage {
            text: text.to_string(),
            is_error,
        });
    }

    /// Clear the status message.
    pub fn clear_status(&mut self) {
        self.status = None;
    }

    /// Check if a port can accept a connection from the given source port.
    pub fn can_connect_ports(
        &self,
        from_node: NodeId,
        from_port: PortId,
        to_node: NodeId,
        to_port: PortId,
    ) -> bool {
        if from_node == to_node {
            return false;
        }
        let from_p = self
            .graph
            .get_node(from_node)
            .and_then(|n| n.find_port(from_port));
        let to_p = self
            .graph
            .get_node(to_node)
            .and_then(|n| n.find_port(to_port));

        match (from_p, to_p) {
            (Some(fp), Some(tp)) => {
                if fp.direction == tp.direction {
                    return false;
                }
                let (out_type, in_type) = if fp.direction == PortDirection::Output {
                    (fp.data_type, tp.data_type)
                } else {
                    (tp.data_type, fp.data_type)
                };
                out_type.is_compatible_with(in_type)
            }
            _ => false,
        }
    }

    /// Get the data type color for a given port data type.
    pub fn data_type_color(dt: DataType) -> egui::Color32 {
        match dt {
            DataType::Flow => egui::Color32::WHITE,
            DataType::Bool => egui::Color32::from_rgb(220, 50, 50),
            DataType::Int => egui::Color32::from_rgb(50, 200, 220),
            DataType::Float => egui::Color32::from_rgb(50, 200, 80),
            DataType::String => egui::Color32::from_rgb(220, 50, 220),
            DataType::Vec3 => egui::Color32::from_rgb(220, 220, 50),
            DataType::Entity => egui::Color32::from_rgb(220, 150, 50),
            DataType::Any => egui::Color32::from_gray(160),
        }
    }

    /// Get the category color for a node kind.
    pub fn node_category_color(kind: &NodeKind) -> egui::Color32 {
        if kind.is_event() {
            egui::Color32::from_rgb(180, 40, 40) // Events: red
        } else if kind.is_pure() {
            // Distinguish math from conditions
            match kind {
                NodeKind::Equal
                | NodeKind::NotEqual
                | NodeKind::Greater
                | NodeKind::Less
                | NodeKind::And
                | NodeKind::Or
                | NodeKind::Not => egui::Color32::from_rgb(200, 200, 40), // Conditions: yellow
                NodeKind::GetVariable { .. } => egui::Color32::from_rgb(40, 160, 40), // Variables: green
                _ => egui::Color32::from_rgb(140, 40, 180), // Math: purple
            }
        } else {
            match kind {
                NodeKind::Branch
                | NodeKind::ForLoop
                | NodeKind::Sequence { .. }
                | NodeKind::Delay { .. } => egui::Color32::from_rgb(200, 120, 40), // Flow: orange
                NodeKind::SetVariable { .. } => egui::Color32::from_rgb(40, 160, 40), // Variables: green
                _ => egui::Color32::from_rgb(40, 100, 200), // Actions: blue
            }
        }
    }

    /// Get the default value for a data type (used for property panel).
    pub fn default_value_for_type(dt: DataType) -> Value {
        match dt {
            DataType::Flow => Value::None,
            DataType::Bool => Value::Bool(false),
            DataType::Int => Value::Int(0),
            DataType::Float => Value::Float(0.0),
            DataType::String => Value::String(String::new()),
            DataType::Vec3 => Value::Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            DataType::Entity => Value::Entity(0),
            DataType::Any => Value::None,
        }
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_editor_state() {
        let state = EditorState::new();
        assert_eq!(state.graph.node_count(), 0);
        assert_eq!(state.selection.count(), 0);
        assert_eq!(state.mode, EditorMode::Normal);
        assert!(!state.can_undo());
        assert!(!state.can_redo());
    }

    #[test]
    fn test_with_graph() {
        let mut graph = NodeGraph::new("test", "Test");
        graph.add_node(NodeKind::OnStart).unwrap();
        let state = EditorState::with_graph(graph);
        assert_eq!(state.graph.node_count(), 1);
    }

    // Selection tests

    #[test]
    fn test_selection_select_single() {
        let mut sel = Selection::default();
        sel.select_single(1);
        assert!(sel.is_selected(1));
        assert_eq!(sel.count(), 1);

        sel.select_single(2);
        assert!(!sel.is_selected(1));
        assert!(sel.is_selected(2));
        assert_eq!(sel.count(), 1);
    }

    #[test]
    fn test_selection_toggle() {
        let mut sel = Selection::default();
        sel.toggle(1);
        assert!(sel.is_selected(1));
        sel.toggle(2);
        assert!(sel.is_selected(1));
        assert!(sel.is_selected(2));
        assert_eq!(sel.count(), 2);

        sel.toggle(1);
        assert!(!sel.is_selected(1));
        assert!(sel.is_selected(2));
        assert_eq!(sel.count(), 1);
    }

    #[test]
    fn test_selection_add() {
        let mut sel = Selection::default();
        sel.add(1);
        sel.add(2);
        sel.add(3);
        assert_eq!(sel.count(), 3);
    }

    #[test]
    fn test_selection_clear() {
        let mut sel = Selection::default();
        sel.add(1);
        sel.add(2);
        sel.clear();
        assert_eq!(sel.count(), 0);
    }

    #[test]
    fn test_selection_single() {
        let mut sel = Selection::default();
        assert!(sel.single().is_none());

        sel.add(42);
        assert_eq!(sel.single(), Some(42));

        sel.add(43);
        assert!(sel.single().is_none());
    }

    // Undo/redo tests

    #[test]
    fn test_undo_redo_basic() {
        let mut state = EditorState::new();
        state.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
        assert_eq!(state.graph.node_count(), 1);
        assert!(state.can_undo());

        state.undo();
        assert_eq!(state.graph.node_count(), 0);
        assert!(state.can_redo());

        state.redo();
        assert_eq!(state.graph.node_count(), 1);
    }

    #[test]
    fn test_undo_clears_redo() {
        let mut state = EditorState::new();
        state.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
        state.add_node_at(NodeKind::Log, 100.0, 0.0).unwrap();
        assert_eq!(state.graph.node_count(), 2);

        state.undo();
        assert!(state.can_redo());

        // New action clears redo
        state.add_node_at(NodeKind::Add, 200.0, 0.0).unwrap();
        assert!(!state.can_redo());
    }

    #[test]
    fn test_undo_empty() {
        let mut state = EditorState::new();
        assert!(!state.undo());
    }

    #[test]
    fn test_redo_empty() {
        let mut state = EditorState::new();
        assert!(!state.redo());
    }

    #[test]
    fn test_undo_count() {
        let mut state = EditorState::new();
        assert_eq!(state.undo_count(), 0);
        state.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
        assert_eq!(state.undo_count(), 1);
        state.add_node_at(NodeKind::Log, 100.0, 0.0).unwrap();
        assert_eq!(state.undo_count(), 2);
    }

    #[test]
    fn test_redo_count() {
        let mut state = EditorState::new();
        state.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
        state.add_node_at(NodeKind::Log, 100.0, 0.0).unwrap();
        assert_eq!(state.redo_count(), 0);

        state.undo();
        assert_eq!(state.redo_count(), 1);
        state.undo();
        assert_eq!(state.redo_count(), 2);
    }

    // Delete tests

    #[test]
    fn test_delete_selected() {
        let mut state = EditorState::new();
        let id = state.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
        assert_eq!(state.graph.node_count(), 1);

        state.selection.select_single(id);
        state.delete_selected();
        assert_eq!(state.graph.node_count(), 0);
        assert_eq!(state.selection.count(), 0);
    }

    #[test]
    fn test_delete_empty_selection() {
        let mut state = EditorState::new();
        state.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
        state.selection.clear();
        state.delete_selected();
        // Should not delete anything
        assert_eq!(state.graph.node_count(), 1);
    }

    // Connection tests

    #[test]
    fn test_can_connect_ports_compatible() {
        let mut state = EditorState::new();
        let event_id = state.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
        let log_id = state.add_node_at(NodeKind::Log, 100.0, 0.0).unwrap();

        let exec_out = state.graph.get_node(event_id).unwrap().find_output("exec").unwrap().id;
        let exec_in = state.graph.get_node(log_id).unwrap().find_input("exec").unwrap().id;

        assert!(state.can_connect_ports(event_id, exec_out, log_id, exec_in));
    }

    #[test]
    fn test_can_connect_ports_self_connection() {
        let mut state = EditorState::new();
        let id = state.add_node_at(NodeKind::Add, 0.0, 0.0).unwrap();
        let out_port = state.graph.get_node(id).unwrap().outputs[0].id;
        let in_port = state.graph.get_node(id).unwrap().inputs[0].id;

        assert!(!state.can_connect_ports(id, out_port, id, in_port));
    }

    #[test]
    fn test_can_connect_ports_incompatible() {
        let mut state = EditorState::new();
        let event_id = state.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
        let branch_id = state.add_node_at(NodeKind::Branch, 100.0, 0.0).unwrap();

        let exec_out = state.graph.get_node(event_id).unwrap().find_output("exec").unwrap().id;
        let cond_in = state.graph.get_node(branch_id).unwrap().find_input("condition").unwrap().id;

        // Flow -> Bool is incompatible
        assert!(!state.can_connect_ports(event_id, exec_out, branch_id, cond_in));
    }

    #[test]
    fn test_can_connect_ports_same_direction() {
        let mut state = EditorState::new();
        let add1 = state.add_node_at(NodeKind::Add, 0.0, 0.0).unwrap();
        let add2 = state.add_node_at(NodeKind::Add, 100.0, 0.0).unwrap();

        let out1 = state.graph.get_node(add1).unwrap().outputs[0].id;
        let out2 = state.graph.get_node(add2).unwrap().outputs[0].id;

        // output -> output is invalid
        assert!(!state.can_connect_ports(add1, out1, add2, out2));
    }

    // Connect/disconnect via editor state

    #[test]
    fn test_connect_via_state() {
        let mut state = EditorState::new();
        let event_id = state.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
        let log_id = state.add_node_at(NodeKind::Log, 100.0, 0.0).unwrap();

        let exec_out = state.graph.get_node(event_id).unwrap().find_output("exec").unwrap().id;
        let exec_in = state.graph.get_node(log_id).unwrap().find_input("exec").unwrap().id;

        let conn_id = state.connect(event_id, exec_out, log_id, exec_in).unwrap();
        assert_eq!(state.graph.connection_count(), 1);

        state.disconnect(conn_id).unwrap();
        assert_eq!(state.graph.connection_count(), 0);
    }

    // Copy to clipboard

    #[test]
    fn test_copy_selected() {
        let mut state = EditorState::new();
        let id = state.add_node_at(NodeKind::OnStart, 0.0, 0.0).unwrap();
        state.selection.select_single(id);
        state.copy_selected();
        assert!(state.clipboard.content.is_some());
    }

    #[test]
    fn test_copy_empty_selection() {
        let mut state = EditorState::new();
        state.copy_selected();
        assert!(state.clipboard.content.is_none());
    }

    // Status message

    #[test]
    fn test_status_message() {
        let mut state = EditorState::new();
        state.set_status("Hello", false);
        assert!(state.status.is_some());
        assert_eq!(state.status.as_ref().unwrap().text, "Hello");
        assert!(!state.status.as_ref().unwrap().is_error);

        state.clear_status();
        assert!(state.status.is_none());
    }

    // Editor mode transitions

    #[test]
    fn test_editor_mode_connecting() {
        let mut state = EditorState::new();
        state.mode = EditorMode::Connecting {
            from_node: 1,
            from_port: 2,
            from_direction: PortDirection::Output,
        };
        assert!(matches!(state.mode, EditorMode::Connecting { .. }));

        state.mode = EditorMode::Normal;
        assert_eq!(state.mode, EditorMode::Normal);
    }

    #[test]
    fn test_editor_mode_box_selecting() {
        let mut state = EditorState::new();
        state.mode = EditorMode::BoxSelecting;
        assert_eq!(state.mode, EditorMode::BoxSelecting);
    }

    // Data type colors

    #[test]
    fn test_data_type_colors_unique() {
        let types = [
            DataType::Flow,
            DataType::Bool,
            DataType::Int,
            DataType::Float,
            DataType::String,
            DataType::Vec3,
            DataType::Entity,
            DataType::Any,
        ];
        let colors: Vec<_> = types.iter().map(|dt| EditorState::data_type_color(*dt)).collect();
        // Each type should have a different color
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(colors[i], colors[j], "colors for {:?} and {:?} should differ", types[i], types[j]);
            }
        }
    }

    // Default values

    #[test]
    fn test_default_value_for_type() {
        assert_eq!(EditorState::default_value_for_type(DataType::Bool), Value::Bool(false));
        assert_eq!(EditorState::default_value_for_type(DataType::Int), Value::Int(0));
        assert_eq!(EditorState::default_value_for_type(DataType::Float), Value::Float(0.0));
        assert_eq!(EditorState::default_value_for_type(DataType::String), Value::String(String::new()));
        assert_eq!(
            EditorState::default_value_for_type(DataType::Vec3),
            Value::Vec3 { x: 0.0, y: 0.0, z: 0.0 }
        );
        assert_eq!(EditorState::default_value_for_type(DataType::Entity), Value::Entity(0));
        assert_eq!(EditorState::default_value_for_type(DataType::Flow), Value::None);
        assert_eq!(EditorState::default_value_for_type(DataType::Any), Value::None);
    }
}
