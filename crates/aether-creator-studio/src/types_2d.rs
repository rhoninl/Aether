//! 2D-specific types for sprites, tilemaps, physics, and lighting.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Transform
// ---------------------------------------------------------------------------

/// 2D transform: position (x,y), angle in radians, scale (x,y).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transform2D {
    pub position: [f32; 2],
    pub angle: f32,
    pub scale: [f32; 2],
}

impl Default for Transform2D {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            angle: 0.0,
            scale: [1.0, 1.0],
        }
    }
}

// ---------------------------------------------------------------------------
// Physics
// ---------------------------------------------------------------------------

/// 2D collider shapes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Collider2D {
    Box { half_extents: [f32; 2] },
    Circle { radius: f32 },
    Polygon { vertices: Vec<[f32; 2]> },
}

/// 2D rigid body type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BodyType2D {
    #[default]
    Static,
    Dynamic,
    Kinematic,
}

/// 2D rigid body configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RigidBody2D {
    pub body_type: BodyType2D,
    pub collider: Collider2D,
    pub fixed_rotation: bool,
    pub is_sensor: bool,
}

// ---------------------------------------------------------------------------
// Lighting
// ---------------------------------------------------------------------------

/// Falloff model for 2D point lights.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Falloff2D {
    #[default]
    Linear,
    Quadratic,
}

/// 2D light types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Light2D {
    Point {
        position: [f32; 2],
        color: [f32; 3],
        intensity: f32,
        radius: f32,
        falloff: Falloff2D,
    },
    Spot {
        position: [f32; 2],
        color: [f32; 3],
        intensity: f32,
        radius: f32,
        angle: f32,
        direction: f32,
    },
    Global {
        color: [f32; 3],
        intensity: f32,
    },
}

// ---------------------------------------------------------------------------
// Sprites
// ---------------------------------------------------------------------------

/// Sprite entity definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpriteEntity {
    pub sprite: String,
    pub animation: Option<String>,
    pub flip_x: bool,
    pub flip_y: bool,
    pub layer: String,
    pub z_order: i32,
}

impl Default for SpriteEntity {
    fn default() -> Self {
        Self {
            sprite: String::new(),
            animation: None,
            flip_x: false,
            flip_y: false,
            layer: "default".to_string(),
            z_order: 0,
        }
    }
}

/// A single animation definition inside a sprite sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationDef {
    pub name: String,
    pub frames: Vec<u32>,
    pub fps: u32,
    pub looping: bool,
}

/// Sprite sheet definition (maps to .sheet.toml files).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpriteSheetDef {
    pub image: String,
    pub frame_width: u32,
    pub frame_height: u32,
    pub animations: Vec<AnimationDef>,
}

// ---------------------------------------------------------------------------
// Tilemaps
// ---------------------------------------------------------------------------

/// Auto-tile configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoTileConfig {
    pub enabled: bool,
    pub rules: String,
}

/// Tilemap data for a single layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TilemapData {
    pub tileset: String,
    pub tile_size: [u32; 2],
    pub width: u32,
    pub height: u32,
    pub layer: String,
    pub data: Vec<u32>,
    pub auto_tile: Option<AutoTileConfig>,
}

