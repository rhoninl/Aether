//! Authority tracking and atomic transfer for zone-owned entities.

use std::collections::HashMap;

pub type AuthorityZoneId = String;

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

    pub fn get(&self, entity_id: u64) -> Option<&NetworkIdentity> {
        self.entries.iter().find(|e| e.entity_id == entity_id)
    }

    pub fn get_authority_zone(&self, entity_id: u64) -> Option<&str> {
        self.get(entity_id).map(|e| e.authority_zone.as_str())
    }
}

impl Default for AuthorityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Authority transfer with prepare / commit / rollback
// ---------------------------------------------------------------------------

/// State of a pending authority transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferState {
    /// Transfer prepared -- writes are frozen on the entity.
    Prepared,
    /// Transfer committed -- authority moved to target zone.
    Committed,
    /// Transfer rolled back -- authority returned to source zone.
    RolledBack,
}

/// A pending authority transfer record.
#[derive(Debug, Clone)]
pub struct PendingTransfer {
    pub entity_id: u64,
    pub from_zone: AuthorityZoneId,
    pub to_zone: AuthorityZoneId,
    pub state: TransferState,
    pub prepared_ms: u64,
}

/// Result of an authority transfer operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferResult {
    /// Transfer was prepared successfully.
    Prepared { entity_id: u64 },
    /// Transfer was committed successfully.
    Committed { entity_id: u64, new_zone: String },
    /// Transfer was rolled back successfully.
    RolledBack { entity_id: u64, restored_zone: String },
    /// Entity is not registered.
    EntityNotFound { entity_id: u64 },
    /// Entity is already in a pending transfer.
    AlreadyPending { entity_id: u64 },
    /// No pending transfer exists for this entity.
    NoPendingTransfer { entity_id: u64 },
    /// Authority mismatch -- entity is not owned by the claimed source zone.
    AuthorityMismatch {
        entity_id: u64,
        expected: String,
        actual: String,
    },
}

/// Manages atomic authority transfers with prepare/commit/rollback semantics.
#[derive(Debug)]
pub struct AuthorityTransferManager {
    registry: AuthorityRegistry,
    pending: HashMap<u64, PendingTransfer>,
}

impl AuthorityTransferManager {
    pub fn new() -> Self {
        Self {
            registry: AuthorityRegistry::new(),
            pending: HashMap::new(),
        }
    }

    pub fn registry(&self) -> &AuthorityRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut AuthorityRegistry {
        &mut self.registry
    }

    /// Register an entity with initial authority.
    pub fn register(&mut self, entity_id: u64, zone: &str) {
        let identity = NetworkIdentity::new(entity_id, zone);
        self.registry.upsert(identity);
    }

    /// Prepare an authority transfer. Marks entity as pending (frozen for writes).
    pub fn prepare_transfer(
        &mut self,
        entity_id: u64,
        from_zone: &str,
        to_zone: &str,
        now_ms: u64,
    ) -> TransferResult {
        // Check entity exists
        let identity = match self.registry.get(entity_id) {
            Some(id) => id,
            None => return TransferResult::EntityNotFound { entity_id },
        };

        // Check authority matches
        if identity.authority_zone != from_zone {
            return TransferResult::AuthorityMismatch {
                entity_id,
                expected: from_zone.to_string(),
                actual: identity.authority_zone.clone(),
            };
        }

        // Check not already pending
        if self.pending.contains_key(&entity_id) {
            return TransferResult::AlreadyPending { entity_id };
        }

        // Mark as pending in registry
        let mut updated = identity.clone();
        updated.pending_transition = true;
        self.registry.upsert(updated);

        // Record pending transfer
        self.pending.insert(
            entity_id,
            PendingTransfer {
                entity_id,
                from_zone: from_zone.to_string(),
                to_zone: to_zone.to_string(),
                state: TransferState::Prepared,
                prepared_ms: now_ms,
            },
        );

        TransferResult::Prepared { entity_id }
    }

    /// Commit the authority transfer -- move authority to the target zone.
    pub fn commit_transfer(&mut self, entity_id: u64) -> TransferResult {
        let transfer = match self.pending.remove(&entity_id) {
            Some(t) if t.state == TransferState::Prepared => t,
            Some(_) => return TransferResult::NoPendingTransfer { entity_id },
            None => return TransferResult::NoPendingTransfer { entity_id },
        };

        // Update registry to new zone
        let identity = NetworkIdentity::new(entity_id, &transfer.to_zone);
        self.registry.upsert(identity);

        TransferResult::Committed {
            entity_id,
            new_zone: transfer.to_zone,
        }
    }

    /// Rollback the authority transfer -- restore original authority.
    pub fn rollback_transfer(&mut self, entity_id: u64) -> TransferResult {
        let transfer = match self.pending.remove(&entity_id) {
            Some(t) if t.state == TransferState::Prepared => t,
            Some(_) => return TransferResult::NoPendingTransfer { entity_id },
            None => return TransferResult::NoPendingTransfer { entity_id },
        };

        // Restore original authority and clear pending flag
        let identity = NetworkIdentity::new(entity_id, &transfer.from_zone);
        self.registry.upsert(identity);

        TransferResult::RolledBack {
            entity_id,
            restored_zone: transfer.from_zone,
        }
    }

    /// Check if an entity has a pending transfer.
    pub fn is_pending(&self, entity_id: u64) -> bool {
        self.pending.contains_key(&entity_id)
    }

    /// Number of pending transfers.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

impl Default for AuthorityTransferManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- AuthorityRegistry tests ---

