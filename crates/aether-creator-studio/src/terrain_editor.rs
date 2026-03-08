//! Terrain editing operations: sculpt, paint, and vegetation placement.

use serde::{Deserialize, Serialize};

use crate::scene::{EditorScene, ObjectKind, Position, Rotation, Scale, SceneObject};
use crate::terrain::TerrainBrush;
use crate::undo::{CommandError, CommandResult, EditorCommand};

/// Grid of terrain height values and paint layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainData {
    pub width: usize,
    pub height: usize,
    /// Row-major heightmap: `heightmap[z * width + x]`.
    pub heightmap: Vec<f32>,
    pub paint_layers: Vec<PaintLayer>,
}

/// A texture paint layer over the terrain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaintLayer {
    pub texture_id: String,
    /// Per-cell weight in [0.0, 1.0]. Same dimensions as the heightmap.
    pub weights: Vec<f32>,
}

impl TerrainData {
    pub fn new(width: usize, height: usize) -> Self {
        let size = width * height;
        Self {
            width,
            height,
            heightmap: vec![0.0; size],
            paint_layers: Vec::new(),
        }
    }

    /// Add a paint layer.
    pub fn add_paint_layer(&mut self, texture_id: String) -> usize {
        let size = self.width * self.height;
        let idx = self.paint_layers.len();
        self.paint_layers.push(PaintLayer {
            texture_id,
            weights: vec![0.0; size],
        });
        idx
    }

    /// Get heightmap index for (x, z) coordinates. Returns None if out of bounds.
    fn index(&self, x: usize, z: usize) -> Option<usize> {
        if x < self.width && z < self.height {
            Some(z * self.width + x)
        } else {
            None
        }
    }

    /// Get height at integer grid coordinate.
    pub fn get_height(&self, x: usize, z: usize) -> Option<f32> {
        self.index(x, z).map(|i| self.heightmap[i])
    }

    /// Set height at integer grid coordinate.
    pub fn set_height(&mut self, x: usize, z: usize, val: f32) {
        if let Some(i) = self.index(x, z) {
            self.heightmap[i] = val;
        }
    }
}

/// Sculpt command: applies a brush to the heightmap.
pub struct SculptCommand {
    pub brush: TerrainBrush,
    pub center_x: usize,
    pub center_z: usize,
    pub radius: usize,
    pub intensity: f32,
    /// Stored previous heights for undo: `(x, z, old_value)`.
    previous_heights: Vec<(usize, usize, f32)>,
}

impl SculptCommand {
    pub fn new(
        brush: TerrainBrush,
        center_x: usize,
        center_z: usize,
        radius: usize,
        intensity: f32,
    ) -> Self {
        Self {
            brush,
            center_x,
            center_z,
            radius,
            intensity,
            previous_heights: Vec::new(),
        }
    }
}

impl EditorCommand for SculptCommand {
    fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        let terrain = scene.terrain.as_mut().ok_or(CommandError::NoTerrain)?;
        self.previous_heights.clear();

        let cx = self.center_x as isize;
        let cz = self.center_z as isize;
        let r = self.radius as isize;

        for dz in -r..=r {
            for dx in -r..=r {
                let x = cx + dx;
                let z = cz + dz;
                if x < 0 || z < 0 {
                    continue;
                }
                let ux = x as usize;
                let uz = z as usize;

                if let Some(old_h) = terrain.get_height(ux, uz) {
                    // Compute distance-based falloff
                    let dist = ((dx * dx + dz * dz) as f32).sqrt();
                    let max_dist = self.radius as f32;
                    if dist > max_dist {
                        continue;
                    }
                    let falloff = if max_dist > 0.0 {
                        1.0 - (dist / max_dist)
                    } else {
                        1.0 // radius=0 means only center cell at full intensity
                    };
                    let delta = self.intensity * falloff;

                    let new_h = match self.brush {
                        TerrainBrush::Raise => old_h + delta,
                        TerrainBrush::Lower => old_h - delta,
                        TerrainBrush::Smooth => {
                            // Average with neighbors
                            let avg = compute_neighbor_average(terrain, ux, uz);
                            old_h + (avg - old_h) * delta.min(1.0)
                        }
                        TerrainBrush::Flatten => {
                            // Flatten toward center height
                            let center_h = terrain
                                .get_height(self.center_x, self.center_z)
                                .unwrap_or(0.0);
                            old_h + (center_h - old_h) * delta.min(1.0)
                        }
                    };

                    self.previous_heights.push((ux, uz, old_h));
                    terrain.set_height(ux, uz, new_h);
                }
            }
        }

