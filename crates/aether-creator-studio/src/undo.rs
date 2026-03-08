//! Undo/Redo system using the command pattern.

use crate::scene::EditorScene;
use std::fmt;

const DEFAULT_MAX_HISTORY: usize = 100;

/// Errors from editor command execution.
#[derive(Debug)]
pub enum CommandError {
    /// The target object was not found.
    ObjectNotFound(u64),
    /// The terrain data is missing.
    NoTerrain,
    /// A validation constraint was violated.
    ValidationError(String),
    /// Generic error.
    Other(String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObjectNotFound(id) => write!(f, "object not found: {id}"),
            Self::NoTerrain => write!(f, "no terrain data"),
            Self::ValidationError(msg) => write!(f, "validation error: {msg}"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for CommandError {}

pub type CommandResult<T> = Result<T, CommandError>;

/// Trait for undoable editor commands.
pub trait EditorCommand: Send + Sync {
    /// Execute the command, mutating the scene.
    fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()>;

    /// Undo the command, reverting scene changes.
    fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<()>;

    /// Human-readable description of this command.
    fn description(&self) -> &str;
}

/// Stack of undoable/redoable commands.
pub struct UndoStack {
    undo_stack: Vec<Box<dyn EditorCommand>>,
    redo_stack: Vec<Box<dyn EditorCommand>>,
    max_history: usize,
}

impl UndoStack {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history: DEFAULT_MAX_HISTORY,
        }
    }

    pub fn with_max_history(max_history: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history,
        }
    }

    /// Execute a command and push it onto the undo stack.
    /// Clears the redo stack (standard undo/redo semantics).
    pub fn push(
        &mut self,
        mut cmd: Box<dyn EditorCommand>,
        scene: &mut EditorScene,
    ) -> CommandResult<()> {
        cmd.execute(scene)?;
        self.undo_stack.push(cmd);
        self.redo_stack.clear();

        // Evict oldest if over capacity
        if self.undo_stack.len() > self.max_history {
            self.undo_stack.remove(0);
        }
        Ok(())
    }

