//! Entity, Prop, and Component schemas (task 68).
//!
//! These mirror the runtime types in `aether-ecs` but are *declarative*:
//! every field must be expressible in YAML/JSON. No raw pointers, no closures,
//! no Rust-code-only values. A component value is restricted to a small
//! typed tree defined by [`ComponentValue`].

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{SchemaError, SchemaResult};

/// Declarative entity. Mirrors `aether_ecs::Entity` but is keyed by a stable
/// string ID rather than the runtime generational index.
///
/// The runtime resolves `id` to a generational `aether_ecs::Entity` at load
/// time; the declarative form is insulated from ID recycling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Entity {
    /// Stable, human- and agent-readable identifier. Must be unique within a
    /// world manifest.
    pub id: String,

    /// Semantic role of this entity. Hints the runtime at archetype placement.
    pub kind: EntityKind,

    /// World transform at spawn time.
    pub transform: Transform,

    /// Components attached to this entity at spawn time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<Component>,

    /// Optional parent entity `id`. If set, this entity is spawned as a child
    /// in the scene graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,

    /// Free-form tags for agent filtering (e.g., "boss", "merchant").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl Entity {
    /// Validate semantic constraints that cannot be enforced by serde alone.
    pub fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        if self.id.trim().is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/id"),
                "entity id must be non-empty",
                "assign a stable, unique `id` such as `boss.giant_spider`",
            ));
        }
        self.transform
            .validate(&format!("{pointer_base}/transform"))?;
        for (i, component) in self.components.iter().enumerate() {
            component.validate(&format!("{pointer_base}/components/{i}"))?;
        }
        Ok(())
    }
}

/// Common entity shapes. Extra kinds are expected to be added later; readers
/// must therefore treat `Custom` as an extension hatch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    /// A static or kinematic prop (mesh, collider, etc.).
    Prop,
    /// A player or agent-controllable character.
    Character,
    /// An interactable NPC.
    Npc,
    /// A non-visible gameplay trigger (volume).
    Trigger,
    /// Anything else; resolvable by user-space systems.
    Custom(String),
}

/// Rigid-body transform in world space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    pub position: [f32; 3],
    pub rotation_euler_deg: [f32; 3],
    #[serde(default = "Transform::default_scale")]
    pub scale: [f32; 3],
}

impl Transform {
    fn default_scale() -> [f32; 3] {
        [1.0, 1.0, 1.0]
    }

    /// The identity transform.
    pub fn identity() -> Self {
        Transform {
            position: [0.0, 0.0, 0.0],
            rotation_euler_deg: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
    }

    fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        for (i, v) in self.position.iter().enumerate() {
            if !v.is_finite() {
                return Err(SchemaError::validation(
                    format!("{pointer_base}/position/{i}"),
                    "position component must be a finite float",
                    "replace NaN/Inf with a finite value",
                ));
            }
        }
        for (i, v) in self.scale.iter().enumerate() {
            if !v.is_finite() || *v == 0.0 {
                return Err(SchemaError::validation(
                    format!("{pointer_base}/scale/{i}"),
                    "scale component must be finite and non-zero",
                    "use a positive scale factor such as 1.0",
                ));
            }
        }
        Ok(())
    }
}

impl Default for Transform {
    fn default() -> Self {
        Transform::identity()
    }
}

/// A declarative component attached to an entity.
///
/// Components are addressed by string `ty` (for example `"physics.rigid_body"`
/// or `"render.mesh"`); the runtime dispatches to the registered component
/// type. This decoupling is what lets the agent emit arbitrary components
/// without the host Rust code needing to enumerate every possible type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Component {
    /// Dotted-path type identifier.
    pub ty: String,

    /// The structured value payload.
    pub value: ComponentValue,
}

impl Component {
    fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        if self.ty.trim().is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/ty"),
                "component type must be non-empty",
                "use a dotted-path identifier such as `physics.rigid_body`",
            ));
        }
        Ok(())
    }
}

/// Declarative component value. Restricted to a small typed tree so that both
/// humans and agents can author it without consulting the Rust type system.
///
/// The `Ref` variant links to another declared entity by `id`, making cross-
/// entity references first-class (instead of smuggling them through strings).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ComponentValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    /// Entity reference by declarative `id`. Must come before `Map` so that
    /// serde's untagged deserializer matches the `$ref` shape first.
    Ref {
        #[serde(rename = "$ref")]
        ref_id: String,
    },
    Vec(Vec<ComponentValue>),
    Map(BTreeMap<String, ComponentValue>),
}

/// A declarative prop definition — a reusable spawnable template.
///
/// An [`Entity`] carries runtime state (position, transform); a [`Prop`]
/// declares the *template* from which many entities may be spawned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Prop {
    pub id: String,
    /// Asset CID (content-addressed mesh/animation/etc.).
    pub asset_cid: String,
    /// Default components applied when spawning.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_components: Vec<Component>,
    /// Default transform used when a spawn point doesn't override it.
    #[serde(default)]
    pub default_transform: Transform,
    /// Free-form tags for agent filtering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl Prop {
    /// Validate prop constraints.
    pub fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        if self.id.trim().is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/id"),
                "prop id must be non-empty",
                "assign a stable, unique `id`",
            ));
        }
        if self.asset_cid.trim().is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/asset_cid"),
                "prop asset_cid must be non-empty",
                "set asset_cid to a valid `cid:v1:<64 hex chars>`",
            ));
        }
        self.default_transform
            .validate(&format!("{pointer_base}/default_transform"))?;
        for (i, c) in self.default_components.iter().enumerate() {
            c.validate(&format!("{pointer_base}/default_components/{i}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_roundtrip() {
        let e = Entity {
            id: "giant_spider".into(),
            kind: EntityKind::Npc,
            transform: Transform::identity(),
            components: vec![Component {
                ty: "physics.rigid_body".into(),
                value: ComponentValue::Map({
                    let mut m = BTreeMap::new();
                    m.insert("mass".into(), ComponentValue::Float(120.0));
                    m
                }),
            }],
            parent: None,
            tags: vec!["boss".into()],
        };
        let json = serde_json::to_value(&e).unwrap();
        let back: Entity = serde_json::from_value(json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn transform_rejects_nan() {
        let t = Transform {
            position: [f32::NAN, 0.0, 0.0],
            rotation_euler_deg: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        };
        assert!(t.validate("/e/transform").is_err());
    }

    #[test]
    fn transform_rejects_zero_scale() {
        let t = Transform {
            position: [0.0, 0.0, 0.0],
            rotation_euler_deg: [0.0, 0.0, 0.0],
            scale: [1.0, 0.0, 1.0],
        };
        assert!(t.validate("/e/transform").is_err());
    }

    #[test]
    fn entity_empty_id_is_rejected() {
        let e = Entity {
            id: String::new(),
            kind: EntityKind::Prop,
            transform: Transform::identity(),
            components: vec![],
            parent: None,
            tags: vec![],
        };
        let err = e.validate("/entities/0").unwrap_err();
        assert_eq!(err.pointer(), "/entities/0/id");
    }

    #[test]
    fn component_ref_roundtrip() {
        let v = ComponentValue::Ref {
            ref_id: "giant_spider".into(),
        };
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, r#"{"$ref":"giant_spider"}"#);
        let back: ComponentValue = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn prop_validation_catches_missing_cid() {
        let p = Prop {
            id: "oak_tree".into(),
            asset_cid: "".into(),
            default_components: vec![],
            default_transform: Transform::identity(),
            tags: vec![],
        };
        let err = p.validate("/props/0").unwrap_err();
        assert_eq!(err.pointer(), "/props/0/asset_cid");
    }
}