        Ok(())
    }

    fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        let terrain = scene.terrain.as_mut().ok_or(CommandError::NoTerrain)?;
        for &(x, z, old_h) in &self.previous_heights {
            terrain.set_height(x, z, old_h);
        }
        Ok(())
    }

    fn description(&self) -> &str {
        "sculpt terrain"
    }
}

/// Compute the average height of the 4 cardinal neighbors.
fn compute_neighbor_average(terrain: &TerrainData, x: usize, z: usize) -> f32 {
    let mut sum = 0.0;
    let mut count = 0;
    if x > 0 {
        if let Some(h) = terrain.get_height(x - 1, z) {
            sum += h;
            count += 1;
        }
    }
    if let Some(h) = terrain.get_height(x + 1, z) {
        sum += h;
        count += 1;
    }
    if z > 0 {
        if let Some(h) = terrain.get_height(x, z - 1) {
            sum += h;
            count += 1;
        }
    }
    if let Some(h) = terrain.get_height(x, z + 1) {
        sum += h;
        count += 1;
    }
    if count > 0 {
        sum / count as f32
    } else {
        terrain.get_height(x, z).unwrap_or(0.0)
    }
}

/// Paint command: modifies a paint layer's weights.
pub struct PaintCommand {
    pub layer_index: usize,
    pub center_x: usize,
    pub center_z: usize,
    pub radius: usize,
    pub intensity: f32,
    /// Stored previous weights for undo: `(x, z, old_weight)`.
    previous_weights: Vec<(usize, usize, f32)>,
}

impl PaintCommand {
    pub fn new(
        layer_index: usize,
        center_x: usize,
        center_z: usize,
        radius: usize,
        intensity: f32,
    ) -> Self {
        Self {
            layer_index,
            center_x,
            center_z,
            radius,
            intensity,
            previous_weights: Vec::new(),
        }
    }
}

impl EditorCommand for PaintCommand {
    fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        let terrain = scene.terrain.as_mut().ok_or(CommandError::NoTerrain)?;

        if self.layer_index >= terrain.paint_layers.len() {
            return Err(CommandError::ValidationError(format!(
                "paint layer index {} out of range (have {})",
                self.layer_index,
                terrain.paint_layers.len()
            )));
        }

        self.previous_weights.clear();

        let cx = self.center_x as isize;
        let cz = self.center_z as isize;
        let r = self.radius as isize;

        for dz in -r..=r {
            for dx in -r..=r {
                let x = cx + dx;
                let z = cz + dz;
                if x < 0 || z < 0 {
                    continue;
                }
                let ux = x as usize;
                let uz = z as usize;

                if let Some(idx) = terrain.index(ux, uz) {
                    let dist = ((dx * dx + dz * dz) as f32).sqrt();
                    let max_dist = self.radius as f32;
                    if dist > max_dist {
                        continue;
                    }
                    let falloff = if max_dist > 0.0 {
                        1.0 - (dist / max_dist)
                    } else {
                        1.0
                    };
                    let delta = self.intensity * falloff;

                    let layer = &mut terrain.paint_layers[self.layer_index];
                    let old_w = layer.weights[idx];
                    let new_w = (old_w + delta).clamp(0.0, 1.0);

                    self.previous_weights.push((ux, uz, old_w));
                    layer.weights[idx] = new_w;
                }
            }
        }

        Ok(())
    }

    fn undo(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        let terrain = scene.terrain.as_mut().ok_or(CommandError::NoTerrain)?;

        if self.layer_index >= terrain.paint_layers.len() {
            return Err(CommandError::ValidationError(
                "paint layer disappeared".into(),
            ));
        }

        for &(x, z, old_w) in &self.previous_weights {
            if let Some(idx) = terrain.index(x, z) {
                terrain.paint_layers[self.layer_index].weights[idx] = old_w;
            }
        }
        Ok(())
    }

    fn description(&self) -> &str {
        "paint terrain"
    }
}

/// Command that places vegetation as a scene object.
pub struct PlaceVegetationCommand {
    pub template_id: String,
    pub position: Position,
    placed_id: Option<u64>,
}

impl PlaceVegetationCommand {
    pub fn new(template_id: String, position: Position) -> Self {
        Self {
            template_id,
            position,
            placed_id: None,
        }
    }
}

