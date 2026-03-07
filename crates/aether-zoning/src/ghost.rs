use crate::authority::NetworkIdentity;

#[derive(Debug, Clone)]
pub struct GhostEntity {
    pub source_entity: u64,
    pub local_entity: u64,
    pub source_zone: String,
    pub remote_zone: String,
    pub ttl_ms: u64,
    pub collision_enabled: bool,
    pub render_only: bool,
}

#[derive(Debug, Clone)]
pub enum GhostVisibilityScope {
    Always,
    DistanceCapped { max_distance_m: f32 },
}

#[derive(Debug)]
pub struct GhostPolicy {
    pub ttl_ms: u64,
    pub max_ghosts_per_connection: usize,
    pub visibility: GhostVisibilityScope,
}

#[derive(Debug, Default)]
pub struct GhostCache {
    ghosts: Vec<GhostEntity>,
}

impl GhostCache {
    pub fn new(policy: GhostPolicy) -> Self {
        let _ = policy.ttl_ms;
        let _ = policy.max_ghosts_per_connection;
        Self::default()
    }

    pub fn add(&mut self, ghost: GhostEntity) {
        self.ghosts.push(ghost);
    }

    pub fn remove_by_local_entity(&mut self, local_entity: u64) -> Option<GhostEntity> {
        let idx = self.ghosts.iter().position(|entry| entry.local_entity == local_entity)?;
        Some(self.ghosts.swap_remove(idx))
    }

    pub fn cull_expired(&mut self, now_ms: u64) {
        self.ghosts.retain(|ghost| ghost.ttl_ms > now_ms.saturating_sub(ghost.ttl_ms));
    }

    pub fn as_identities(&self) -> Vec<NetworkIdentity> {
        self.ghosts
            .iter()
            .map(|ghost| NetworkIdentity::new(ghost.local_entity, ghost.remote_zone.clone()))
            .collect()
    }
}