    /// Undo the most recent command.
    pub fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<bool> {
        if let Some(mut cmd) = self.undo_stack.pop() {
            cmd.undo(scene)?;
            self.redo_stack.push(cmd);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Redo the most recently undone command.
    pub fn redo(&mut self, scene: &mut EditorScene) -> CommandResult<bool> {
        if let Some(mut cmd) = self.redo_stack.pop() {
            cmd.execute(scene)?;
            self.undo_stack.push(cmd);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Clear all undo and redo history.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trivial test command that increments/decrements a counter stored
    /// as the number of objects in the scene.
    struct AddDummyObject {
        added_id: Option<u64>,
    }

    impl AddDummyObject {
        fn new() -> Self {
            Self { added_id: None }
        }
    }

    impl EditorCommand for AddDummyObject {
        fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
            use crate::scene::*;
            let id = scene.next_id();
            scene.add_object(SceneObject {
                id,
                name: format!("dummy_{id}"),
                kind: ObjectKind::Prop {
                    template: "test".into(),
                },
                position: Position::zero(),
                rotation: Rotation::zero(),
                scale: Scale::one(),
            });
            self.added_id = Some(id);
            Ok(())
        }

        fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
            if let Some(id) = self.added_id {
                scene.remove_object(id);
            }
            Ok(())
        }

        fn description(&self) -> &str {
            "add dummy object"
        }
    }

    fn make_scene() -> EditorScene {
        EditorScene::new()
    }

    #[test]
    fn test_push_executes_command() {
        let mut scene = make_scene();
        let mut stack = UndoStack::new();
        stack
            .push(Box::new(AddDummyObject::new()), &mut scene)
            .unwrap();
        assert_eq!(scene.objects.len(), 1);
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_undo_reverts() {
        let mut scene = make_scene();
        let mut stack = UndoStack::new();
        stack
            .push(Box::new(AddDummyObject::new()), &mut scene)
            .unwrap();
        assert_eq!(scene.objects.len(), 1);

        let undone = stack.undo(&mut scene).unwrap();
        assert!(undone);
        assert_eq!(scene.objects.len(), 0);
        assert!(!stack.can_undo());
        assert!(stack.can_redo());
    }

    #[test]
    fn test_redo_reapplies() {
        let mut scene = make_scene();
        let mut stack = UndoStack::new();
        stack
            .push(Box::new(AddDummyObject::new()), &mut scene)
            .unwrap();
        stack.undo(&mut scene).unwrap();
        assert_eq!(scene.objects.len(), 0);

        let redone = stack.redo(&mut scene).unwrap();
        assert!(redone);
        assert_eq!(scene.objects.len(), 1);
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_new_push_clears_redo() {
        let mut scene = make_scene();
        let mut stack = UndoStack::new();
        stack
            .push(Box::new(AddDummyObject::new()), &mut scene)
            .unwrap();
        stack.undo(&mut scene).unwrap();
        assert!(stack.can_redo());

        // New push should clear redo
        stack
            .push(Box::new(AddDummyObject::new()), &mut scene)
            .unwrap();
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_max_history_eviction() {
        let mut scene = make_scene();
        let mut stack = UndoStack::with_max_history(3);

        for _ in 0..5 {
            stack
                .push(Box::new(AddDummyObject::new()), &mut scene)
                .unwrap();
        }
        assert_eq!(stack.undo_count(), 3);
    }

    #[test]
    fn test_undo_empty_returns_false() {
        let mut scene = make_scene();
        let mut stack = UndoStack::new();
        let result = stack.undo(&mut scene).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_redo_empty_returns_false() {
        let mut scene = make_scene();
        let mut stack = UndoStack::new();
        let result = stack.redo(&mut scene).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_clear() {
        let mut scene = make_scene();
        let mut stack = UndoStack::new();
        stack
            .push(Box::new(AddDummyObject::new()), &mut scene)
            .unwrap();
        stack
            .push(Box::new(AddDummyObject::new()), &mut scene)
            .unwrap();
        stack.undo(&mut scene).unwrap();

        assert!(stack.can_undo());
        assert!(stack.can_redo());

        stack.clear();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_count(), 0);
        assert_eq!(stack.redo_count(), 0);
    }

    #[test]
    fn test_multiple_undo_redo_sequence() {
        let mut scene = make_scene();
        let mut stack = UndoStack::new();

        // Push 3 commands
        for _ in 0..3 {
            stack
                .push(Box::new(AddDummyObject::new()), &mut scene)
                .unwrap();
        }
        assert_eq!(scene.objects.len(), 3);

        // Undo all
        stack.undo(&mut scene).unwrap();
        assert_eq!(scene.objects.len(), 2);
        stack.undo(&mut scene).unwrap();
        assert_eq!(scene.objects.len(), 1);
        stack.undo(&mut scene).unwrap();
        assert_eq!(scene.objects.len(), 0);

        // Redo all
        stack.redo(&mut scene).unwrap();
        assert_eq!(scene.objects.len(), 1);
        stack.redo(&mut scene).unwrap();
        assert_eq!(scene.objects.len(), 2);
        stack.redo(&mut scene).unwrap();
        assert_eq!(scene.objects.len(), 3);
    }

    #[test]
    fn test_command_description() {
        let cmd = AddDummyObject::new();
        assert_eq!(cmd.description(), "add dummy object");
    }

    #[test]
    fn test_command_error_display() {
        let e = CommandError::ObjectNotFound(42);
        assert_eq!(format!("{e}"), "object not found: 42");

        let e = CommandError::NoTerrain;
        assert_eq!(format!("{e}"), "no terrain data");

        let e = CommandError::ValidationError("bad input".into());
        assert!(format!("{e}").contains("bad input"));

        let e = CommandError::Other("something".into());
        assert_eq!(format!("{e}"), "something");
    }
}
