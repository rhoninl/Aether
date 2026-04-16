//! MMORPG primitives (task 71). Feature-gated behind `mmorpg`.
//!
//! These are deliberately minimal — just enough structure that agents can
//! generate coherent quests and loot tables, and the runtime can dispatch
//! objectives to game systems. Complex gameplay systems live downstream.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{SchemaError, SchemaResult};

/// A quest: an ordered list of objectives plus a reward bundle on completion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Quest {
    pub id: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Objectives are completed in order unless [`QuestObjective::parallel`]
    /// is set on a specific objective.
    pub objectives: Vec<QuestObjective>,

    /// Rewards granted on completion.
    #[serde(default)]
    pub rewards: QuestRewards,

    /// Optional minimum level gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_level: Option<u32>,

    /// Free-form tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl Quest {
    pub fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        if self.id.trim().is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/id"),
                "quest id must be non-empty",
                "assign a stable id such as `mq_prologue_01`",
            ));
        }
        if self.display_name.trim().is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/display_name"),
                "display_name must be non-empty",
                "set a user-facing title",
            ));
        }
        if self.objectives.is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/objectives"),
                "a quest must have at least one objective",
                "add at least one objective such as `kill 3 wolves`",
            ));
        }
        for (i, o) in self.objectives.iter().enumerate() {
            o.validate(&format!("{pointer_base}/objectives/{i}"))?;
        }
        Ok(())
    }
}

/// A single objective within a quest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuestObjective {
    pub id: String,
    pub kind: ObjectiveKind,
    /// Whether this objective can be completed in parallel with the next one.
    #[serde(default)]
    pub parallel: bool,
}

impl QuestObjective {
    fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        if self.id.trim().is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/id"),
                "objective id must be non-empty",
                "assign a stable id",
            ));
        }
        self.kind.validate(&format!("{pointer_base}/kind"))?;
        Ok(())
    }
}

/// Objective kinds. New variants go through versioning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObjectiveKind {
    Kill {
        /// Template id of the enemy to kill (matches [`crate::entity::Prop::id`]).
        target_prop: String,
        count: u32,
    },
    Collect {
        item_id: String,
        count: u32,
    },
    Reach {
        /// Reach a specific world position (x, y, z).
        position: [f32; 3],
        radius_meters: f32,
    },
    Talk {
        /// Entity id of the NPC to talk to.
        npc_entity: String,
    },
}

impl ObjectiveKind {
    fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        match self {
            ObjectiveKind::Kill { count, target_prop } => {
                if *count == 0 {
                    return Err(SchemaError::validation(
                        format!("{pointer_base}/count"),
                        "kill count must be > 0",
                        "set count to 1 or more",
                    ));
                }
                if target_prop.trim().is_empty() {
                    return Err(SchemaError::validation(
                        format!("{pointer_base}/target_prop"),
                        "target_prop must be non-empty",
                        "reference a declared prop id",
                    ));
                }
            }
            ObjectiveKind::Collect { count, item_id } => {
                if *count == 0 {
                    return Err(SchemaError::validation(
                        format!("{pointer_base}/count"),
                        "collect count must be > 0",
                        "set count to 1 or more",
                    ));
                }
                if item_id.trim().is_empty() {
                    return Err(SchemaError::validation(
                        format!("{pointer_base}/item_id"),
                        "item_id must be non-empty",
                        "reference a declared item id",
                    ));
                }
            }
            ObjectiveKind::Reach {
                position,
                radius_meters,
            } => {
                for (i, v) in position.iter().enumerate() {
                    if !v.is_finite() {
                        return Err(SchemaError::validation(
                            format!("{pointer_base}/position/{i}"),
                            "position must be finite",
                            "replace NaN/Inf with a finite value",
                        ));
                    }
                }
                if !radius_meters.is_finite() || *radius_meters <= 0.0 {
                    return Err(SchemaError::validation(
                        format!("{pointer_base}/radius_meters"),
                        "radius must be finite and positive",
                        "use e.g. 5.0",
                    ));
                }
            }
            ObjectiveKind::Talk { npc_entity } => {
                if npc_entity.trim().is_empty() {
                    return Err(SchemaError::validation(
                        format!("{pointer_base}/npc_entity"),
                        "npc_entity must be non-empty",
                        "reference a declared entity id",
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Rewards granted on quest completion.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuestRewards {
    #[serde(default)]
    pub experience: u64,
    #[serde(default)]
    pub currency: u64,
    /// Items granted on completion, keyed by item id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<LootEntry>,
    /// Optional loot table reference for randomized drops.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loot_table: Option<String>,
}

/// A loot table: weighted random entries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LootTable {
    pub id: String,
    pub entries: Vec<LootEntry>,
    /// Max number of distinct entries to draw in a single roll.
    #[serde(default = "LootTable::default_rolls")]
    pub rolls: u32,
}

impl LootTable {
    fn default_rolls() -> u32 {
        1
    }

    pub fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        if self.id.trim().is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/id"),
                "loot table id must be non-empty",
                "assign a stable id",
            ));
        }
        if self.entries.is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/entries"),
                "a loot table must have at least one entry",
                "add at least one entry",
            ));
        }
        for (i, e) in self.entries.iter().enumerate() {
            e.validate(&format!("{pointer_base}/entries/{i}"))?;
        }
        if self.rolls == 0 {
            return Err(SchemaError::validation(
                format!("{pointer_base}/rolls"),
                "rolls must be > 0",
                "set rolls to 1 or more",
            ));
        }
        Ok(())
    }
}

