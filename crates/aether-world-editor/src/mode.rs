//! Mode manager for the world editor.
//!
//! Manages transitions between Editor, PlayTest, and Play modes, and tracks
//! the world dimension (2D vs 3D).

use serde::{Deserialize, Serialize};

use crate::error::ModeError;

/// World editing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldMode {
    /// Full editing capabilities. Editor overlay visible.
    Editor,
    /// Play-testing within the editor. Can toggle back to Editor.
    PlayTest,
    /// Published world. No editing. Players join here.
    Play,
}

/// World dimension type. Chosen at creation time and immutable after.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorldDimension {
    /// 2D world -- orthographic camera, 2D physics, sprite-based rendering.
    TwoD,
    /// 3D world -- perspective camera, 3D physics, mesh-based rendering.
    ThreeD,
}

/// Manages mode transitions and enforces valid state changes.
#[derive(Debug, Clone)]
pub struct ModeManager {
    current_mode: WorldMode,
    dimension: WorldDimension,
    previous_mode: Option<WorldMode>,
}

impl ModeManager {
    /// Create a new mode manager in Editor mode with the given dimension.
    pub fn new(dimension: WorldDimension) -> Self {
        Self {
            current_mode: WorldMode::Editor,
            dimension,
            previous_mode: None,
        }
    }

    /// Create a mode manager for players joining -- always starts in Play mode.
    pub fn new_play() -> Self {
        Self {
            current_mode: WorldMode::Play,
            dimension: WorldDimension::ThreeD,
            previous_mode: None,
        }
    }

    /// Returns the current mode.
    pub fn current(&self) -> WorldMode {
        self.current_mode
    }

    /// Returns the world dimension.
    pub fn dimension(&self) -> WorldDimension {
        self.dimension
    }

    /// Transition to Editor mode.
    ///
    /// Valid from: PlayTest, Play (creator re-entering edit mode).
    /// Invalid from: Editor (already there).
    pub fn enter_editor(&mut self) -> Result<(), ModeError> {
        match self.current_mode {
            WorldMode::Editor => Err(ModeError::InvalidTransition {
                from: WorldMode::Editor,
                to: WorldMode::Editor,
            }),
            WorldMode::PlayTest | WorldMode::Play => {
                self.previous_mode = Some(self.current_mode);
                self.current_mode = WorldMode::Editor;
                Ok(())
            }
        }
    }

    /// Transition to PlayTest mode.
    ///
    /// Valid from: Editor.
    /// Invalid from: Play, PlayTest.
    pub fn enter_play_test(&mut self) -> Result<(), ModeError> {
        match self.current_mode {
            WorldMode::Editor => {
                self.previous_mode = Some(self.current_mode);
                self.current_mode = WorldMode::PlayTest;
                Ok(())
            }
            other => Err(ModeError::InvalidTransition {
                from: other,
                to: WorldMode::PlayTest,
            }),
        }
    }

    /// Transition to Play mode.
    ///
    /// Valid from: Editor (publish and switch to play).
    /// Invalid from: PlayTest (must go through Editor first), Play (already there).
    pub fn enter_play(&mut self) -> Result<(), ModeError> {
        match self.current_mode {
            WorldMode::Editor => {
                self.previous_mode = Some(self.current_mode);
                self.current_mode = WorldMode::Play;
                Ok(())
            }
            other => Err(ModeError::InvalidTransition {
                from: other,
                to: WorldMode::Play,
            }),
        }
    }

    /// Exit PlayTest mode, returning to Editor.
    ///
    /// Only valid from PlayTest mode.
    pub fn exit_play_test(&mut self) -> Result<(), ModeError> {
        match self.current_mode {
            WorldMode::PlayTest => {
                self.previous_mode = Some(WorldMode::PlayTest);
                self.current_mode = WorldMode::Editor;
                Ok(())
            }
            other => Err(ModeError::InvalidTransition {
                from: other,
                to: WorldMode::Editor,
            }),
        }
    }

    /// Returns true if the current mode allows editing.
    pub fn can_edit(&self) -> bool {
        self.current_mode == WorldMode::Editor
    }

