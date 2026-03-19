//! In-world editor for Aether -- edit 2D and 3D worlds from within.
//!
//! This crate provides mode management, version tracking, world project I/O,
//! and editor state for the Aether in-world editing experience.

pub mod editor_state;
pub mod error;
pub mod mode;
pub mod project;
pub mod version;

// Re-export primary types for convenience.
pub use editor_state::{EditorState, EditorTool};
pub use error::{ModeError, ProjectError, VersionError};
pub use mode::{ModeManager, WorldDimension, WorldMode};
pub use project::{
    load_project, load_version_history, save_manifest, save_version_history,
    scaffold_project, WorldProject, WorldProjectManifest,
};
pub use version::{
    bump_version, deserialize_version_history, serialize_version_history, BumpKind,
    VersionHistory, VersionRecord,
};
