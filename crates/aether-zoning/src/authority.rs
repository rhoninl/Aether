#[derive(Debug, Clone)]
pub struct NetworkIdentity {
    pub entity_id: u64,
    pub authority_zone: String,
    pub sequence: u64,
    pub pending_transition: bool,
}

#[derive(Debug, Clone)]
pub enum SingleWriterMode {
    ZoneOwned { zone_id: String },
    FallbackMaster,
}

pub type AuthorityZoneId = String;

impl NetworkIdentity {
    pub fn new(entity_id: u64, authority_zone: impl Into<String>) -> Self {
        Self {
            entity_id,
            authority_zone: authority_zone.into(),
            sequence: 0,
            pending_transition: false,
        }
    }

    pub fn with_mode(&self, mode: SingleWriterMode) -> bool {
        match mode {
            SingleWriterMode::ZoneOwned { .. } => self.pending_transition,
            SingleWriterMode::FallbackMaster => true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthorityTransition {
    pub entity_id: u64,
    pub from_zone: AuthorityZoneId,
    pub to_zone: AuthorityZoneId,
    pub sequence: u64,
    pub requested_ms: u64,
}

#[derive(Debug)]
pub struct AuthorityRegistry {
    entries: Vec<NetworkIdentity>,
    next_sequence: u64,
}

impl AuthorityRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_sequence: 0,
        }
    }

    pub fn upsert(&mut self, mut identity: NetworkIdentity) -> NetworkIdentity {
        self.next_sequence = self.next_sequence.saturating_add(1);
        identity.sequence = self.next_sequence;
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|e| e.entity_id == identity.entity_id)
        {
            *existing = identity.clone();
            return identity;
        }
        self.entries.push(identity.clone());
        identity
    }

    pub fn is_authority(&self, entity_id: u64, zone_id: &str) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.entity_id == entity_id && entry.authority_zone == zone_id)
    }
}