    /// Returns true if the current mode allows playing.
    pub fn can_play(&self) -> bool {
        matches!(self.current_mode, WorldMode::PlayTest | WorldMode::Play)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_manager_starts_in_editor_mode() {
        let mgr = ModeManager::new(WorldDimension::ThreeD);
        assert_eq!(mgr.current(), WorldMode::Editor);
    }

    #[test]
    fn new_manager_preserves_dimension() {
        let mgr_2d = ModeManager::new(WorldDimension::TwoD);
        assert_eq!(mgr_2d.dimension(), WorldDimension::TwoD);

        let mgr_3d = ModeManager::new(WorldDimension::ThreeD);
        assert_eq!(mgr_3d.dimension(), WorldDimension::ThreeD);
    }

    #[test]
    fn new_play_starts_in_play_mode() {
        let mgr = ModeManager::new_play();
        assert_eq!(mgr.current(), WorldMode::Play);
    }

    #[test]
    fn editor_to_play_test_valid() {
        let mut mgr = ModeManager::new(WorldDimension::ThreeD);
        assert!(mgr.enter_play_test().is_ok());
        assert_eq!(mgr.current(), WorldMode::PlayTest);
    }

    #[test]
    fn play_test_to_editor_valid() {
        let mut mgr = ModeManager::new(WorldDimension::ThreeD);
        mgr.enter_play_test().unwrap();
        assert!(mgr.enter_editor().is_ok());
        assert_eq!(mgr.current(), WorldMode::Editor);
    }

    #[test]
    fn editor_to_play_valid() {
        let mut mgr = ModeManager::new(WorldDimension::ThreeD);
        assert!(mgr.enter_play().is_ok());
        assert_eq!(mgr.current(), WorldMode::Play);
    }

    #[test]
    fn play_to_editor_valid() {
        let mut mgr = ModeManager::new(WorldDimension::ThreeD);
        mgr.enter_play().unwrap();
        assert!(mgr.enter_editor().is_ok());
        assert_eq!(mgr.current(), WorldMode::Editor);
    }

    #[test]
    fn play_to_play_test_invalid() {
        let mut mgr = ModeManager::new(WorldDimension::ThreeD);
        mgr.enter_play().unwrap();
        let err = mgr.enter_play_test().unwrap_err();
        match err {
            ModeError::InvalidTransition { from, to } => {
                assert_eq!(from, WorldMode::Play);
                assert_eq!(to, WorldMode::PlayTest);
            }
            _ => panic!("expected InvalidTransition"),
        }
    }

    #[test]
    fn play_test_to_play_invalid() {
        let mut mgr = ModeManager::new(WorldDimension::ThreeD);
        mgr.enter_play_test().unwrap();
        let err = mgr.enter_play().unwrap_err();
        match err {
            ModeError::InvalidTransition { from, to } => {
                assert_eq!(from, WorldMode::PlayTest);
                assert_eq!(to, WorldMode::Play);
            }
            _ => panic!("expected InvalidTransition"),
        }
    }

    #[test]
    fn can_edit_true_only_in_editor() {
        let mut mgr = ModeManager::new(WorldDimension::ThreeD);
        assert!(mgr.can_edit());

        mgr.enter_play_test().unwrap();
        assert!(!mgr.can_edit());

        mgr.exit_play_test().unwrap();
        mgr.enter_play().unwrap();
        assert!(!mgr.can_edit());
    }

    #[test]
    fn can_play_true_in_play_test_and_play() {
        let mut mgr = ModeManager::new(WorldDimension::ThreeD);
        assert!(!mgr.can_play());

        mgr.enter_play_test().unwrap();
        assert!(mgr.can_play());

        mgr.exit_play_test().unwrap();
        mgr.enter_play().unwrap();
        assert!(mgr.can_play());
    }

    #[test]
    fn exit_play_test_returns_to_editor() {
        let mut mgr = ModeManager::new(WorldDimension::ThreeD);
        mgr.enter_play_test().unwrap();
        assert!(mgr.exit_play_test().is_ok());
        assert_eq!(mgr.current(), WorldMode::Editor);
    }

    #[test]
    fn exit_play_test_from_editor_invalid() {
        let mut mgr = ModeManager::new(WorldDimension::ThreeD);
        assert!(mgr.exit_play_test().is_err());
    }

    #[test]
    fn exit_play_test_from_play_invalid() {
        let mut mgr = ModeManager::new(WorldDimension::ThreeD);
        mgr.enter_play().unwrap();
        assert!(mgr.exit_play_test().is_err());
    }

    #[test]
    fn editor_to_editor_invalid() {
        let mut mgr = ModeManager::new(WorldDimension::ThreeD);
        assert!(mgr.enter_editor().is_err());
    }

    #[test]
    fn full_round_trip_editor_playtest_editor_play_editor() {
        let mut mgr = ModeManager::new(WorldDimension::TwoD);
        assert_eq!(mgr.current(), WorldMode::Editor);

        mgr.enter_play_test().unwrap();
        assert_eq!(mgr.current(), WorldMode::PlayTest);

        mgr.exit_play_test().unwrap();
        assert_eq!(mgr.current(), WorldMode::Editor);

        mgr.enter_play().unwrap();
        assert_eq!(mgr.current(), WorldMode::Play);

        mgr.enter_editor().unwrap();
        assert_eq!(mgr.current(), WorldMode::Editor);
    }
}
