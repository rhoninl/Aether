//! Lighting editor: light probe placement and ambient settings.

use serde::{Deserialize, Serialize};

use crate::scene::{EditorScene, ObjectId, ObjectKind, Position, Rotation, Scale, SceneObject};
use crate::undo::{CommandError, CommandResult, EditorCommand};

/// Ambient lighting settings for the scene.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AmbientSettings {
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    pub intensity: f32,
}

impl AmbientSettings {
    pub fn new(r: f32, g: f32, b: f32, intensity: f32) -> Self {
        Self {
            color_r: r,
            color_g: g,
            color_b: b,
            intensity,
        }
    }
}

impl Default for AmbientSettings {
    fn default() -> Self {
        Self::new(1.0, 1.0, 1.0, 0.3)
    }
}

/// A light probe placed in the scene.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightProbe {
    pub object_id: ObjectId,
    pub radius: f32,
    pub intensity: f32,
}

/// Holds lighting state: ambient settings and probe references.
#[derive(Debug, Clone)]
pub struct LightingState {
    pub ambient: AmbientSettings,
    pub probes: Vec<LightProbe>,
}

impl LightingState {
    pub fn new() -> Self {
        Self {
            ambient: AmbientSettings::default(),
            probes: Vec::new(),
        }
    }

    pub fn find_probe(&self, object_id: ObjectId) -> Option<&LightProbe> {
        self.probes.iter().find(|p| p.object_id == object_id)
    }

    pub fn remove_probe(&mut self, object_id: ObjectId) -> Option<LightProbe> {
        if let Some(pos) = self.probes.iter().position(|p| p.object_id == object_id) {
            Some(self.probes.remove(pos))
        } else {
            None
        }
    }
}

impl Default for LightingState {
    fn default() -> Self {
        Self::new()
    }
}

/// Command: place a light probe in the scene.
pub struct PlaceLightProbeCommand {
    pub position: Position,
    pub radius: f32,
    pub intensity: f32,
    placed_id: Option<ObjectId>,
}

impl PlaceLightProbeCommand {
    pub fn new(position: Position, radius: f32, intensity: f32) -> Self {
        Self {
            position,
            radius,
            intensity,
            placed_id: None,
        }
    }
}

impl EditorCommand for PlaceLightProbeCommand {
    fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        let id = scene.next_id();
        scene.add_object(SceneObject {
            id,
            name: format!("light_probe_{id}"),
            kind: ObjectKind::Light,
            position: self.position,
            rotation: Rotation::zero(),
            scale: Scale::one(),
        });
        scene.lighting.probes.push(LightProbe {
            object_id: id,
            radius: self.radius,
            intensity: self.intensity,
        });
        self.placed_id = Some(id);
        Ok(())
    }

    fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        if let Some(id) = self.placed_id {
            scene.remove_object(id);
            scene.lighting.remove_probe(id);
        }
        Ok(())
    }

    fn description(&self) -> &str {
        "place light probe"
    }
}

/// Command: remove a light probe from the scene.
pub struct RemoveLightProbeCommand {
    pub object_id: ObjectId,
    removed_object: Option<SceneObject>,
    removed_probe: Option<LightProbe>,
}

impl RemoveLightProbeCommand {
    pub fn new(object_id: ObjectId) -> Self {
        Self {
            object_id,
            removed_object: None,
            removed_probe: None,
        }
    }
}

impl EditorCommand for RemoveLightProbeCommand {
    fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        let obj = scene
            .remove_object(self.object_id)
            .ok_or(CommandError::ObjectNotFound(self.object_id))?;
        self.removed_object = Some(obj);
        self.removed_probe = scene.lighting.remove_probe(self.object_id);
        Ok(())
    }

    fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        if let Some(obj) = self.removed_object.take() {
            scene.add_object(obj);
        }
        if let Some(probe) = self.removed_probe.take() {
            scene.lighting.probes.push(probe);
        }
        Ok(())
    }

    fn description(&self) -> &str {
        "remove light probe"
    }
}

/// Command: change ambient lighting settings.
pub struct SetAmbientCommand {
    pub new_settings: AmbientSettings,
    old_settings: Option<AmbientSettings>,
}

impl SetAmbientCommand {
    pub fn new(settings: AmbientSettings) -> Self {
        Self {
            new_settings: settings,
            old_settings: None,
        }
    }
}

