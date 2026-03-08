//! Prop placement and manipulation commands.

use crate::scene::{
    EditorScene, ObjectId, ObjectKind, Position, Rotation, Scale, SceneObject,
};
use crate::undo::{CommandError, CommandResult, EditorCommand};

/// Snap a float value to the nearest grid increment.
pub fn snap_to_grid(value: f32, grid_size: f32) -> f32 {
    if grid_size <= 0.0 {
        return value;
    }
    (value / grid_size).round() * grid_size
}

/// Snap a position to the grid.
pub fn snap_position(pos: Position, grid_size: f32) -> Position {
    Position {
        x: snap_to_grid(pos.x, grid_size),
        y: snap_to_grid(pos.y, grid_size),
        z: snap_to_grid(pos.z, grid_size),
    }
}

/// Command: place a new prop in the scene.
pub struct PlacePropCommand {
    pub template: String,
    pub position: Position,
    pub rotation: Rotation,
    pub snap_enabled: bool,
    pub grid_size: f32,
    placed_id: Option<ObjectId>,
}

impl PlacePropCommand {
    pub fn new(template: String, position: Position, rotation: Rotation) -> Self {
        Self {
            template,
            position,
            rotation,
            snap_enabled: false,
            grid_size: 1.0,
            placed_id: None,
        }
    }

    pub fn with_snap(mut self, grid_size: f32) -> Self {
        self.snap_enabled = true;
        self.grid_size = grid_size;
        self
    }
}

impl EditorCommand for PlacePropCommand {
    fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        let pos = if self.snap_enabled {
            snap_position(self.position, self.grid_size)
        } else {
            self.position
        };

        let id = scene.next_id();
        scene.add_object(SceneObject {
            id,
            name: format!("prop_{}", self.template),
            kind: ObjectKind::Prop {
                template: self.template.clone(),
            },
            position: pos,
            rotation: self.rotation,
            scale: Scale::one(),
        });
        self.placed_id = Some(id);
        Ok(())
    }

    fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        if let Some(id) = self.placed_id {
            scene.remove_object(id);
        }
        Ok(())
    }

    fn description(&self) -> &str {
        "place prop"
    }
}

/// Command: move a prop to a new position.
pub struct MovePropCommand {
    pub object_id: ObjectId,
    pub new_position: Position,
    old_position: Option<Position>,
}

impl MovePropCommand {
    pub fn new(object_id: ObjectId, new_position: Position) -> Self {
        Self {
            object_id,
            new_position,
            old_position: None,
        }
    }
}

impl EditorCommand for MovePropCommand {
    fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        let obj = scene
            .find_object_mut(self.object_id)
            .ok_or(CommandError::ObjectNotFound(self.object_id))?;
        self.old_position = Some(obj.position);
        obj.position = self.new_position;
        Ok(())
    }

    fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        if let Some(old_pos) = self.old_position {
            let obj = scene
                .find_object_mut(self.object_id)
                .ok_or(CommandError::ObjectNotFound(self.object_id))?;
            obj.position = old_pos;
        }
        Ok(())
    }

    fn description(&self) -> &str {
        "move prop"
    }
}

/// Command: rotate a prop.
pub struct RotatePropCommand {
    pub object_id: ObjectId,
    pub new_rotation: Rotation,
    old_rotation: Option<Rotation>,
}

impl RotatePropCommand {
    pub fn new(object_id: ObjectId, new_rotation: Rotation) -> Self {
        Self {
            object_id,
            new_rotation,
            old_rotation: None,
        }
    }
}

impl EditorCommand for RotatePropCommand {
    fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        let obj = scene
            .find_object_mut(self.object_id)
            .ok_or(CommandError::ObjectNotFound(self.object_id))?;
        self.old_rotation = Some(obj.rotation);
        obj.rotation = self.new_rotation;
        Ok(())
    }

    fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        if let Some(old_rot) = self.old_rotation {
            let obj = scene
                .find_object_mut(self.object_id)
                .ok_or(CommandError::ObjectNotFound(self.object_id))?;
            obj.rotation = old_rot;
        }
        Ok(())
    }

    fn description(&self) -> &str {
        "rotate prop"
    }
}