/// A single weighted loot entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LootEntry {
    pub item_id: String,
    /// Non-negative weight; higher is more likely.
    pub weight: f32,
    #[serde(default = "LootEntry::default_min")]
    pub min_count: u32,
    #[serde(default = "LootEntry::default_max")]
    pub max_count: u32,
}

impl LootEntry {
    fn default_min() -> u32 {
        1
    }
    fn default_max() -> u32 {
        1
    }

    fn validate(&self, pointer_base: &str) -> SchemaResult<()> {
        if self.item_id.trim().is_empty() {
            return Err(SchemaError::validation(
                format!("{pointer_base}/item_id"),
                "item_id must be non-empty",
                "reference a declared item id",
            ));
        }
        if !self.weight.is_finite() || self.weight < 0.0 {
            return Err(SchemaError::validation(
                format!("{pointer_base}/weight"),
                "weight must be finite and non-negative",
                "use e.g. 1.0",
            ));
        }
        if self.min_count > self.max_count {
            return Err(SchemaError::validation(
                format!("{pointer_base}/min_count"),
                "min_count must be <= max_count",
                "swap or align the bounds",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_requires_objective() {
        let q = Quest {
            id: "q".into(),
            display_name: "Quest".into(),
            description: None,
            objectives: vec![],
            rewards: QuestRewards::default(),
            required_level: None,
            tags: vec![],
        };
        let err = q.validate("/quests/0").unwrap_err();
        assert_eq!(err.pointer(), "/quests/0/objectives");
    }

    #[test]
    fn objective_kill_count_must_be_positive() {
        let o = QuestObjective {
            id: "o".into(),
            kind: ObjectiveKind::Kill {
                target_prop: "wolf".into(),
                count: 0,
            },
            parallel: false,
        };
        let err = o.validate("/q/objectives/0").unwrap_err();
        assert_eq!(err.pointer(), "/q/objectives/0/kind/count");
    }

    #[test]
    fn loot_table_weights_must_be_finite() {
        let t = LootTable {
            id: "t".into(),
            entries: vec![LootEntry {
                item_id: "gold".into(),
                weight: f32::NAN,
                min_count: 1,
                max_count: 1,
            }],
            rolls: 1,
        };
        let err = t.validate("/loot/0").unwrap_err();
        assert_eq!(err.pointer(), "/loot/0/entries/0/weight");
    }

    #[test]
    fn loot_entry_min_max_order() {
        let e = LootEntry {
            item_id: "gold".into(),
            weight: 1.0,
            min_count: 5,
            max_count: 3,
        };
        let err = e.validate("/e").unwrap_err();
        assert_eq!(err.pointer(), "/e/min_count");
    }

    #[test]
    fn quest_roundtrip() {
        let q = Quest {
            id: "q".into(),
            display_name: "Q".into(),
            description: Some("desc".into()),
            objectives: vec![QuestObjective {
                id: "o1".into(),
                kind: ObjectiveKind::Collect {
                    item_id: "herb".into(),
                    count: 3,
                },
                parallel: false,
            }],
            rewards: QuestRewards {
                experience: 100,
                ..Default::default()
            },
            required_level: Some(5),
            tags: vec![],
        };
        q.validate("/q").unwrap();
        let y = serde_yaml::to_string(&q).unwrap();
        let back: Quest = serde_yaml::from_str(&y).unwrap();
        assert_eq!(q, back);
    }
}