impl EditorCommand for SetAmbientCommand {
    fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        self.old_settings = Some(scene.lighting.ambient);
        scene.lighting.ambient = self.new_settings;
        Ok(())
    }

    fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        if let Some(old) = self.old_settings {
            scene.lighting.ambient = old;
        }
        Ok(())
    }

    fn description(&self) -> &str {
        "set ambient lighting"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::undo::UndoStack;

    #[test]
    fn test_ambient_default() {
        let a = AmbientSettings::default();
        assert_eq!(a.color_r, 1.0);
        assert_eq!(a.color_g, 1.0);
        assert_eq!(a.color_b, 1.0);
        assert_eq!(a.intensity, 0.3);
    }

    #[test]
    fn test_lighting_state_new() {
        let state = LightingState::new();
        assert!(state.probes.is_empty());
        assert_eq!(state.ambient, AmbientSettings::default());
    }

    // PlaceLightProbe tests
    #[test]
    fn test_place_light_probe() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlaceLightProbeCommand::new(
                    Position::new(5.0, 3.0, 5.0),
                    10.0,
                    1.5,
                )),
                &mut scene,
            )
            .unwrap();

        assert_eq!(scene.objects.len(), 1);
        assert!(matches!(scene.objects[0].kind, ObjectKind::Light));
        assert_eq!(scene.lighting.probes.len(), 1);
        assert_eq!(scene.lighting.probes[0].radius, 10.0);
        assert_eq!(scene.lighting.probes[0].intensity, 1.5);
    }

    #[test]
    fn test_place_light_probe_undo() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlaceLightProbeCommand::new(Position::zero(), 5.0, 1.0)),
                &mut scene,
            )
            .unwrap();
        assert_eq!(scene.lighting.probes.len(), 1);

        stack.undo(&mut scene).unwrap();
        assert_eq!(scene.objects.len(), 0);
        assert_eq!(scene.lighting.probes.len(), 0);
    }

    // RemoveLightProbe tests
    #[test]
    fn test_remove_light_probe() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlaceLightProbeCommand::new(Position::zero(), 5.0, 1.0)),
                &mut scene,
            )
            .unwrap();
        let id = scene.objects[0].id;

        stack
            .push(Box::new(RemoveLightProbeCommand::new(id)), &mut scene)
            .unwrap();
        assert_eq!(scene.objects.len(), 0);
        assert_eq!(scene.lighting.probes.len(), 0);
    }

    #[test]
    fn test_remove_light_probe_undo() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlaceLightProbeCommand::new(
                    Position::new(1.0, 2.0, 3.0),
                    8.0,
                    2.0,
                )),
                &mut scene,
            )
            .unwrap();
        let id = scene.objects[0].id;

        stack
            .push(Box::new(RemoveLightProbeCommand::new(id)), &mut scene)
            .unwrap();

        stack.undo(&mut scene).unwrap();
        assert_eq!(scene.objects.len(), 1);
        assert_eq!(scene.lighting.probes.len(), 1);
        assert_eq!(scene.lighting.probes[0].radius, 8.0);
    }

    #[test]
    fn test_remove_nonexistent_probe() {
        let mut scene = EditorScene::new();
        let mut cmd = RemoveLightProbeCommand::new(999);
        assert!(cmd.execute(&mut scene).is_err());
    }

    // SetAmbient tests
    #[test]
    fn test_set_ambient() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        let new_ambient = AmbientSettings::new(0.5, 0.6, 0.7, 0.8);
        stack
            .push(Box::new(SetAmbientCommand::new(new_ambient)), &mut scene)
            .unwrap();

        assert_eq!(scene.lighting.ambient.color_r, 0.5);
        assert_eq!(scene.lighting.ambient.intensity, 0.8);
    }

    #[test]
    fn test_set_ambient_undo() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        let original = scene.lighting.ambient;

        let new_ambient = AmbientSettings::new(0.1, 0.2, 0.3, 0.9);
        stack
            .push(Box::new(SetAmbientCommand::new(new_ambient)), &mut scene)
            .unwrap();

        stack.undo(&mut scene).unwrap();
        assert_eq!(scene.lighting.ambient, original);
    }

    // Multiple probes test
    #[test]
    fn test_multiple_probes() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlaceLightProbeCommand::new(
                    Position::new(0.0, 0.0, 0.0),
                    5.0,
                    1.0,
                )),
                &mut scene,
            )
            .unwrap();
        stack
            .push(
                Box::new(PlaceLightProbeCommand::new(
                    Position::new(10.0, 0.0, 10.0),
                    8.0,
                    2.0,
                )),
                &mut scene,
            )
            .unwrap();

        assert_eq!(scene.lighting.probes.len(), 2);
        assert_eq!(scene.objects.len(), 2);
    }

    #[test]
    fn test_find_probe() {
        let mut state = LightingState::new();
        state.probes.push(LightProbe {
            object_id: 1,
            radius: 5.0,
            intensity: 1.0,
        });
        assert!(state.find_probe(1).is_some());
        assert!(state.find_probe(99).is_none());
    }
}
