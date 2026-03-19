//! Editor state tracking -- selected entities, active tool, grid settings, etc.

use crate::mode::{ModeManager, WorldDimension};

/// Tools available in the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorTool {
    // Shared tools
    Select,
    Translate,
    Rotate,
    Scale,
    Place,
    Delete,
    // 3D-specific tools
    Sculpt,
    Paint,
    // 2D-specific tools
    TilePaint,
    TileErase,
    TileFill,
}

const DEFAULT_GRID_SIZE: f32 = 1.0;

/// Current editor state.
#[derive(Debug, Clone)]
pub struct EditorState {
    pub mode_manager: ModeManager,
    pub selected_entities: Vec<String>,
    pub active_tool: EditorTool,
    pub grid_visible: bool,
    pub grid_snap: bool,
    pub grid_size: f32,
    pub debug_physics: bool,
}

impl EditorState {
    /// Create a new editor state for the given world dimension.
    pub fn new(dimension: WorldDimension) -> Self {
        Self {
            mode_manager: ModeManager::new(dimension),
            selected_entities: Vec::new(),
            active_tool: EditorTool::Select,
            grid_visible: true,
            grid_snap: true,
            grid_size: DEFAULT_GRID_SIZE,
            debug_physics: false,
        }
    }

    /// Select an entity by ID. No-op if already selected.
    pub fn select_entity(&mut self, id: &str) {
        if !self.selected_entities.iter().any(|e| e == id) {
            self.selected_entities.push(id.to_string());
        }
    }

    /// Deselect an entity by ID. No-op if not selected.
    pub fn deselect_entity(&mut self, id: &str) {
        self.selected_entities.retain(|e| e != id);
    }

    /// Clear all selected entities.
    pub fn clear_selection(&mut self) {
        self.selected_entities.clear();
    }

    /// Returns true if the entity is currently selected.
    pub fn is_selected(&self, id: &str) -> bool {
        self.selected_entities.iter().any(|e| e == id)
    }

    /// Set the active editor tool.
    pub fn set_tool(&mut self, tool: EditorTool) {
        self.active_tool = tool;
    }

    /// Toggle grid visibility.
    pub fn toggle_grid(&mut self) {
        self.grid_visible = !self.grid_visible;
    }

    /// Toggle grid snapping.
    pub fn toggle_grid_snap(&mut self) {
        self.grid_snap = !self.grid_snap;
    }

    /// Toggle physics debug visualization.
    pub fn toggle_debug_physics(&mut self) {
        self.debug_physics = !self.debug_physics;
    }