/// Tileset definition (maps to .tileset.toml files).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TilesetDef {
    pub image: String,
    pub tile_width: u32,
    pub tile_height: u32,
    pub tile_count: u32,
    pub columns: u32,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Transform2D ---------------------------------------------------------

    #[test]
    fn test_transform2d_default() {
        let t = Transform2D::default();
        assert_eq!(t.position, [0.0, 0.0]);
        assert_eq!(t.angle, 0.0);
        assert_eq!(t.scale, [1.0, 1.0]);
    }

    #[test]
    fn test_transform2d_round_trip() {
        let t = Transform2D {
            position: [10.5, -3.2],
            angle: 1.57,
            scale: [2.0, 0.5],
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: Transform2D = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn test_transform2d_default_round_trip() {
        let t = Transform2D::default();
        let json = serde_json::to_string(&t).unwrap();
        let back: Transform2D = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    // -- Collider2D ----------------------------------------------------------

    #[test]
    fn test_collider2d_box_round_trip() {
        let c = Collider2D::Box {
            half_extents: [1.0, 2.0],
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Collider2D = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn test_collider2d_circle_round_trip() {
        let c = Collider2D::Circle { radius: 5.0 };
        let json = serde_json::to_string(&c).unwrap();
        let back: Collider2D = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn test_collider2d_polygon_round_trip() {
        let c = Collider2D::Polygon {
            vertices: vec![[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]],
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Collider2D = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn test_collider2d_polygon_empty_vertices() {
        let c = Collider2D::Polygon { vertices: vec![] };
        let json = serde_json::to_string(&c).unwrap();
        let back: Collider2D = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    // -- BodyType2D ----------------------------------------------------------

    #[test]
    fn test_body_type2d_default() {
        assert_eq!(BodyType2D::default(), BodyType2D::Static);
    }

    #[test]
    fn test_body_type2d_round_trip() {
        for bt in [
            BodyType2D::Static,
            BodyType2D::Dynamic,
            BodyType2D::Kinematic,
        ] {
            let json = serde_json::to_string(&bt).unwrap();
            let back: BodyType2D = serde_json::from_str(&json).unwrap();
            assert_eq!(bt, back);
        }
    }

    // -- RigidBody2D ---------------------------------------------------------

    #[test]
    fn test_rigid_body2d_round_trip() {
        let rb = RigidBody2D {
            body_type: BodyType2D::Dynamic,
            collider: Collider2D::Circle { radius: 1.0 },
            fixed_rotation: true,
            is_sensor: false,
        };
        let json = serde_json::to_string(&rb).unwrap();
        let back: RigidBody2D = serde_json::from_str(&json).unwrap();
        assert_eq!(rb, back);
    }

    #[test]
    fn test_rigid_body2d_sensor() {
        let rb = RigidBody2D {
            body_type: BodyType2D::Static,
            collider: Collider2D::Box {
                half_extents: [2.0, 3.0],
            },
            fixed_rotation: false,
            is_sensor: true,
        };
        let json = serde_json::to_string(&rb).unwrap();
        let back: RigidBody2D = serde_json::from_str(&json).unwrap();
        assert_eq!(rb, back);
    }

    // -- Falloff2D -----------------------------------------------------------

    #[test]
    fn test_falloff2d_default() {
        assert_eq!(Falloff2D::default(), Falloff2D::Linear);
    }

    #[test]
    fn test_falloff2d_round_trip() {
        for f in [Falloff2D::Linear, Falloff2D::Quadratic] {
            let json = serde_json::to_string(&f).unwrap();
            let back: Falloff2D = serde_json::from_str(&json).unwrap();
            assert_eq!(f, back);
        }
    }

    // -- Light2D -------------------------------------------------------------

    #[test]
    fn test_light2d_point_round_trip() {
        let l = Light2D::Point {
            position: [10.0, 20.0],
            color: [1.0, 0.8, 0.2],
            intensity: 5.0,
            radius: 100.0,
            falloff: Falloff2D::Quadratic,
        };
        let json = serde_json::to_string(&l).unwrap();
        let back: Light2D = serde_json::from_str(&json).unwrap();
        assert_eq!(l, back);
    }

    #[test]
    fn test_light2d_spot_round_trip() {
        let l = Light2D::Spot {
            position: [5.0, 5.0],
            color: [1.0, 1.0, 1.0],
            intensity: 3.0,
            radius: 50.0,
            angle: 0.78,
            direction: 1.57,
        };
        let json = serde_json::to_string(&l).unwrap();
        let back: Light2D = serde_json::from_str(&json).unwrap();
        assert_eq!(l, back);
    }

    #[test]
    fn test_light2d_global_round_trip() {
        let l = Light2D::Global {
            color: [0.5, 0.5, 0.5],
            intensity: 1.0,
        };
        let json = serde_json::to_string(&l).unwrap();
        let back: Light2D = serde_json::from_str(&json).unwrap();
        assert_eq!(l, back);
    }

    // -- SpriteEntity --------------------------------------------------------

    #[test]
    fn test_sprite_entity_default() {
        let s = SpriteEntity::default();
        assert!(s.sprite.is_empty());
        assert!(s.animation.is_none());
        assert!(!s.flip_x);
        assert!(!s.flip_y);
        assert_eq!(s.layer, "default");
        assert_eq!(s.z_order, 0);
    }

    #[test]
    fn test_sprite_entity_round_trip() {
        let s = SpriteEntity {
            sprite: "hero.png".to_string(),
            animation: Some("walk".to_string()),
            flip_x: true,
            flip_y: false,
            layer: "foreground".to_string(),
            z_order: 10,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SpriteEntity = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn test_sprite_entity_no_animation() {
        let s = SpriteEntity {
            sprite: "tree.png".to_string(),
            animation: None,
            flip_x: false,
            flip_y: false,
            layer: "background".to_string(),
            z_order: -5,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SpriteEntity = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    // -- AnimationDef --------------------------------------------------------

    #[test]
    fn test_animation_def_round_trip() {
        let a = AnimationDef {
            name: "idle".to_string(),
            frames: vec![0, 1, 2, 3],
            fps: 12,
            looping: true,
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: AnimationDef = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn test_animation_def_no_loop() {
        let a = AnimationDef {
            name: "death".to_string(),
            frames: vec![0, 1, 2],
            fps: 8,
            looping: false,
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: AnimationDef = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn test_animation_def_empty_frames() {
        let a = AnimationDef {
            name: "empty".to_string(),
            frames: vec![],
            fps: 1,
            looping: false,
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: AnimationDef = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }

    // -- SpriteSheetDef ------------------------------------------------------

    #[test]
    fn test_sprite_sheet_def_round_trip() {
        let s = SpriteSheetDef {
            image: "hero_sheet.png".to_string(),
            frame_width: 32,
            frame_height: 32,
            animations: vec![
                AnimationDef {
                    name: "walk".to_string(),
                    frames: vec![0, 1, 2, 3],
                    fps: 12,
                    looping: true,
                },
                AnimationDef {
                    name: "jump".to_string(),
                    frames: vec![4, 5],
                    fps: 8,
                    looping: false,
                },
            ],
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SpriteSheetDef = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn test_sprite_sheet_def_no_animations() {
        let s = SpriteSheetDef {
            image: "tiles.png".to_string(),
            frame_width: 16,
            frame_height: 16,
            animations: vec![],
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SpriteSheetDef = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    // -- AutoTileConfig ------------------------------------------------------

    #[test]
    fn test_auto_tile_config_round_trip() {
        let a = AutoTileConfig {
            enabled: true,
            rules: "blob47".to_string(),
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: AutoTileConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }

    // -- TilemapData ---------------------------------------------------------

    #[test]
    fn test_tilemap_data_round_trip() {
        let t = TilemapData {
            tileset: "forest.tileset".to_string(),
            tile_size: [16, 16],
            width: 3,
            height: 2,
            layer: "ground".to_string(),
            data: vec![1, 2, 3, 4, 5, 6],
            auto_tile: Some(AutoTileConfig {
                enabled: true,
                rules: "wang".to_string(),
            }),
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: TilemapData = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn test_tilemap_data_no_auto_tile() {
        let t = TilemapData {
            tileset: "dungeon.tileset".to_string(),
            tile_size: [32, 32],
            width: 10,
            height: 10,
            layer: "floor".to_string(),
            data: vec![0; 100],
            auto_tile: None,
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: TilemapData = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn test_tilemap_data_empty() {
        let t = TilemapData {
            tileset: "empty.tileset".to_string(),
            tile_size: [8, 8],
            width: 0,
            height: 0,
            layer: "default".to_string(),
            data: vec![],
            auto_tile: None,
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: TilemapData = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    // -- TilesetDef ----------------------------------------------------------

    #[test]
    fn test_tileset_def_round_trip() {
        let t = TilesetDef {
            image: "forest_tiles.png".to_string(),
            tile_width: 16,
            tile_height: 16,
            tile_count: 256,
            columns: 16,
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: TilesetDef = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }
}
