//! Creator Studio contracts and editor logic for terrain, props, lighting,
//! manifest editing, undo/redo, and object selection.

pub mod editor;
pub mod lighting_editor;
pub mod manifest;
pub mod manifest_editor;
pub mod preview;
pub mod prop_editor;
pub mod scene;
pub mod selection;
pub mod terrain;
pub mod terrain_editor;
pub mod tools;
pub mod undo;
pub mod visual_script;

// Re-export existing contract types.
pub use editor::{EditorEvent, EditorMode, ErrorReport, StudioManifestDraft};
pub use manifest::{ManifestEdit, SpawnPointEdit, TerrainEdit, WorldManifestPatch};
pub use preview::{HotReloadAction, LivePreviewError, PreviewFrame};
pub use terrain::{PaintStroke, SculptBrush, TerrainBrush, TerrainTool};
pub use tools::{GizmoMode, PropPlacement, ScriptMode};

// Re-export editor logic types.
pub use lighting_editor::{
    AmbientSettings, LightProbe, LightingState, PlaceLightProbeCommand,
    RemoveLightProbeCommand, SetAmbientCommand,
};
pub use manifest_editor::{
    apply_patch, create_default_manifest, validate_manifest, ManifestPatch,
    ManifestValidationError, PhysicsSettings, SpawnPoint, WorldManifest,
};
pub use prop_editor::{
    DeletePropCommand, MovePropCommand, PlacePropCommand, RotatePropCommand,
    ScalePropCommand,
};
pub use scene::{EditorScene, ObjectId, ObjectKind, Position, Rotation, Scale, SceneObject};
pub use selection::{ClearSelectionCommand, DeselectCommand, SelectCommand, Selection};
pub use terrain_editor::{
    PaintCommand, PaintLayer, PlaceVegetationCommand, SculptCommand, TerrainData,
};
pub use undo::{CommandError, CommandResult, EditorCommand, UndoStack};

// Re-export visual scripting types.
pub use visual_script::{
    compile, all_templates, instantiate_template, validate_graph, compute_layout, apply_layout,
    BinaryOp, CompileError, CompiledScript, Connection, ConnectionId, DataType,
    EngineApi, GraphError, IrInstruction, LayoutConfig, LayoutResult, NoOpApi, Node, NodeGraph,
    NodeId, NodeKind, Port, PortDirection, PortId, RecordingApi, RuntimeError, ScriptVm,
    Severity, TemplateKind, ValidationDiagnostic, ValidationResult, Value, VmConfig,
};
