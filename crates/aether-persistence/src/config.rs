use std::time::Duration;

/// Snapshot every 5 seconds for ephemeral world state unless policy overrides.
pub const DEFAULT_EPHEMERAL_SNAPSHOT_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldPersistenceClass {
    Stateful,
    Stateless,
}

#[derive(Debug, Clone)]
pub struct WorldPersistenceProfile {
    pub world_id: String,
    pub has_economy_state: bool,
    pub has_inventory: bool,
    pub has_identity: bool,
    pub has_durable_script_state: bool,
    pub expected_players: u32,
    pub snapshot_interval: Duration,
}

impl WorldPersistenceProfile {
    pub fn from_defaults(world_id: impl Into<String>) -> Self {
        Self {
            world_id: world_id.into(),
            has_economy_state: false,
            has_inventory: false,
            has_identity: false,
            has_durable_script_state: false,
            expected_players: 0,
            snapshot_interval: DEFAULT_EPHEMERAL_SNAPSHOT_INTERVAL,
        }
    }

    pub fn requires_stateful_storage(&self) -> bool {
        self.has_economy_state
            || self.has_inventory
            || self.has_identity
            || self.has_durable_script_state
    }

    pub fn classify_pod(&self) -> WorldPersistenceClass {
        if self.requires_stateful_storage() || self.expected_players >= 128 {
            WorldPersistenceClass::Stateful
        } else {
            WorldPersistenceClass::Stateless
        }
    }
}
