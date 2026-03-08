//! Object selection system for the editor.

use std::collections::HashSet;

use crate::scene::{EditorScene, ObjectId};
use crate::undo::{CommandResult, EditorCommand};

/// Tracks which objects are currently selected.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    selected: HashSet<ObjectId>,
}

impl Selection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Select a single object (adds to selection).
    pub fn select(&mut self, id: ObjectId) {
        self.selected.insert(id);
    }

    /// Deselect a single object.
    pub fn deselect(&mut self, id: ObjectId) {
        self.selected.remove(&id);
    }

    /// Toggle selection of a single object.
    pub fn toggle(&mut self, id: ObjectId) {
        if self.selected.contains(&id) {
            self.selected.remove(&id);
        } else {
            self.selected.insert(id);
        }
    }

    /// Select multiple objects at once.
    pub fn select_all(&mut self, ids: &[ObjectId]) {
        for &id in ids {
            self.selected.insert(id);
        }
    }

    /// Clear the entire selection.
    pub fn clear(&mut self) -> HashSet<ObjectId> {
        std::mem::take(&mut self.selected)
    }

    /// Replace the selection with the given set.
    pub fn set(&mut self, ids: HashSet<ObjectId>) {
        self.selected = ids;
    }

    pub fn is_selected(&self, id: ObjectId) -> bool {
        self.selected.contains(&id)
    }

    pub fn count(&self) -> usize {
        self.selected.len()
    }

    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    /// Get a snapshot of the current selection.
    pub fn snapshot(&self) -> HashSet<ObjectId> {
        self.selected.clone()
    }

    /// Iterate over selected object ids.
    pub fn iter(&self) -> impl Iterator<Item = &ObjectId> {
        self.selected.iter()
    }
}

/// Command that selects a single object.
pub struct SelectCommand {
    id: ObjectId,
    was_selected: bool,
}

impl SelectCommand {
    pub fn new(id: ObjectId) -> Self {
        Self {
            id,
            was_selected: false,
        }
    }
}

impl EditorCommand for SelectCommand {
    fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        self.was_selected = scene.selection.is_selected(self.id);
        scene.selection.select(self.id);
        Ok(())
    }

    fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        if !self.was_selected {
            scene.selection.deselect(self.id);
        }
        Ok(())
    }

    fn description(&self) -> &str {
        "select object"
    }
}

/// Command that deselects a single object.
pub struct DeselectCommand {
    id: ObjectId,
    was_selected: bool,
}

impl DeselectCommand {
    pub fn new(id: ObjectId) -> Self {
        Self {
            id,
            was_selected: false,
        }
    }
}

impl EditorCommand for DeselectCommand {
    fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        self.was_selected = scene.selection.is_selected(self.id);
        scene.selection.deselect(self.id);
        Ok(())
    }

    fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        if self.was_selected {
            scene.selection.select(self.id);
        }
        Ok(())
    }

    fn description(&self) -> &str {
        "deselect object"
    }
}

/// Command that clears the entire selection.
pub struct ClearSelectionCommand {
    previous: HashSet<ObjectId>,
}

impl ClearSelectionCommand {
    pub fn new() -> Self {
        Self {
            previous: HashSet::new(),
        }
    }
}

