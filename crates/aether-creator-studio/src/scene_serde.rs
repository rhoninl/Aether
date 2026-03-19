//! Scene serialization for both 2D and 3D worlds.
//!
//! Provides serializable scene structs and JSON round-trip functions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types_2d::{Light2D, RigidBody2D, SpriteEntity, Transform2D};

/// Serialization/deserialization error type.
pub type SerdeError = serde_json::Error;

// ---------------------------------------------------------------------------
// 3D Scene types
// ---------------------------------------------------------------------------

/// 3D transform with position, quaternion rotation, and scale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transform3D {
    pub position: [f32; 3],
    /// Quaternion [x, y, z, w].
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for Transform3D {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

/// 3D collider configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Collider3DConfig {
    Box { half_extents: [f32; 3] },
    Sphere { radius: f32 },
    Capsule { radius: f32, height: f32 },
    Mesh { asset: String },
}

/// 3D physics configuration for an entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Physics3D {
    pub body_type: String,
    pub collider: Collider3DConfig,
}

/// 3D light types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Light3D {
    Point {
        position: [f32; 3],
        color: [f32; 3],
        intensity: f32,
        range: f32,
    },
    Directional {
        direction: [f32; 3],
        color: [f32; 3],
        intensity: f32,
    },
    Spot {
        position: [f32; 3],
        direction: [f32; 3],
        color: [f32; 3],
        intensity: f32,
        range: f32,
        inner_angle: f32,
        outer_angle: f32,
    },
}

/// A single 3D entity in a scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity3D {
    pub id: String,
    /// Entity kind: "Prop", "SpawnPoint", etc.
    pub kind: String,
    pub template: Option<String>,
    pub scripts: Vec<String>,
    pub transform: Transform3D,
    pub physics: Option<Physics3D>,
}

/// A serializable 3D scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene3D {
    pub name: String,
    pub description: String,
    pub entities: Vec<Entity3D>,
    pub lights: Vec<Light3D>,
}

// ---------------------------------------------------------------------------
// 2D Scene types
// ---------------------------------------------------------------------------

/// A single 2D entity in a scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity2D {
    pub id: String,
    /// Entity kind: "Sprite", "SpawnPoint", etc.
    pub kind: String,
    pub sprite: Option<SpriteEntity>,
    pub scripts: Vec<String>,
    pub transform: Transform2D,
    pub physics: Option<RigidBody2D>,
}

/// A serializable 2D scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene2D {
    pub name: String,
    pub description: String,
    /// layer_name -> tilemap file path
    pub tilemaps: HashMap<String, String>,
    pub entities: Vec<Entity2D>,
    pub lights: Vec<Light2D>,
}

// ---------------------------------------------------------------------------
// Serialization functions
// ---------------------------------------------------------------------------

/// Serialize a 3D scene to a JSON string.
pub fn serialize_scene_3d(scene: &Scene3D) -> Result<String, SerdeError> {
    serde_json::to_string_pretty(scene)
}

/// Deserialize a 3D scene from a JSON string.
pub fn deserialize_scene_3d(json: &str) -> Result<Scene3D, SerdeError> {
    serde_json::from_str(json)
}

/// Serialize a 2D scene to a JSON string.
pub fn serialize_scene_2d(scene: &Scene2D) -> Result<String, SerdeError> {
    serde_json::to_string_pretty(scene)
}

