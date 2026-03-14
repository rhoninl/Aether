//! Visual Script Editor GUI for the Aether VR Engine.
//!
//! Provides an interactive node-based visual programming editor built with egui/eframe.
//! Integrates with `aether-creator-studio`'s visual scripting backend for graph manipulation,
//! validation, compilation, and auto-layout.
//!
//! # Architecture
//!
//! - [`app::VisualEditorApp`] - Main eframe application
//! - [`state::EditorState`] - Editor state (graph, selection, undo/redo, clipboard)
//! - [`canvas`] - Pan/zoom coordinate transforms and grid rendering
//! - [`node_renderer`] - Node box and port rendering
//! - [`connection_renderer`] - Bezier curve connections
//! - [`palette`] - Sidebar with categorized node list
//! - [`properties`] - Property panel for selected node
//! - [`interaction`] - Input handling (selection, dragging, connecting)
//! - [`toolbar`] - Top bar (compile, validate, layout, undo/redo, zoom)
//! - [`minimap`] - Graph overview with viewport indicator

pub mod app;
pub mod canvas;
pub mod connection_renderer;
pub mod interaction;
pub mod minimap;
pub mod node_renderer;
pub mod palette;
pub mod properties;
pub mod state;
pub mod toolbar;

// Re-export key types for convenient use.
pub use app::VisualEditorApp;
pub use canvas::ViewTransform;
pub use state::{EditorMode, EditorState, Selection};
