//! Creator Studio contracts and editor logic for terrain, props, lighting,
//! manifest editing, undo/redo, and object selection.

pub mod dimension;
pub mod editor;
pub mod lighting_editor;
pub mod manifest;
pub mod manifest_editor;
pub mod preview;
pub mod project;
pub mod prop_editor;
pub mod scene;
pub mod scene_serde;
pub mod selection;
pub mod terrain;
pub mod terrain_editor;
pub mod tools;
pub mod types_2d;
pub mod undo;
pub mod visual_script;

// Re-export existing contract types.
pub use editor::{EditorEvent, EditorMode, ErrorReport, StudioManifestDraft};
pub use manifest::{
    ManifestEdit, PhysicsSettingsPatch, SpawnPointEdit, TerrainEdit, WorldManifestPatch,
};
pub use preview::{HotReloadAction, LivePreviewError, PreviewFrame};
pub use terrain::{PaintStroke, SculptBrush, TerrainBrush, TerrainTool};
pub use tools::{GizmoMode, PropPlacement, ScriptMode};

// Re-export editor logic types.
pub use lighting_editor::{
    AmbientSettings, LightProbe, LightingState, PlaceLightProbeCommand, RemoveLightProbeCommand,
    SetAmbientCommand,
};
pub use manifest_editor::{
    apply_patch, create_default_manifest, validate_manifest, ManifestPatch,
    ManifestValidationError, PhysicsSettings, SpawnPoint, WorldManifest,
};
pub use prop_editor::{
    DeletePropCommand, MovePropCommand, PlacePropCommand, RotatePropCommand, ScalePropCommand,
};
pub use scene::{EditorScene, ObjectId, ObjectKind, Position, Rotation, Scale, SceneObject};
pub use selection::{ClearSelectionCommand, DeselectCommand, SelectCommand, Selection};
pub use terrain_editor::{
    PaintCommand, PaintLayer, PlaceVegetationCommand, SculptCommand, TerrainData,
};
pub use undo::{CommandError, CommandResult, EditorCommand, UndoStack};

// Re-export visual scripting types.
pub use visual_script::{
    all_templates, apply_layout, compile, compute_layout, instantiate_template, validate_graph,
    BinaryOp, CompileError, CompiledScript, Connection, ConnectionId, DataType, EngineApi,
    GraphError, IrInstruction, LayoutConfig, LayoutResult, NoOpApi, Node, NodeGraph, NodeId,
    NodeKind, Port, PortDirection, PortId, RecordingApi, RuntimeError, ScriptVm, Severity,
    TemplateKind, ValidationDiagnostic, ValidationResult, Value, VmConfig,
};

// Re-export dimension types.
pub use dimension::WorldDimension;

// Re-export 2D types.
pub use types_2d::{
    AnimationDef, AutoTileConfig, BodyType2D, Collider2D, Falloff2D, Light2D, RigidBody2D,
    SpriteEntity, SpriteSheetDef, TilemapData, TilesetDef, Transform2D,
};

// Re-export scene serialization types and functions.
pub use scene_serde::{
    deserialize_scene_2d, deserialize_scene_3d, serialize_scene_2d, serialize_scene_3d,
    Collider3DConfig, Entity2D, Entity3D, Light3D, Physics3D, Scene2D, Scene3D, Transform3D,
};

// Re-export project manifest types and functions.
pub use project::{
    deserialize_manifest as deserialize_project_manifest,
    serialize_manifest as serialize_project_manifest,
    validate_manifest as validate_project_manifest, Bounds2D, CameraConfig2D, CameraMode2D,
    EnvironmentConfig, ParallaxLayer, PhysicsConfig, PlayerConfig, SceneConfig, ValidationError,
    WorldInfo, WorldProjectManifest,
};