impl Default for ClearSelectionCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorCommand for ClearSelectionCommand {
    fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        self.previous = scene.selection.clear();
        Ok(())
    }

    fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        scene.selection.set(self.previous.clone());
        Ok(())
    }

    fn description(&self) -> &str {
        "clear selection"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::undo::UndoStack;

    #[test]
    fn test_select_deselect() {
        let mut sel = Selection::new();
        assert!(sel.is_empty());

        sel.select(1);
        assert!(sel.is_selected(1));
        assert_eq!(sel.count(), 1);

        sel.deselect(1);
        assert!(!sel.is_selected(1));
        assert_eq!(sel.count(), 0);
    }

    #[test]
    fn test_toggle() {
        let mut sel = Selection::new();
        sel.toggle(1);
        assert!(sel.is_selected(1));

        sel.toggle(1);
        assert!(!sel.is_selected(1));
    }

    #[test]
    fn test_select_all() {
        let mut sel = Selection::new();
        sel.select_all(&[1, 2, 3]);
        assert_eq!(sel.count(), 3);
        assert!(sel.is_selected(1));
        assert!(sel.is_selected(2));
        assert!(sel.is_selected(3));
    }

    #[test]
    fn test_clear() {
        let mut sel = Selection::new();
        sel.select_all(&[1, 2, 3]);
        let prev = sel.clear();
        assert!(sel.is_empty());
        assert_eq!(prev.len(), 3);
    }

    #[test]
    fn test_snapshot() {
        let mut sel = Selection::new();
        sel.select_all(&[10, 20]);
        let snap = sel.snapshot();
        assert!(snap.contains(&10));
        assert!(snap.contains(&20));
        assert_eq!(snap.len(), 2);
    }

    #[test]
    fn test_set() {
        let mut sel = Selection::new();
        sel.select(1);
        let mut new_set = HashSet::new();
        new_set.insert(5);
        new_set.insert(6);
        sel.set(new_set);
        assert!(!sel.is_selected(1));
        assert!(sel.is_selected(5));
        assert!(sel.is_selected(6));
    }

    #[test]
    fn test_deselect_nonexistent() {
        let mut sel = Selection::new();
        sel.deselect(99); // should not panic
        assert!(sel.is_empty());
    }

    #[test]
    fn test_select_duplicate() {
        let mut sel = Selection::new();
        sel.select(1);
        sel.select(1);
        assert_eq!(sel.count(), 1);
    }

    #[test]
    fn test_iter() {
        let mut sel = Selection::new();
        sel.select_all(&[1, 2, 3]);
        let collected: HashSet<ObjectId> = sel.iter().copied().collect();
        assert_eq!(collected.len(), 3);
    }

    // Command tests
    #[test]
    fn test_select_command_with_undo() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        assert!(!scene.selection.is_selected(42));

        stack
            .push(Box::new(SelectCommand::new(42)), &mut scene)
            .unwrap();
        assert!(scene.selection.is_selected(42));

        stack.undo(&mut scene).unwrap();
        assert!(!scene.selection.is_selected(42));
    }

    #[test]
    fn test_select_command_already_selected() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        scene.selection.select(42);

        // Select again
        stack
            .push(Box::new(SelectCommand::new(42)), &mut scene)
            .unwrap();
        assert!(scene.selection.is_selected(42));

        // Undo should NOT deselect because it was already selected
        stack.undo(&mut scene).unwrap();
        assert!(scene.selection.is_selected(42));
    }

    #[test]
    fn test_deselect_command_with_undo() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        scene.selection.select(42);

        stack
            .push(Box::new(DeselectCommand::new(42)), &mut scene)
            .unwrap();
        assert!(!scene.selection.is_selected(42));

        stack.undo(&mut scene).unwrap();
        assert!(scene.selection.is_selected(42));
    }

    #[test]
    fn test_deselect_command_not_selected() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(Box::new(DeselectCommand::new(42)), &mut scene)
            .unwrap();
        assert!(!scene.selection.is_selected(42));

        // Undo should not re-select since it wasn't selected before
        stack.undo(&mut scene).unwrap();
        assert!(!scene.selection.is_selected(42));
    }

    #[test]
    fn test_clear_selection_command_with_undo() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        scene.selection.select_all(&[1, 2, 3]);
        assert_eq!(scene.selection.count(), 3);

        stack
            .push(Box::new(ClearSelectionCommand::new()), &mut scene)
            .unwrap();
        assert!(scene.selection.is_empty());

        stack.undo(&mut scene).unwrap();
        assert_eq!(scene.selection.count(), 3);
        assert!(scene.selection.is_selected(1));
        assert!(scene.selection.is_selected(2));
        assert!(scene.selection.is_selected(3));
    }
}
