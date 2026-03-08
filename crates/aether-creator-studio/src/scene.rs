//! Core scene types for the editor.

use serde::{Deserialize, Serialize};

/// Unique identifier for scene objects.
pub type ObjectId = u64;

/// 3D position.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Position {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

/// 3D rotation in degrees.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rotation {
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    pub roll_deg: f32,
}

impl Rotation {
    pub fn new(yaw_deg: f32, pitch_deg: f32, roll_deg: f32) -> Self {
        Self {
            yaw_deg,
            pitch_deg,
            roll_deg,
        }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

/// 3D scale.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Scale {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Scale {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn uniform(s: f32) -> Self {
        Self::new(s, s, s)
    }

    pub fn one() -> Self {
        Self::uniform(1.0)
    }
}

/// The kind of object in the scene.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectKind {
    Prop { template: String },
    Light,
    SpawnPoint,
    Vegetation { template: String },
}

/// A single object in the editor scene.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneObject {
    pub id: ObjectId,
    pub name: String,
    pub kind: ObjectKind,
    pub position: Position,
    pub rotation: Rotation,
    pub scale: Scale,
}

/// Editor scene holding all mutable state.
#[derive(Debug)]
pub struct EditorScene {
    pub objects: Vec<SceneObject>,
    pub selection: super::selection::Selection,
    pub terrain: Option<super::terrain_editor::TerrainData>,
    pub lighting: super::lighting_editor::LightingState,
    next_id: ObjectId,
}

impl EditorScene {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            selection: super::selection::Selection::new(),
            terrain: None,
            lighting: super::lighting_editor::LightingState::new(),
            next_id: 1,
        }
    }

    /// Allocate the next unique object id.
    pub fn next_id(&mut self) -> ObjectId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Find an object by id.
    pub fn find_object(&self, id: ObjectId) -> Option<&SceneObject> {
        self.objects.iter().find(|o| o.id == id)
    }

    /// Find a mutable reference to an object by id.
    pub fn find_object_mut(&mut self, id: ObjectId) -> Option<&mut SceneObject> {
        self.objects.iter_mut().find(|o| o.id == id)
    }

    /// Remove an object by id, returning it if found.
    pub fn remove_object(&mut self, id: ObjectId) -> Option<SceneObject> {
        if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
            Some(self.objects.remove(pos))
        } else {
            None
        }
    }

    /// Add an object to the scene.
    pub fn add_object(&mut self, obj: SceneObject) {
        self.objects.push(obj);
    }
}

impl Default for EditorScene {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_constructors() {
        let p = Position::new(1.0, 2.0, 3.0);
        assert_eq!(p.x, 1.0);
        assert_eq!(p.y, 2.0);
        assert_eq!(p.z, 3.0);

        let z = Position::zero();
        assert_eq!(z.x, 0.0);
        assert_eq!(z.y, 0.0);
        assert_eq!(z.z, 0.0);
    }

    #[test]
    fn test_rotation_constructors() {
        let r = Rotation::new(90.0, 45.0, 0.0);
        assert_eq!(r.yaw_deg, 90.0);
        assert_eq!(r.pitch_deg, 45.0);
        assert_eq!(r.roll_deg, 0.0);

        let z = Rotation::zero();
        assert_eq!(z.yaw_deg, 0.0);
    }

    #[test]
    fn test_scale_constructors() {
        let s = Scale::new(1.0, 2.0, 3.0);
        assert_eq!(s.x, 1.0);
        assert_eq!(s.y, 2.0);
        assert_eq!(s.z, 3.0);

        let u = Scale::uniform(5.0);
        assert_eq!(u.x, 5.0);
        assert_eq!(u.y, 5.0);
        assert_eq!(u.z, 5.0);

        let one = Scale::one();
        assert_eq!(one.x, 1.0);
    }

    #[test]
    fn test_scene_next_id() {
        let mut scene = EditorScene::new();
        assert_eq!(scene.next_id(), 1);
        assert_eq!(scene.next_id(), 2);
        assert_eq!(scene.next_id(), 3);
    }

    #[test]
    fn test_scene_add_find_remove() {
        let mut scene = EditorScene::new();
        let id = scene.next_id();
        let obj = SceneObject {
            id,
            name: "TestObj".into(),
            kind: ObjectKind::Prop {
                template: "cube".into(),
            },
            position: Position::zero(),
            rotation: Rotation::zero(),
            scale: Scale::one(),
        };
        scene.add_object(obj);

        assert!(scene.find_object(id).is_some());
        assert_eq!(scene.find_object(id).unwrap().name, "TestObj");

        // Mutable find
        scene.find_object_mut(id).unwrap().name = "Renamed".into();
        assert_eq!(scene.find_object(id).unwrap().name, "Renamed");

        // Remove
        let removed = scene.remove_object(id);
        assert!(removed.is_some());
        assert!(scene.find_object(id).is_none());

        // Remove non-existent
        assert!(scene.remove_object(999).is_none());
    }

    #[test]
    fn test_scene_default() {
        let scene = EditorScene::default();
        assert!(scene.objects.is_empty());
    }
}