    /// Returns the tools available for the current world dimension.
    pub fn available_tools(&self) -> Vec<EditorTool> {
        let mut tools = vec![
            EditorTool::Select,
            EditorTool::Translate,
            EditorTool::Rotate,
            EditorTool::Scale,
            EditorTool::Place,
            EditorTool::Delete,
        ];

        match self.mode_manager.dimension() {
            WorldDimension::ThreeD => {
                tools.push(EditorTool::Sculpt);
                tools.push(EditorTool::Paint);
            }
            WorldDimension::TwoD => {
                tools.push(EditorTool::TilePaint);
                tools.push(EditorTool::TileErase);
                tools.push(EditorTool::TileFill);
            }
        }

        tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_starts_with_select_tool() {
        let state = EditorState::new(WorldDimension::ThreeD);
        assert_eq!(state.active_tool, EditorTool::Select);
    }

    #[test]
    fn new_state_defaults() {
        let state = EditorState::new(WorldDimension::TwoD);
        assert!(state.grid_visible);
        assert!(state.grid_snap);
        assert!(!state.debug_physics);
        assert!(state.selected_entities.is_empty());
    }

    #[test]
    fn select_entity() {
        let mut state = EditorState::new(WorldDimension::ThreeD);
        state.select_entity("entity-1");
        assert!(state.is_selected("entity-1"));
        assert!(!state.is_selected("entity-2"));
    }

    #[test]
    fn select_entity_idempotent() {
        let mut state = EditorState::new(WorldDimension::ThreeD);
        state.select_entity("entity-1");
        state.select_entity("entity-1");
        assert_eq!(state.selected_entities.len(), 1);
    }

    #[test]
    fn deselect_entity() {
        let mut state = EditorState::new(WorldDimension::ThreeD);
        state.select_entity("entity-1");
        state.select_entity("entity-2");
        state.deselect_entity("entity-1");
        assert!(!state.is_selected("entity-1"));
        assert!(state.is_selected("entity-2"));
    }

    #[test]
    fn deselect_nonexistent_is_noop() {
        let mut state = EditorState::new(WorldDimension::ThreeD);
        state.deselect_entity("nonexistent");
        assert!(state.selected_entities.is_empty());
    }

    #[test]
    fn clear_selection() {
        let mut state = EditorState::new(WorldDimension::ThreeD);
        state.select_entity("entity-1");
        state.select_entity("entity-2");
        state.clear_selection();
        assert!(state.selected_entities.is_empty());
    }

    #[test]
    fn set_tool() {
        let mut state = EditorState::new(WorldDimension::ThreeD);
        state.set_tool(EditorTool::Translate);
        assert_eq!(state.active_tool, EditorTool::Translate);
    }

    #[test]
    fn toggle_grid() {
        let mut state = EditorState::new(WorldDimension::ThreeD);
        assert!(state.grid_visible);
        state.toggle_grid();
        assert!(!state.grid_visible);
        state.toggle_grid();
        assert!(state.grid_visible);
    }

    #[test]
    fn toggle_grid_snap() {
        let mut state = EditorState::new(WorldDimension::ThreeD);
        assert!(state.grid_snap);
        state.toggle_grid_snap();
        assert!(!state.grid_snap);
        state.toggle_grid_snap();
        assert!(state.grid_snap);
    }

    #[test]
    fn toggle_debug_physics() {
        let mut state = EditorState::new(WorldDimension::ThreeD);
        assert!(!state.debug_physics);
        state.toggle_debug_physics();
        assert!(state.debug_physics);
        state.toggle_debug_physics();
        assert!(!state.debug_physics);
    }

    #[test]
    fn available_tools_3d_include_sculpt_and_paint() {
        let state = EditorState::new(WorldDimension::ThreeD);
        let tools = state.available_tools();
        assert!(tools.contains(&EditorTool::Sculpt));
        assert!(tools.contains(&EditorTool::Paint));
        // Should NOT contain 2D-specific tools.
        assert!(!tools.contains(&EditorTool::TilePaint));
        assert!(!tools.contains(&EditorTool::TileErase));
        assert!(!tools.contains(&EditorTool::TileFill));
    }

    #[test]
    fn available_tools_2d_include_tile_tools() {
        let state = EditorState::new(WorldDimension::TwoD);
        let tools = state.available_tools();
        assert!(tools.contains(&EditorTool::TilePaint));
        assert!(tools.contains(&EditorTool::TileErase));
        assert!(tools.contains(&EditorTool::TileFill));
        // Should NOT contain 3D-specific tools.
        assert!(!tools.contains(&EditorTool::Sculpt));
        assert!(!tools.contains(&EditorTool::Paint));
    }

    #[test]
    fn available_tools_shared_tools_present_in_both() {
        let shared = vec![
            EditorTool::Select,
            EditorTool::Translate,
            EditorTool::Rotate,
            EditorTool::Scale,
            EditorTool::Place,
            EditorTool::Delete,
        ];
        for dim in [WorldDimension::TwoD, WorldDimension::ThreeD] {
            let state = EditorState::new(dim);
            let tools = state.available_tools();
            for tool in &shared {
                assert!(tools.contains(tool), "missing {tool:?} in {dim:?}");
            }
        }
    }

    #[test]
    fn multiple_entities_selected() {
        let mut state = EditorState::new(WorldDimension::ThreeD);
        state.select_entity("a");
        state.select_entity("b");
        state.select_entity("c");
        assert_eq!(state.selected_entities.len(), 3);
        assert!(state.is_selected("a"));
        assert!(state.is_selected("b"));
        assert!(state.is_selected("c"));
    }
}