/// Command: scale a prop.
pub struct ScalePropCommand {
    pub object_id: ObjectId,
    pub new_scale: Scale,
    old_scale: Option<Scale>,
}

impl ScalePropCommand {
    pub fn new(object_id: ObjectId, new_scale: Scale) -> Self {
        Self {
            object_id,
            new_scale,
            old_scale: None,
        }
    }
}

impl EditorCommand for ScalePropCommand {
    fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        let obj = scene
            .find_object_mut(self.object_id)
            .ok_or(CommandError::ObjectNotFound(self.object_id))?;
        self.old_scale = Some(obj.scale);
        obj.scale = self.new_scale;
        Ok(())
    }

    fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        if let Some(old_sc) = self.old_scale {
            let obj = scene
                .find_object_mut(self.object_id)
                .ok_or(CommandError::ObjectNotFound(self.object_id))?;
            obj.scale = old_sc;
        }
        Ok(())
    }

    fn description(&self) -> &str {
        "scale prop"
    }
}

/// Command: delete a prop from the scene.
pub struct DeletePropCommand {
    pub object_id: ObjectId,
    removed_object: Option<SceneObject>,
}

impl DeletePropCommand {
    pub fn new(object_id: ObjectId) -> Self {
        Self {
            object_id,
            removed_object: None,
        }
    }
}

