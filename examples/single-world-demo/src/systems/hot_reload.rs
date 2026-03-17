//! Hot-reload system: bridges asset file watching with the GPU renderer's
//! asset re-upload pipeline.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use aether_asset_pipeline::hot_reload::asset_type::AssetType;
use aether_asset_pipeline::hot_reload::events::{ChangeKind, ReloadEvent};
use aether_renderer::gpu::material::MaterialId;
use aether_renderer::gpu::mesh::MeshId;

/// Action to take in response to a reload event.
///
/// The game loop reads these actions and dispatches them to the appropriate
/// subsystem (renderer, scripting engine, etc.).
#[derive(Debug, Clone, PartialEq)]
pub enum ReloadAction {
    /// Re-upload a mesh from disk to the GPU.
    ReloadMesh { path: PathBuf, mesh_id: MeshId },
    /// Re-upload a material from disk to the GPU.
    ReloadMaterial {
        path: PathBuf,
        material_id: MaterialId,
    },
    /// Re-compile and hot-swap a script.
    ReloadScript { path: PathBuf },
    /// Stop tracking a deleted asset.
    Untrack { path: PathBuf },
    /// Asset type not handled by the hot-reload system.
    Unknown,
}

/// Maps file paths to GPU asset IDs so the hot-reload system knows which
/// GPU resources to re-upload when a file changes on disk.
#[derive(Debug, Default)]
pub struct AssetTracker {
    meshes: HashMap<PathBuf, MeshId>,
    materials: HashMap<PathBuf, MaterialId>,
}

impl AssetTracker {
    /// Create a new empty asset tracker.
    pub fn new() -> Self {
        Self {
            meshes: HashMap::new(),
            materials: HashMap::new(),
        }
    }

    /// Register a mesh file path and its GPU mesh ID.
    pub fn register_mesh(&mut self, path: PathBuf, mesh_id: MeshId) {
        self.meshes.insert(path, mesh_id);
    }

    /// Register a material file path and its GPU material ID.
    pub fn register_material(&mut self, path: PathBuf, material_id: MaterialId) {
        self.materials.insert(path, material_id);
    }

    /// Look up the GPU mesh ID for a file path.
    pub fn get_mesh(&self, path: &Path) -> Option<MeshId> {
        self.meshes.get(path).copied()
    }

    /// Look up the GPU material ID for a file path.
    pub fn get_material(&self, path: &Path) -> Option<MaterialId> {
        self.materials.get(path).copied()
    }

    /// Remove tracking for an asset at the given path (mesh or material).
    pub fn remove(&mut self, path: &Path) {
        self.meshes.remove(path);
        self.materials.remove(path);
    }

    /// Number of tracked mesh files.
    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }

    /// Number of tracked material files.
    pub fn material_count(&self) -> usize {
        self.materials.len()
    }
}

