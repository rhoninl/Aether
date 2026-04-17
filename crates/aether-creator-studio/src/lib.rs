//! Creator Studio contracts for editing modes, terrain operations, and manifest patching.

pub mod editor;
pub mod manifest;
pub mod preview;
pub mod terrain;
pub mod tools;

pub use editor::{EditorEvent, EditorMode, ErrorReport, StudioManifestDraft};
pub use manifest::{
    ManifestEdit, PhysicsSettingsPatch, SpawnPointEdit, TerrainEdit, WorldManifest,
    WorldManifestPatch,
};
pub use preview::{HotReloadAction, LivePreviewError, PreviewFrame};
pub use terrain::{PaintStroke, SculptBrush, TerrainBrush, TerrainTool};
pub use tools::{GizmoMode, PropEdit, PropPlacement, ScriptMode};