impl EditorCommand for DeletePropCommand {
    fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        let obj = scene
            .remove_object(self.object_id)
            .ok_or(CommandError::ObjectNotFound(self.object_id))?;
        self.removed_object = Some(obj);
        Ok(())
    }

    fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        if let Some(obj) = self.removed_object.take() {
            scene.add_object(obj);
        }
        Ok(())
    }

    fn description(&self) -> &str {
        "delete prop"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::undo::UndoStack;

    // Grid snap tests
    #[test]
    fn test_snap_to_grid_round_down() {
        assert_eq!(snap_to_grid(1.2, 1.0), 1.0);
    }

    #[test]
    fn test_snap_to_grid_round_up() {
        assert_eq!(snap_to_grid(1.7, 1.0), 2.0);
    }

    #[test]
    fn test_snap_to_grid_exact() {
        assert_eq!(snap_to_grid(2.0, 1.0), 2.0);
    }

    #[test]
    fn test_snap_to_grid_half() {
        // Rust f32::round() uses "round half away from zero"
        assert_eq!(snap_to_grid(0.5, 1.0), 1.0);
        assert_eq!(snap_to_grid(1.5, 1.0), 2.0);
    }

    #[test]
    fn test_snap_to_grid_negative() {
        assert_eq!(snap_to_grid(-1.3, 1.0), -1.0);
    }

    #[test]
    fn test_snap_to_grid_zero_grid() {
        assert_eq!(snap_to_grid(1.5, 0.0), 1.5);
    }

    #[test]
    fn test_snap_to_grid_fine_grid() {
        let result = snap_to_grid(1.37, 0.25);
        assert!((result - 1.25).abs() < f32::EPSILON);
    }

    #[test]
    fn test_snap_position() {
        let snapped = snap_position(Position::new(1.3, 2.7, -0.4), 1.0);
        assert_eq!(snapped.x, 1.0);
        assert_eq!(snapped.y, 3.0);
        assert_eq!(snapped.z, 0.0); // -0.4 rounds to 0
    }

    // PlaceProp tests
    #[test]
    fn test_place_prop() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlacePropCommand::new(
                    "chair".into(),
                    Position::new(1.0, 0.0, 2.0),
                    Rotation::zero(),
                )),
                &mut scene,
            )
            .unwrap();

        assert_eq!(scene.objects.len(), 1);
        assert_eq!(scene.objects[0].position.x, 1.0);
        assert!(matches!(scene.objects[0].kind, ObjectKind::Prop { .. }));
    }

    #[test]
    fn test_place_prop_with_snap() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(
                    PlacePropCommand::new(
                        "table".into(),
                        Position::new(1.3, 0.7, 2.8),
                        Rotation::zero(),
                    )
                    .with_snap(1.0),
                ),
                &mut scene,
            )
            .unwrap();

        let pos = scene.objects[0].position;
        assert_eq!(pos.x, 1.0);
        assert_eq!(pos.y, 1.0);
        assert_eq!(pos.z, 3.0);
    }

    #[test]
    fn test_place_prop_undo() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlacePropCommand::new(
                    "lamp".into(),
                    Position::zero(),
                    Rotation::zero(),
                )),
                &mut scene,
            )
            .unwrap();
        assert_eq!(scene.objects.len(), 1);

        stack.undo(&mut scene).unwrap();
        assert_eq!(scene.objects.len(), 0);
    }

    // MoveProp tests
    #[test]
    fn test_move_prop() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlacePropCommand::new(
                    "box".into(),
                    Position::new(0.0, 0.0, 0.0),
                    Rotation::zero(),
                )),
                &mut scene,
            )
            .unwrap();
        let id = scene.objects[0].id;

        stack
            .push(
                Box::new(MovePropCommand::new(id, Position::new(5.0, 0.0, 5.0))),
                &mut scene,
            )
            .unwrap();
        assert_eq!(scene.find_object(id).unwrap().position.x, 5.0);
    }

    #[test]
    fn test_move_prop_undo() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlacePropCommand::new(
                    "box".into(),
                    Position::new(1.0, 2.0, 3.0),
                    Rotation::zero(),
                )),
                &mut scene,
            )
            .unwrap();
        let id = scene.objects[0].id;

        stack
            .push(
                Box::new(MovePropCommand::new(id, Position::new(10.0, 0.0, 0.0))),
                &mut scene,
            )
            .unwrap();
        stack.undo(&mut scene).unwrap();

        let pos = scene.find_object(id).unwrap().position;
        assert_eq!(pos.x, 1.0);
        assert_eq!(pos.y, 2.0);
        assert_eq!(pos.z, 3.0);
    }

    #[test]
    fn test_move_prop_not_found() {
        let mut scene = EditorScene::new();
        let mut cmd = MovePropCommand::new(999, Position::zero());
        assert!(cmd.execute(&mut scene).is_err());
    }

    // RotateProp tests
    #[test]
    fn test_rotate_prop() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlacePropCommand::new(
                    "statue".into(),
                    Position::zero(),
                    Rotation::zero(),
                )),
                &mut scene,
            )
            .unwrap();
        let id = scene.objects[0].id;

        stack
            .push(
                Box::new(RotatePropCommand::new(
                    id,
                    Rotation::new(90.0, 0.0, 0.0),
                )),
                &mut scene,
            )
            .unwrap();
        assert_eq!(scene.find_object(id).unwrap().rotation.yaw_deg, 90.0);
    }

    #[test]
    fn test_rotate_prop_undo() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlacePropCommand::new(
                    "statue".into(),
                    Position::zero(),
                    Rotation::new(45.0, 0.0, 0.0),
                )),
                &mut scene,
            )
            .unwrap();
        let id = scene.objects[0].id;

        stack
            .push(
                Box::new(RotatePropCommand::new(
                    id,
                    Rotation::new(180.0, 0.0, 0.0),
                )),
                &mut scene,
            )
            .unwrap();
        stack.undo(&mut scene).unwrap();
        assert_eq!(scene.find_object(id).unwrap().rotation.yaw_deg, 45.0);
    }

    // ScaleProp tests
    #[test]
    fn test_scale_prop() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlacePropCommand::new(
                    "cube".into(),
                    Position::zero(),
                    Rotation::zero(),
                )),
                &mut scene,
            )
            .unwrap();
        let id = scene.objects[0].id;

        stack
            .push(
                Box::new(ScalePropCommand::new(id, Scale::uniform(2.0))),
                &mut scene,
            )
            .unwrap();
        assert_eq!(scene.find_object(id).unwrap().scale.x, 2.0);
    }

    #[test]
    fn test_scale_prop_undo() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlacePropCommand::new(
                    "cube".into(),
                    Position::zero(),
                    Rotation::zero(),
                )),
                &mut scene,
            )
            .unwrap();
        let id = scene.objects[0].id;

        stack
            .push(
                Box::new(ScalePropCommand::new(id, Scale::uniform(3.0))),
                &mut scene,
            )
            .unwrap();
        stack.undo(&mut scene).unwrap();
        assert_eq!(scene.find_object(id).unwrap().scale.x, 1.0);
    }

    // DeleteProp tests
    #[test]
    fn test_delete_prop() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlacePropCommand::new(
                    "barrel".into(),
                    Position::zero(),
                    Rotation::zero(),
                )),
                &mut scene,
            )
            .unwrap();
        let id = scene.objects[0].id;

        stack
            .push(Box::new(DeletePropCommand::new(id)), &mut scene)
            .unwrap();
        assert_eq!(scene.objects.len(), 0);
    }

    #[test]
    fn test_delete_prop_undo() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlacePropCommand::new(
                    "barrel".into(),
                    Position::zero(),
                    Rotation::zero(),
                )),
                &mut scene,
            )
            .unwrap();
        let id = scene.objects[0].id;

        stack
            .push(Box::new(DeletePropCommand::new(id)), &mut scene)
            .unwrap();
        stack.undo(&mut scene).unwrap();

        assert_eq!(scene.objects.len(), 1);
        assert_eq!(scene.objects[0].id, id);
    }

    #[test]
    fn test_delete_nonexistent_prop() {
        let mut scene = EditorScene::new();
        let mut cmd = DeletePropCommand::new(999);
        assert!(cmd.execute(&mut scene).is_err());
    }

    // Integration: place, move, rotate, scale, delete sequence
    #[test]
    fn test_full_prop_lifecycle() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        // Place
        stack
            .push(
                Box::new(PlacePropCommand::new(
                    "crate".into(),
                    Position::new(0.0, 0.0, 0.0),
                    Rotation::zero(),
                )),
                &mut scene,
            )
            .unwrap();
        let id = scene.objects[0].id;

        // Move
        stack
            .push(
                Box::new(MovePropCommand::new(id, Position::new(5.0, 0.0, 5.0))),
                &mut scene,
            )
            .unwrap();

        // Rotate
        stack
            .push(
                Box::new(RotatePropCommand::new(
                    id,
                    Rotation::new(90.0, 0.0, 0.0),
                )),
                &mut scene,
            )
            .unwrap();

        // Scale
        stack
            .push(
                Box::new(ScalePropCommand::new(id, Scale::uniform(2.0))),
                &mut scene,
            )
            .unwrap();

        // Delete
        stack
            .push(Box::new(DeletePropCommand::new(id)), &mut scene)
            .unwrap();
        assert_eq!(scene.objects.len(), 0);

        // Undo everything back
        stack.undo(&mut scene).unwrap(); // undo delete
        assert_eq!(scene.objects.len(), 1);

        stack.undo(&mut scene).unwrap(); // undo scale
        assert_eq!(scene.find_object(id).unwrap().scale.x, 1.0);

        stack.undo(&mut scene).unwrap(); // undo rotate
        assert_eq!(scene.find_object(id).unwrap().rotation.yaw_deg, 0.0);

        stack.undo(&mut scene).unwrap(); // undo move
        assert_eq!(scene.find_object(id).unwrap().position.x, 0.0);

        stack.undo(&mut scene).unwrap(); // undo place
        assert_eq!(scene.objects.len(), 0);
    }
}