/// Convert a `ReloadEvent` into a list of concrete `ReloadAction`s.
///
/// For deleted assets, the action is always `Untrack`.
/// For created or modified assets, the action depends on the asset type
/// and whether the tracker knows about the file.
pub fn process_reload_event(event: &ReloadEvent, tracker: &AssetTracker) -> Vec<ReloadAction> {
    let mut actions = Vec::new();

    // Deleted assets are always untracked.
    if event.change_kind == ChangeKind::Deleted {
        actions.push(ReloadAction::Untrack {
            path: event.path.clone(),
        });
        return actions;
    }

    match event.asset_type {
        AssetType::Mesh => {
            if let Some(mesh_id) = tracker.get_mesh(&event.path) {
                actions.push(ReloadAction::ReloadMesh {
                    path: event.path.clone(),
                    mesh_id,
                });
            }
        }
        AssetType::Material => {
            if let Some(material_id) = tracker.get_material(&event.path) {
                actions.push(ReloadAction::ReloadMaterial {
                    path: event.path.clone(),
                    material_id,
                });
            }
        }
        AssetType::Script => {
            actions.push(ReloadAction::ReloadScript {
                path: event.path.clone(),
            });
        }
        AssetType::Texture | AssetType::Audio | AssetType::Unknown => {
            actions.push(ReloadAction::Unknown);
        }
    }

    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- AssetTracker tests --

    #[test]
    fn tracker_starts_empty() {
        let tracker = AssetTracker::new();
        assert_eq!(tracker.mesh_count(), 0);
        assert_eq!(tracker.material_count(), 0);
    }

    #[test]
    fn tracker_default_is_empty() {
        let tracker = AssetTracker::default();
        assert_eq!(tracker.mesh_count(), 0);
        assert_eq!(tracker.material_count(), 0);
    }

    #[test]
    fn register_and_get_mesh() {
        let mut tracker = AssetTracker::new();
        let path = PathBuf::from("assets/model.glb");
        let mesh_id = MeshId(1);
        tracker.register_mesh(path.clone(), mesh_id);
        assert_eq!(tracker.get_mesh(&path), Some(MeshId(1)));
        assert_eq!(tracker.mesh_count(), 1);
    }

    #[test]
    fn register_and_get_material() {
        let mut tracker = AssetTracker::new();
        let path = PathBuf::from("assets/metal.mat");
        let material_id = MaterialId(2);
        tracker.register_material(path.clone(), material_id);
        assert_eq!(tracker.get_material(&path), Some(MaterialId(2)));
        assert_eq!(tracker.material_count(), 1);
    }

    #[test]
    fn get_mesh_missing_returns_none() {
        let tracker = AssetTracker::new();
        assert!(tracker.get_mesh(Path::new("nonexistent.glb")).is_none());
    }

    #[test]
    fn get_material_missing_returns_none() {
        let tracker = AssetTracker::new();
        assert!(tracker
            .get_material(Path::new("nonexistent.mat"))
            .is_none());
    }

    #[test]
    fn register_mesh_overwrites() {
        let mut tracker = AssetTracker::new();
        let path = PathBuf::from("model.glb");
        tracker.register_mesh(path.clone(), MeshId(1));
        tracker.register_mesh(path.clone(), MeshId(2));
        assert_eq!(tracker.get_mesh(&path), Some(MeshId(2)));
        assert_eq!(tracker.mesh_count(), 1);
    }

    #[test]
    fn remove_mesh() {
        let mut tracker = AssetTracker::new();
        let path = PathBuf::from("model.glb");
        tracker.register_mesh(path.clone(), MeshId(1));
        tracker.remove(&path);
        assert!(tracker.get_mesh(&path).is_none());
        assert_eq!(tracker.mesh_count(), 0);
    }

    #[test]
    fn remove_material() {
        let mut tracker = AssetTracker::new();
        let path = PathBuf::from("metal.mat");
        tracker.register_material(path.clone(), MaterialId(3));
        tracker.remove(&path);
        assert!(tracker.get_material(&path).is_none());
        assert_eq!(tracker.material_count(), 0);
    }

    #[test]
    fn remove_nonexistent_is_noop() {
        let mut tracker = AssetTracker::new();
        tracker.remove(Path::new("nope.glb"));
        assert_eq!(tracker.mesh_count(), 0);
    }

    #[test]
    fn multiple_meshes_and_materials() {
        let mut tracker = AssetTracker::new();
        tracker.register_mesh(PathBuf::from("a.glb"), MeshId(1));
        tracker.register_mesh(PathBuf::from("b.glb"), MeshId(2));
        tracker.register_material(PathBuf::from("x.mat"), MaterialId(10));
        tracker.register_material(PathBuf::from("y.mat"), MaterialId(20));
        assert_eq!(tracker.mesh_count(), 2);
        assert_eq!(tracker.material_count(), 2);
    }

    // -- process_reload_event tests --

    #[test]
    fn process_deleted_mesh_event() {
        let tracker = AssetTracker::new();
        let event = ReloadEvent::new(
            PathBuf::from("model.glb"),
            AssetType::Mesh,
            ChangeKind::Deleted,
            vec![],
        );
        let actions = process_reload_event(&event, &tracker);
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            ReloadAction::Untrack {
                path: PathBuf::from("model.glb")
            }
        );
    }

    #[test]
    fn process_deleted_material_event() {
        let tracker = AssetTracker::new();
        let event = ReloadEvent::new(
            PathBuf::from("metal.mat"),
            AssetType::Material,
            ChangeKind::Deleted,
            vec![],
        );
        let actions = process_reload_event(&event, &tracker);
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], ReloadAction::Untrack { .. }));
    }

    #[test]
    fn process_modified_tracked_mesh() {
        let mut tracker = AssetTracker::new();
        let path = PathBuf::from("model.glb");
        tracker.register_mesh(path.clone(), MeshId(5));
        let event = ReloadEvent::new(path.clone(), AssetType::Mesh, ChangeKind::Modified, vec![]);
        let actions = process_reload_event(&event, &tracker);
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            ReloadAction::ReloadMesh {
                path,
                mesh_id: MeshId(5)
            }
        );
    }

    #[test]
    fn process_modified_untracked_mesh() {
        let tracker = AssetTracker::new();
        let event = ReloadEvent::new(
            PathBuf::from("new_model.glb"),
            AssetType::Mesh,
            ChangeKind::Modified,
            vec![],
        );
        let actions = process_reload_event(&event, &tracker);
        // Not tracked, so no action for mesh.
        assert!(actions.is_empty());
    }

    #[test]
    fn process_modified_tracked_material() {
        let mut tracker = AssetTracker::new();
        let path = PathBuf::from("metal.mat");
        tracker.register_material(path.clone(), MaterialId(7));
        let event =
            ReloadEvent::new(path.clone(), AssetType::Material, ChangeKind::Modified, vec![]);
        let actions = process_reload_event(&event, &tracker);
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            ReloadAction::ReloadMaterial {
                path,
                material_id: MaterialId(7)
            }
        );
    }

    #[test]
    fn process_modified_script() {
        let tracker = AssetTracker::new();
        let path = PathBuf::from("logic.wasm");
        let event =
            ReloadEvent::new(path.clone(), AssetType::Script, ChangeKind::Modified, vec![]);
        let actions = process_reload_event(&event, &tracker);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], ReloadAction::ReloadScript { path });
    }

    #[test]
    fn process_created_script() {
        let tracker = AssetTracker::new();
        let path = PathBuf::from("new_script.lua");
        let event =
            ReloadEvent::new(path.clone(), AssetType::Script, ChangeKind::Created, vec![]);
        let actions = process_reload_event(&event, &tracker);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], ReloadAction::ReloadScript { path });
    }

    #[test]
    fn process_texture_event_returns_unknown() {
        let tracker = AssetTracker::new();
        let event = ReloadEvent::new(
            PathBuf::from("diffuse.png"),
            AssetType::Texture,
            ChangeKind::Modified,
            vec![],
        );
        let actions = process_reload_event(&event, &tracker);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], ReloadAction::Unknown);
    }

    #[test]
    fn process_audio_event_returns_unknown() {
        let tracker = AssetTracker::new();
        let event = ReloadEvent::new(
            PathBuf::from("music.ogg"),
            AssetType::Audio,
            ChangeKind::Modified,
            vec![],
        );
        let actions = process_reload_event(&event, &tracker);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], ReloadAction::Unknown);
    }

    #[test]
    fn process_unknown_asset_type_returns_unknown() {
        let tracker = AssetTracker::new();
        let event = ReloadEvent::new(
            PathBuf::from("readme.txt"),
            AssetType::Unknown,
            ChangeKind::Created,
            vec![],
        );
        let actions = process_reload_event(&event, &tracker);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], ReloadAction::Unknown);
    }

    #[test]
    fn process_deleted_script() {
        let tracker = AssetTracker::new();
        let event = ReloadEvent::new(
            PathBuf::from("old.wasm"),
            AssetType::Script,
            ChangeKind::Deleted,
            vec![],
        );
        let actions = process_reload_event(&event, &tracker);
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], ReloadAction::Untrack { .. }));
    }

    #[test]
    fn reload_action_debug_format() {
        let action = ReloadAction::ReloadMesh {
            path: PathBuf::from("test.glb"),
            mesh_id: MeshId(1),
        };
        let debug = format!("{action:?}");
        assert!(debug.contains("ReloadMesh"));
    }

    #[test]
    fn process_created_tracked_mesh() {
        let mut tracker = AssetTracker::new();
        let path = PathBuf::from("model.glb");
        tracker.register_mesh(path.clone(), MeshId(3));
        let event = ReloadEvent::new(path.clone(), AssetType::Mesh, ChangeKind::Created, vec![]);
        let actions = process_reload_event(&event, &tracker);
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            ReloadAction::ReloadMesh {
                path,
                mesh_id: MeshId(3)
            }
        );
    }
}