    #[test]
    fn registry_upsert_and_lookup() {
        let mut reg = AuthorityRegistry::new();
        let id = NetworkIdentity::new(1, "zone-a");
        reg.upsert(id);

        assert!(reg.is_authority(1, "zone-a"));
        assert!(!reg.is_authority(1, "zone-b"));
        assert!(!reg.is_authority(2, "zone-a"));
    }

    #[test]
    fn registry_upsert_updates_existing() {
        let mut reg = AuthorityRegistry::new();
        reg.upsert(NetworkIdentity::new(1, "zone-a"));
        reg.upsert(NetworkIdentity::new(1, "zone-b"));

        assert!(!reg.is_authority(1, "zone-a"));
        assert!(reg.is_authority(1, "zone-b"));
    }

    #[test]
    fn registry_sequence_monotonic() {
        let mut reg = AuthorityRegistry::new();
        let id1 = reg.upsert(NetworkIdentity::new(1, "z"));
        let id2 = reg.upsert(NetworkIdentity::new(2, "z"));
        assert!(id2.sequence > id1.sequence);
    }

    // --- AuthorityTransferManager tests ---

    #[test]
    fn prepare_commit_lifecycle() {
        let mut mgr = AuthorityTransferManager::new();
        mgr.register(1, "zone-a");

        // Prepare
        let result = mgr.prepare_transfer(1, "zone-a", "zone-b", 100);
        assert_eq!(result, TransferResult::Prepared { entity_id: 1 });
        assert!(mgr.is_pending(1));

        // Entity should be marked pending_transition
        let identity = mgr.registry().get(1).unwrap();
        assert!(identity.pending_transition);

        // Commit
        let result = mgr.commit_transfer(1);
        assert_eq!(
            result,
            TransferResult::Committed {
                entity_id: 1,
                new_zone: "zone-b".to_string()
            }
        );
        assert!(!mgr.is_pending(1));
        assert!(mgr.registry().is_authority(1, "zone-b"));
        assert!(!mgr.registry().is_authority(1, "zone-a"));
    }

    #[test]
    fn prepare_rollback_lifecycle() {
        let mut mgr = AuthorityTransferManager::new();
        mgr.register(1, "zone-a");

        let result = mgr.prepare_transfer(1, "zone-a", "zone-b", 100);
        assert_eq!(result, TransferResult::Prepared { entity_id: 1 });

        let result = mgr.rollback_transfer(1);
        assert_eq!(
            result,
            TransferResult::RolledBack {
                entity_id: 1,
                restored_zone: "zone-a".to_string()
            }
        );
        assert!(!mgr.is_pending(1));
        assert!(mgr.registry().is_authority(1, "zone-a"));
    }

    #[test]
    fn prepare_entity_not_found() {
        let mut mgr = AuthorityTransferManager::new();
        let result = mgr.prepare_transfer(999, "zone-a", "zone-b", 100);
        assert_eq!(result, TransferResult::EntityNotFound { entity_id: 999 });
    }

    #[test]
    fn prepare_authority_mismatch() {
        let mut mgr = AuthorityTransferManager::new();
        mgr.register(1, "zone-a");

        let result = mgr.prepare_transfer(1, "zone-b", "zone-c", 100);
        assert_eq!(
            result,
            TransferResult::AuthorityMismatch {
                entity_id: 1,
                expected: "zone-b".to_string(),
                actual: "zone-a".to_string(),
            }
        );
    }

    #[test]
    fn prepare_already_pending() {
        let mut mgr = AuthorityTransferManager::new();
        mgr.register(1, "zone-a");
        mgr.prepare_transfer(1, "zone-a", "zone-b", 100);

        let result = mgr.prepare_transfer(1, "zone-a", "zone-c", 200);
        assert_eq!(result, TransferResult::AlreadyPending { entity_id: 1 });
    }

    #[test]
    fn commit_no_pending() {
        let mut mgr = AuthorityTransferManager::new();
        let result = mgr.commit_transfer(999);
        assert_eq!(result, TransferResult::NoPendingTransfer { entity_id: 999 });
    }

    #[test]
    fn rollback_no_pending() {
        let mut mgr = AuthorityTransferManager::new();
        let result = mgr.rollback_transfer(999);
        assert_eq!(result, TransferResult::NoPendingTransfer { entity_id: 999 });
    }

    #[test]
    fn multiple_entities_independent() {
        let mut mgr = AuthorityTransferManager::new();
        mgr.register(1, "zone-a");
        mgr.register(2, "zone-a");

        mgr.prepare_transfer(1, "zone-a", "zone-b", 100);
        assert!(mgr.is_pending(1));
        assert!(!mgr.is_pending(2));
        assert_eq!(mgr.pending_count(), 1);

        mgr.prepare_transfer(2, "zone-a", "zone-c", 200);
        assert_eq!(mgr.pending_count(), 2);

        mgr.commit_transfer(1);
        mgr.rollback_transfer(2);

        assert!(mgr.registry().is_authority(1, "zone-b"));
        assert!(mgr.registry().is_authority(2, "zone-a"));
        assert_eq!(mgr.pending_count(), 0);
    }

    #[test]
    fn get_authority_zone() {
        let mut mgr = AuthorityTransferManager::new();
        mgr.register(1, "zone-a");

        assert_eq!(mgr.registry().get_authority_zone(1), Some("zone-a"));
        assert_eq!(mgr.registry().get_authority_zone(999), None);
    }
}