impl EditorCommand for PlaceVegetationCommand {
    fn execute(&mut self, scene: &mut EditorScene) -> CommandResult<()> {
        let id = scene.next_id();
        scene.add_object(SceneObject {
            id,
            name: format!("vegetation_{}", self.template_id),
            kind: ObjectKind::Vegetation {
                template: self.template_id.clone(),
            },
            position: self.position,
            rotation: Rotation::zero(),
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
        "place vegetation"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::undo::UndoStack;

    fn scene_with_terrain(width: usize, height: usize) -> EditorScene {
        let mut scene = EditorScene::new();
        scene.terrain = Some(TerrainData::new(width, height));
        scene
    }

    // TerrainData tests
    #[test]
    fn test_terrain_data_new() {
        let t = TerrainData::new(4, 4);
        assert_eq!(t.width, 4);
        assert_eq!(t.height, 4);
        assert_eq!(t.heightmap.len(), 16);
        assert!(t.paint_layers.is_empty());
    }

    #[test]
    fn test_get_set_height() {
        let mut t = TerrainData::new(4, 4);
        assert_eq!(t.get_height(0, 0), Some(0.0));
        t.set_height(2, 3, 5.0);
        assert_eq!(t.get_height(2, 3), Some(5.0));
    }

    #[test]
    fn test_get_height_out_of_bounds() {
        let t = TerrainData::new(4, 4);
        assert_eq!(t.get_height(4, 0), None);
        assert_eq!(t.get_height(0, 4), None);
        assert_eq!(t.get_height(10, 10), None);
    }

    #[test]
    fn test_add_paint_layer() {
        let mut t = TerrainData::new(4, 4);
        let idx = t.add_paint_layer("grass".into());
        assert_eq!(idx, 0);
        assert_eq!(t.paint_layers.len(), 1);
        assert_eq!(t.paint_layers[0].texture_id, "grass");
        assert_eq!(t.paint_layers[0].weights.len(), 16);
    }

    // Sculpt command tests
    #[test]
    fn test_sculpt_raise() {
        let mut scene = scene_with_terrain(8, 8);
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(SculptCommand::new(TerrainBrush::Raise, 4, 4, 1, 2.0)),
                &mut scene,
            )
            .unwrap();

        let terrain = scene.terrain.as_ref().unwrap();
        let center_h = terrain.get_height(4, 4).unwrap();
        assert!(center_h > 0.0, "center should be raised");
    }

    #[test]
    fn test_sculpt_lower() {
        let mut scene = scene_with_terrain(8, 8);
        // Set initial height
        scene
            .terrain
            .as_mut()
            .unwrap()
            .set_height(4, 4, 10.0);

        let mut stack = UndoStack::new();
        stack
            .push(
                Box::new(SculptCommand::new(TerrainBrush::Lower, 4, 4, 0, 3.0)),
                &mut scene,
            )
            .unwrap();

        let h = scene.terrain.as_ref().unwrap().get_height(4, 4).unwrap();
        assert!(h < 10.0, "center should be lowered");
    }

    #[test]
    fn test_sculpt_smooth() {
        let mut scene = scene_with_terrain(8, 8);
        // Create a spike
        scene
            .terrain
            .as_mut()
            .unwrap()
            .set_height(4, 4, 10.0);

        let mut stack = UndoStack::new();
        stack
            .push(
                Box::new(SculptCommand::new(TerrainBrush::Smooth, 4, 4, 0, 1.0)),
                &mut scene,
            )
            .unwrap();

        let h = scene.terrain.as_ref().unwrap().get_height(4, 4).unwrap();
        // Should move toward neighbor average (which is 0)
        assert!(h < 10.0, "smooth should reduce spike");
    }

    #[test]
    fn test_sculpt_flatten() {
        let mut scene = scene_with_terrain(8, 8);
        scene
            .terrain
            .as_mut()
            .unwrap()
            .set_height(4, 4, 5.0);
        scene
            .terrain
            .as_mut()
            .unwrap()
            .set_height(5, 4, 10.0);

        let mut stack = UndoStack::new();
        // Use radius=2 so the neighbor at distance 1 gets a non-zero falloff
        stack
            .push(
                Box::new(SculptCommand::new(TerrainBrush::Flatten, 4, 4, 2, 1.0)),
                &mut scene,
            )
            .unwrap();

        let h_neighbor = scene.terrain.as_ref().unwrap().get_height(5, 4).unwrap();
        // Neighbor should be flattened toward center (5.0)
        assert!(h_neighbor < 10.0, "neighbor should flatten toward center");
    }

    #[test]
    fn test_sculpt_undo() {
        let mut scene = scene_with_terrain(8, 8);
        let mut stack = UndoStack::new();

        let original = scene
            .terrain
            .as_ref()
            .unwrap()
            .get_height(4, 4)
            .unwrap();

        stack
            .push(
                Box::new(SculptCommand::new(TerrainBrush::Raise, 4, 4, 1, 5.0)),
                &mut scene,
            )
            .unwrap();

        let raised = scene
            .terrain
            .as_ref()
            .unwrap()
            .get_height(4, 4)
            .unwrap();
        assert!(raised != original);

        stack.undo(&mut scene).unwrap();
        let restored = scene
            .terrain
            .as_ref()
            .unwrap()
            .get_height(4, 4)
            .unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn test_sculpt_no_terrain_error() {
        let mut scene = EditorScene::new();
        let mut cmd = SculptCommand::new(TerrainBrush::Raise, 0, 0, 1, 1.0);
        let result = cmd.execute(&mut scene);
        assert!(result.is_err());
    }

    // Paint command tests
    #[test]
    fn test_paint_applies_weight() {
        let mut scene = scene_with_terrain(8, 8);
        scene
            .terrain
            .as_mut()
            .unwrap()
            .add_paint_layer("grass".into());

        let mut stack = UndoStack::new();
        stack
            .push(
                Box::new(PaintCommand::new(0, 4, 4, 0, 0.5)),
                &mut scene,
            )
            .unwrap();

        let w = scene.terrain.as_ref().unwrap().paint_layers[0].weights
            [4 * 8 + 4];
        assert!(w > 0.0);
    }

    #[test]
    fn test_paint_clamped_to_one() {
        let mut scene = scene_with_terrain(8, 8);
        scene
            .terrain
            .as_mut()
            .unwrap()
            .add_paint_layer("grass".into());

        let mut stack = UndoStack::new();
        stack
            .push(
                Box::new(PaintCommand::new(0, 4, 4, 0, 2.0)),
                &mut scene,
            )
            .unwrap();

        let w = scene.terrain.as_ref().unwrap().paint_layers[0].weights
            [4 * 8 + 4];
        assert!(w <= 1.0, "weight should be clamped to 1.0");
    }

    #[test]
    fn test_paint_undo() {
        let mut scene = scene_with_terrain(8, 8);
        scene
            .terrain
            .as_mut()
            .unwrap()
            .add_paint_layer("grass".into());

        let mut stack = UndoStack::new();
        stack
            .push(
                Box::new(PaintCommand::new(0, 4, 4, 1, 0.8)),
                &mut scene,
            )
            .unwrap();

        stack.undo(&mut scene).unwrap();

        let w = scene.terrain.as_ref().unwrap().paint_layers[0].weights
            [4 * 8 + 4];
        assert_eq!(w, 0.0, "should be restored to original");
    }

    #[test]
    fn test_paint_invalid_layer_index() {
        let mut scene = scene_with_terrain(8, 8);
        let mut cmd = PaintCommand::new(5, 4, 4, 1, 0.5);
        let result = cmd.execute(&mut scene);
        assert!(result.is_err());
    }

    // Vegetation tests
    #[test]
    fn test_place_vegetation() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlaceVegetationCommand::new(
                    "oak_tree".into(),
                    Position::new(5.0, 0.0, 3.0),
                )),
                &mut scene,
            )
            .unwrap();

        assert_eq!(scene.objects.len(), 1);
        assert!(matches!(
            scene.objects[0].kind,
            ObjectKind::Vegetation { .. }
        ));
    }

    #[test]
    fn test_place_vegetation_undo() {
        let mut scene = EditorScene::new();
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(PlaceVegetationCommand::new(
                    "bush".into(),
                    Position::zero(),
                )),
                &mut scene,
            )
            .unwrap();
        assert_eq!(scene.objects.len(), 1);

        stack.undo(&mut scene).unwrap();
        assert_eq!(scene.objects.len(), 0);
    }

    #[test]
    fn test_sculpt_falloff_decreases_with_distance() {
        let mut scene = scene_with_terrain(16, 16);
        let mut stack = UndoStack::new();

        stack
            .push(
                Box::new(SculptCommand::new(TerrainBrush::Raise, 8, 8, 3, 5.0)),
                &mut scene,
            )
            .unwrap();

        let terrain = scene.terrain.as_ref().unwrap();
        let center_h = terrain.get_height(8, 8).unwrap();
        let edge_h = terrain.get_height(8, 6).unwrap();

        assert!(
            center_h > edge_h,
            "center ({center_h}) should be higher than edge ({edge_h})"
        );
    }
}
