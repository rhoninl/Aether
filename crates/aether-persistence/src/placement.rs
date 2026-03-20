use crate::config::WorldPersistenceClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PodRuntimeClass {
    StatefulSet,
    Deployment,
}

#[derive(Debug, Clone)]
pub struct PodTopologyHint {
    pub namespace: String,
    pub pod_class: PodRuntimeClass,
    pub zone_count: u8,
    pub anti_affinity_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct PodPlacementHint {
    pub world_id: String,
    pub pod_class: PodRuntimeClass,
    pub storage_gi: u32,
    pub needs_fast_local_disk: bool,
    pub max_players: u32,
}

#[derive(Debug, Clone)]
pub struct WorldManifest {
    pub world_id: String,
    pub world_name: String,
    pub durability_class: WorldPersistenceClass,
    pub p2p_enabled: bool,
    pub expected_players: u32,
    pub economy_enabled: bool,
}

impl WorldManifest {
    pub fn classify(&self) -> PodRuntimeClass {
        if matches!(self.durability_class, WorldPersistenceClass::Stateful) || self.economy_enabled
        {
            PodRuntimeClass::StatefulSet
        } else {
            PodRuntimeClass::Deployment
        }
    }

    pub fn make_placement_hint(&self) -> PodPlacementHint {
        let players = self.expected_players.max(1);
        let storage_gi = if matches!(self.classify(), PodRuntimeClass::StatefulSet) {
            20 + (players / 20) * 5
        } else {
            0
        };
        PodPlacementHint {
            world_id: self.world_id.clone(),
            pod_class: self.classify(),
            storage_gi,
            needs_fast_local_disk: matches!(self.classify(), PodRuntimeClass::StatefulSet),
            max_players: players,
        }
    }
}
