//! Reload event types for the hot-reload system.

use std::path::PathBuf;

use crate::hot_reload::asset_type::AssetType;

/// The kind of file system change detected.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChangeKind {
    Created,
    Modified,
    Deleted,
}

/// An event emitted when an asset needs reloading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReloadEvent {
    /// Path to the changed file.
    pub path: PathBuf,
    /// Classified asset type.
    pub asset_type: AssetType,
    /// What kind of change occurred.
    pub change_kind: ChangeKind,
    /// Paths of dependent assets that also need reloading.
    pub affected_dependents: Vec<PathBuf>,
}

impl ReloadEvent {
    /// Create a new reload event.
    pub fn new(
        path: PathBuf,
        asset_type: AssetType,
        change_kind: ChangeKind,
        affected_dependents: Vec<PathBuf>,
    ) -> Self {
        Self {
            path,
            asset_type,
            change_kind,
            affected_dependents,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reload_event_new_sets_fields() {
        let event = ReloadEvent::new(
            PathBuf::from("assets/model.glb"),
            AssetType::Mesh,
            ChangeKind::Modified,
            vec![PathBuf::from("assets/scene.toml")],
        );
        assert_eq!(event.path, PathBuf::from("assets/model.glb"));
        assert_eq!(event.asset_type, AssetType::Mesh);
        assert_eq!(event.change_kind, ChangeKind::Modified);
        assert_eq!(event.affected_dependents.len(), 1);
    }

    #[test]
    fn reload_event_created_kind() {
        let event = ReloadEvent::new(
            PathBuf::from("new_file.png"),
            AssetType::Texture,
            ChangeKind::Created,
            vec![],
        );
        assert_eq!(event.change_kind, ChangeKind::Created);
    }

    #[test]
    fn reload_event_deleted_kind() {
        let event = ReloadEvent::new(
            PathBuf::from("old.wav"),
            AssetType::Audio,
            ChangeKind::Deleted,
            vec![],
        );
        assert_eq!(event.change_kind, ChangeKind::Deleted);
    }

    #[test]
    fn reload_event_no_dependents() {
        let event = ReloadEvent::new(
            PathBuf::from("script.lua"),
            AssetType::Script,
            ChangeKind::Modified,
            vec![],
        );
        assert!(event.affected_dependents.is_empty());
    }

    #[test]
    fn reload_event_multiple_dependents() {
        let deps = vec![
            PathBuf::from("a.toml"),
            PathBuf::from("b.toml"),
            PathBuf::from("c.toml"),
        ];
        let event = ReloadEvent::new(
            PathBuf::from("texture.png"),
            AssetType::Texture,
            ChangeKind::Modified,
            deps.clone(),
        );
        assert_eq!(event.affected_dependents, deps);
    }

    #[test]
    fn change_kind_equality() {
        assert_eq!(ChangeKind::Created, ChangeKind::Created);
        assert_ne!(ChangeKind::Created, ChangeKind::Modified);
        assert_ne!(ChangeKind::Modified, ChangeKind::Deleted);
    }

    #[test]
    fn reload_event_clone() {
        let event = ReloadEvent::new(
            PathBuf::from("test.glb"),
            AssetType::Mesh,
            ChangeKind::Modified,
            vec![],
        );
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn reload_event_unknown_asset_type() {
        let event = ReloadEvent::new(
            PathBuf::from("readme.txt"),
            AssetType::Unknown,
            ChangeKind::Created,
            vec![],
        );
        assert_eq!(event.asset_type, AssetType::Unknown);
    }
}