/// Deserialize a 2D scene from a JSON string.
pub fn deserialize_scene_2d(json: &str) -> Result<Scene2D, SerdeError> {
    serde_json::from_str(json)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types_2d::{
        BodyType2D, Collider2D, Falloff2D, Light2D, RigidBody2D, SpriteEntity, Transform2D,
    };

    // -- Transform3D ---------------------------------------------------------

    #[test]
    fn test_transform3d_default() {
        let t = Transform3D::default();
        assert_eq!(t.position, [0.0, 0.0, 0.0]);
        assert_eq!(t.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(t.scale, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_transform3d_round_trip() {
        let t = Transform3D {
            position: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.707, 0.0, 0.707],
            scale: [2.0, 2.0, 2.0],
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: Transform3D = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    // -- Collider3DConfig ----------------------------------------------------

    #[test]
    fn test_collider3d_box_round_trip() {
        let c = Collider3DConfig::Box {
            half_extents: [1.0, 2.0, 3.0],
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Collider3DConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn test_collider3d_sphere_round_trip() {
        let c = Collider3DConfig::Sphere { radius: 5.0 };
        let json = serde_json::to_string(&c).unwrap();
        let back: Collider3DConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn test_collider3d_capsule_round_trip() {
        let c = Collider3DConfig::Capsule {
            radius: 0.5,
            height: 2.0,
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Collider3DConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn test_collider3d_mesh_round_trip() {
        let c = Collider3DConfig::Mesh {
            asset: "meshes/rock.glb".to_string(),
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Collider3DConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    // -- Physics3D -----------------------------------------------------------

    #[test]
    fn test_physics3d_round_trip() {
        let p = Physics3D {
            body_type: "Dynamic".to_string(),
            collider: Collider3DConfig::Sphere { radius: 1.0 },
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: Physics3D = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    // -- Light3D -------------------------------------------------------------

    #[test]
    fn test_light3d_point_round_trip() {
        let l = Light3D::Point {
            position: [0.0, 10.0, 0.0],
            color: [1.0, 1.0, 0.9],
            intensity: 100.0,
            range: 50.0,
        };
        let json = serde_json::to_string(&l).unwrap();
        let back: Light3D = serde_json::from_str(&json).unwrap();
        assert_eq!(l, back);
    }

    #[test]
    fn test_light3d_directional_round_trip() {
        let l = Light3D::Directional {
            direction: [0.0, -1.0, 0.5],
            color: [1.0, 0.95, 0.8],
            intensity: 1.0,
        };
        let json = serde_json::to_string(&l).unwrap();
        let back: Light3D = serde_json::from_str(&json).unwrap();
        assert_eq!(l, back);
    }

    #[test]
    fn test_light3d_spot_round_trip() {
        let l = Light3D::Spot {
            position: [5.0, 5.0, 5.0],
            direction: [0.0, -1.0, 0.0],
            color: [1.0, 1.0, 1.0],
            intensity: 50.0,
            range: 20.0,
            inner_angle: 0.3,
            outer_angle: 0.6,
        };
        let json = serde_json::to_string(&l).unwrap();
        let back: Light3D = serde_json::from_str(&json).unwrap();
        assert_eq!(l, back);
    }

    // -- Entity3D ------------------------------------------------------------

    #[test]
    fn test_entity3d_full_round_trip() {
        let e = Entity3D {
            id: "ent-001".to_string(),
            kind: "Prop".to_string(),
            template: Some("crate_wooden".to_string()),
            scripts: vec!["breakable.lua".to_string()],
            transform: Transform3D {
                position: [10.0, 0.0, -5.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
            },
            physics: Some(Physics3D {
                body_type: "Dynamic".to_string(),
                collider: Collider3DConfig::Box {
                    half_extents: [0.5, 0.5, 0.5],
                },
            }),
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: Entity3D = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn test_entity3d_minimal() {
        let e = Entity3D {
            id: "spawn-1".to_string(),
            kind: "SpawnPoint".to_string(),
            template: None,
            scripts: vec![],
            transform: Transform3D::default(),
            physics: None,
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: Entity3D = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    // -- Scene3D round-trip --------------------------------------------------

    #[test]
    fn test_scene3d_full_round_trip() {
        let scene = Scene3D {
            name: "Main Hall".to_string(),
            description: "The central hub area".to_string(),
            entities: vec![
                Entity3D {
                    id: "prop-1".to_string(),
                    kind: "Prop".to_string(),
                    template: Some("pillar".to_string()),
                    scripts: vec![],
                    transform: Transform3D {
                        position: [0.0, 0.0, 0.0],
                        rotation: [0.0, 0.0, 0.0, 1.0],
                        scale: [1.0, 1.0, 1.0],
                    },
                    physics: Some(Physics3D {
                        body_type: "Static".to_string(),
                        collider: Collider3DConfig::Box {
                            half_extents: [0.5, 5.0, 0.5],
                        },
                    }),
                },
                Entity3D {
                    id: "spawn-1".to_string(),
                    kind: "SpawnPoint".to_string(),
                    template: None,
                    scripts: vec![],
                    transform: Transform3D::default(),
                    physics: None,
                },
            ],
            lights: vec![Light3D::Directional {
                direction: [0.0, -1.0, 0.0],
                color: [1.0, 1.0, 0.95],
                intensity: 1.0,
            }],
        };
        let json = serialize_scene_3d(&scene).unwrap();
        let back = deserialize_scene_3d(&json).unwrap();
        assert_eq!(scene, back);
    }

    #[test]
    fn test_scene3d_empty() {
        let scene = Scene3D {
            name: "Empty".to_string(),
            description: String::new(),
            entities: vec![],
            lights: vec![],
        };
        let json = serialize_scene_3d(&scene).unwrap();
        let back = deserialize_scene_3d(&json).unwrap();
        assert_eq!(scene, back);
    }

    #[test]
    fn test_scene3d_multiple_lights() {
        let scene = Scene3D {
            name: "Lit".to_string(),
            description: String::new(),
            entities: vec![],
            lights: vec![
                Light3D::Point {
                    position: [0.0, 5.0, 0.0],
                    color: [1.0, 0.9, 0.8],
                    intensity: 10.0,
                    range: 20.0,
                },
                Light3D::Spot {
                    position: [5.0, 5.0, 0.0],
                    direction: [0.0, -1.0, 0.0],
                    color: [1.0, 1.0, 1.0],
                    intensity: 50.0,
                    range: 15.0,
                    inner_angle: 0.2,
                    outer_angle: 0.5,
                },
            ],
        };
        let json = serialize_scene_3d(&scene).unwrap();
        let back = deserialize_scene_3d(&json).unwrap();
        assert_eq!(scene, back);
    }

    #[test]
    fn test_scene3d_entity_with_multiple_scripts() {
        let scene = Scene3D {
            name: "Scripted".to_string(),
            description: String::new(),
            entities: vec![Entity3D {
                id: "npc-1".to_string(),
                kind: "Prop".to_string(),
                template: Some("npc_guard".to_string()),
                scripts: vec![
                    "patrol.lua".to_string(),
                    "dialogue.lua".to_string(),
                    "combat.lua".to_string(),
                ],
                transform: Transform3D::default(),
                physics: None,
            }],
            lights: vec![],
        };
        let json = serialize_scene_3d(&scene).unwrap();
        let back = deserialize_scene_3d(&json).unwrap();
        assert_eq!(scene, back);
    }

    #[test]
    fn test_deserialize_scene_3d_invalid_json() {
        let result = deserialize_scene_3d("not json");
        assert!(result.is_err());
    }

    // -- Entity2D ------------------------------------------------------------

    #[test]
    fn test_entity2d_full_round_trip() {
        let e = Entity2D {
            id: "player".to_string(),
            kind: "Sprite".to_string(),
            sprite: Some(SpriteEntity {
                sprite: "hero.png".to_string(),
                animation: Some("idle".to_string()),
                flip_x: false,
                flip_y: false,
                layer: "characters".to_string(),
                z_order: 10,
            }),
            scripts: vec!["player_controller.lua".to_string()],
            transform: Transform2D {
                position: [100.0, 200.0],
                angle: 0.0,
                scale: [1.0, 1.0],
            },
            physics: Some(RigidBody2D {
                body_type: BodyType2D::Dynamic,
                collider: Collider2D::Box {
                    half_extents: [8.0, 16.0],
                },
                fixed_rotation: true,
                is_sensor: false,
            }),
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: Entity2D = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn test_entity2d_minimal() {
        let e = Entity2D {
            id: "spawn-1".to_string(),
            kind: "SpawnPoint".to_string(),
            sprite: None,
            scripts: vec![],
            transform: Transform2D::default(),
            physics: None,
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: Entity2D = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    // -- Scene2D round-trip --------------------------------------------------

    #[test]
    fn test_scene2d_full_round_trip() {
        let mut tilemaps = HashMap::new();
        tilemaps.insert("ground".to_string(), "maps/ground.tilemap".to_string());
        tilemaps.insert(
            "collision".to_string(),
            "maps/collision.tilemap".to_string(),
        );

        let scene = Scene2D {
            name: "Level 1".to_string(),
            description: "First level of the game".to_string(),
            tilemaps,
            entities: vec![
                Entity2D {
                    id: "hero".to_string(),
                    kind: "Sprite".to_string(),
                    sprite: Some(SpriteEntity {
                        sprite: "hero.png".to_string(),
                        animation: Some("idle".to_string()),
                        flip_x: false,
                        flip_y: false,
                        layer: "characters".to_string(),
                        z_order: 10,
                    }),
                    scripts: vec!["player.lua".to_string()],
                    transform: Transform2D {
                        position: [64.0, 128.0],
                        angle: 0.0,
                        scale: [1.0, 1.0],
                    },
                    physics: Some(RigidBody2D {
                        body_type: BodyType2D::Dynamic,
                        collider: Collider2D::Box {
                            half_extents: [8.0, 16.0],
                        },
                        fixed_rotation: true,
                        is_sensor: false,
                    }),
                },
                Entity2D {
                    id: "spawn-1".to_string(),
                    kind: "SpawnPoint".to_string(),
                    sprite: None,
                    scripts: vec![],
                    transform: Transform2D::default(),
                    physics: None,
                },
            ],
            lights: vec![
                Light2D::Global {
                    color: [0.9, 0.85, 0.8],
                    intensity: 0.5,
                },
                Light2D::Point {
                    position: [200.0, 100.0],
                    color: [1.0, 0.7, 0.3],
                    intensity: 3.0,
                    radius: 64.0,
                    falloff: Falloff2D::Quadratic,
                },
            ],
        };
        let json = serialize_scene_2d(&scene).unwrap();
        let back = deserialize_scene_2d(&json).unwrap();
        assert_eq!(scene, back);
    }

    #[test]
    fn test_scene2d_empty() {
        let scene = Scene2D {
            name: "Empty".to_string(),
            description: String::new(),
            tilemaps: HashMap::new(),
            entities: vec![],
            lights: vec![],
        };
        let json = serialize_scene_2d(&scene).unwrap();
        let back = deserialize_scene_2d(&json).unwrap();
        assert_eq!(scene, back);
    }

    #[test]
    fn test_scene2d_tilemaps_only() {
        let mut tilemaps = HashMap::new();
        tilemaps.insert("ground".to_string(), "level1/ground.tilemap".to_string());

        let scene = Scene2D {
            name: "Map Only".to_string(),
            description: String::new(),
            tilemaps,
            entities: vec![],
            lights: vec![],
        };
        let json = serialize_scene_2d(&scene).unwrap();
        let back = deserialize_scene_2d(&json).unwrap();
        assert_eq!(scene, back);
    }

    #[test]
    fn test_scene2d_no_tilemaps() {
        let scene = Scene2D {
            name: "Sprites Only".to_string(),
            description: String::new(),
            tilemaps: HashMap::new(),
            entities: vec![Entity2D {
                id: "tree-1".to_string(),
                kind: "Sprite".to_string(),
                sprite: Some(SpriteEntity {
                    sprite: "tree.png".to_string(),
                    animation: None,
                    flip_x: false,
                    flip_y: false,
                    layer: "background".to_string(),
                    z_order: -10,
                }),
                scripts: vec![],
                transform: Transform2D {
                    position: [50.0, 80.0],
                    angle: 0.0,
                    scale: [1.0, 1.0],
                },
                physics: None,
            }],
            lights: vec![],
        };
        let json = serialize_scene_2d(&scene).unwrap();
        let back = deserialize_scene_2d(&json).unwrap();
        assert_eq!(scene, back);
    }

    #[test]
    fn test_deserialize_scene_2d_invalid_json() {
        let result = deserialize_scene_2d("{invalid}");
        assert!(result.is_err());
    }

    #[test]
    fn test_scene2d_missing_optional_fields() {
        // Entity with all optionals set to None
        let scene = Scene2D {
            name: "Minimal".to_string(),
            description: String::new(),
            tilemaps: HashMap::new(),
            entities: vec![Entity2D {
                id: "e1".to_string(),
                kind: "Empty".to_string(),
                sprite: None,
                scripts: vec![],
                transform: Transform2D::default(),
                physics: None,
            }],
            lights: vec![],
        };
        let json = serialize_scene_2d(&scene).unwrap();
        let back = deserialize_scene_2d(&json).unwrap();
        assert_eq!(scene, back);
    }

    #[test]
    fn test_scene3d_deserialize_preserves_field_values() {
        let scene = Scene3D {
            name: "Test".to_string(),
            description: "Desc".to_string(),
            entities: vec![Entity3D {
                id: "e1".to_string(),
                kind: "Prop".to_string(),
                template: Some("box".to_string()),
                scripts: vec!["a.lua".to_string()],
                transform: Transform3D {
                    position: [1.5, 2.5, 3.5],
                    rotation: [0.1, 0.2, 0.3, 0.9],
                    scale: [0.5, 0.5, 0.5],
                },
                physics: Some(Physics3D {
                    body_type: "Kinematic".to_string(),
                    collider: Collider3DConfig::Capsule {
                        radius: 0.3,
                        height: 1.8,
                    },
                }),
            }],
            lights: vec![],
        };
        let json = serialize_scene_3d(&scene).unwrap();
        let back = deserialize_scene_3d(&json).unwrap();
        assert_eq!(back.name, "Test");
        assert_eq!(back.description, "Desc");
        assert_eq!(back.entities.len(), 1);
        assert_eq!(back.entities[0].transform.position, [1.5, 2.5, 3.5]);
        assert_eq!(back.entities[0].transform.rotation, [0.1, 0.2, 0.3, 0.9]);
    }

    #[test]
    fn test_serialize_scene_3d_produces_valid_json() {
        let scene = Scene3D {
            name: "Test".to_string(),
            description: String::new(),
            entities: vec![],
            lights: vec![],
        };
        let json = serialize_scene_3d(&scene).unwrap();
        // Should be parseable as generic JSON value
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["name"], "Test");
    }

    #[test]
    fn test_serialize_scene_2d_produces_valid_json() {
        let scene = Scene2D {
            name: "Test 2D".to_string(),
            description: String::new(),
            tilemaps: HashMap::new(),
            entities: vec![],
            lights: vec![],
        };
        let json = serialize_scene_2d(&scene).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["name"], "Test 2D");
    }
}
