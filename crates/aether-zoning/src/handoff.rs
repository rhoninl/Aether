//! Cross-server entity handoff coordinator.
//!
//! Drives the state machine for migrating entities between zone processes,
//! including idempotency deduplication and timeout handling.

use std::collections::{HashMap, HashSet, VecDeque};

/// Default handoff timeout in milliseconds.
const DEFAULT_HANDOFF_TIMEOUT_MS: u64 = 8_000;
/// Maximum number of completed idempotency keys to retain.
const DEFAULT_IDEMPOTENCY_CAPACITY: usize = 4096;

/// Request to hand off an entity from one zone to another.
#[derive(Debug, Clone)]
pub struct HandoffRequest {
    pub entity_id: u64,
    pub source_zone: String,
    pub target_zone: String,
    pub position: [f32; 3],
    pub state_snapshot: Vec<u8>,
    pub idempotency_key: u128,
    pub sequence: u64,
}

/// State of an in-flight handoff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandoffPhase {
    /// Handoff has been initiated by the source zone.
    Initiated,
    /// Target zone is preparing to receive the entity.
    Preparing,
    /// Target zone acknowledged readiness.
    Acknowledged,
    /// Authority transfer is in progress.
    Transferring,
    /// Handoff completed successfully.
    Completed,
    /// Handoff failed with a reason.
    Failed(String),
}

/// Internal tracking of an active handoff.
#[derive(Debug, Clone)]
pub struct ActiveHandoff {
    pub request: HandoffRequest,
    pub phase: HandoffPhase,
    pub started_ms: u64,
    pub last_transition_ms: u64,
}

/// Outcome returned to callers after handoff resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandoffOutcome {
    /// Handoff completed successfully.
    Success {
        entity_id: u64,
        source_zone: String,
        target_zone: String,
        sequence: u64,
    },
    /// Handoff was rejected or failed.
    Failure {
        entity_id: u64,
        reason: String,
    },
    /// Duplicate request -- already processed.
    DuplicateIgnored {
        idempotency_key: u128,
    },
    /// Handoff is still in progress.
    InProgress {
        idempotency_key: u128,
        phase: HandoffPhase,
    },
}

/// Coordinates cross-zone entity handoffs.
#[derive(Debug)]
pub struct HandoffCoordinator {
    in_flight: HashMap<u128, ActiveHandoff>,
    completed_keys: HashSet<u128>,
    completed_order: VecDeque<u128>,
    next_sequence: u64,
    timeout_ms: u64,
    idempotency_capacity: usize,
}

impl HandoffCoordinator {
    pub fn new() -> Self {
        Self {
            in_flight: HashMap::new(),
            completed_keys: HashSet::new(),
            completed_order: VecDeque::new(),
            next_sequence: 1,
            timeout_ms: DEFAULT_HANDOFF_TIMEOUT_MS,
            idempotency_capacity: DEFAULT_IDEMPOTENCY_CAPACITY,
        }
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    pub fn with_idempotency_capacity(mut self, capacity: usize) -> Self {
        self.idempotency_capacity = capacity;
        self
    }

    /// Initiate a handoff. Returns `DuplicateIgnored` if the idempotency key was
    /// already processed, or `InProgress` with the `Initiated` phase.
    pub fn initiate(&mut self, request: HandoffRequest, now_ms: u64) -> HandoffOutcome {
        // Check idempotency
        if self.completed_keys.contains(&request.idempotency_key) {
            return HandoffOutcome::DuplicateIgnored {
                idempotency_key: request.idempotency_key,
            };
        }

        if let Some(existing) = self.in_flight.get(&request.idempotency_key) {
            return HandoffOutcome::InProgress {
                idempotency_key: request.idempotency_key,
                phase: existing.phase.clone(),
            };
        }

        let key = request.idempotency_key;
        let handoff = ActiveHandoff {
            request,
            phase: HandoffPhase::Initiated,
            started_ms: now_ms,
            last_transition_ms: now_ms,
        };
        self.in_flight.insert(key, handoff);

        HandoffOutcome::InProgress {
            idempotency_key: key,
            phase: HandoffPhase::Initiated,
        }
    }

    /// Advance a handoff to the Preparing phase (target zone starts receiving).
    pub fn mark_preparing(&mut self, idempotency_key: u128, now_ms: u64) -> Option<HandoffOutcome> {
        self.transition(idempotency_key, HandoffPhase::Preparing, now_ms)
    }

    /// Target zone acknowledged readiness.
    pub fn mark_acknowledged(&mut self, idempotency_key: u128, now_ms: u64) -> Option<HandoffOutcome> {
        self.transition(idempotency_key, HandoffPhase::Acknowledged, now_ms)
    }

    /// Begin authority transfer.
    pub fn mark_transferring(&mut self, idempotency_key: u128, now_ms: u64) -> Option<HandoffOutcome> {
        self.transition(idempotency_key, HandoffPhase::Transferring, now_ms)
    }

    /// Complete the handoff.
    pub fn mark_completed(&mut self, idempotency_key: u128, _now_ms: u64) -> Option<HandoffOutcome> {
        let handoff = self.in_flight.remove(&idempotency_key)?;
        self.record_completed(idempotency_key);

        Some(HandoffOutcome::Success {
            entity_id: handoff.request.entity_id,
            source_zone: handoff.request.source_zone,
            target_zone: handoff.request.target_zone,
            sequence: handoff.request.sequence,
        })
    }

    /// Fail the handoff with a reason.
    pub fn mark_failed(
        &mut self,
        idempotency_key: u128,
        reason: String,
        _now_ms: u64,
    ) -> Option<HandoffOutcome> {
        let handoff = self.in_flight.remove(&idempotency_key)?;
        self.record_completed(idempotency_key);

        Some(HandoffOutcome::Failure {
            entity_id: handoff.request.entity_id,
            reason,
        })
    }

    /// Check for timed-out handoffs and fail them. Returns outcomes for each timed-out handoff.
    pub fn tick_timeouts(&mut self, now_ms: u64) -> Vec<HandoffOutcome> {
        let timed_out: Vec<u128> = self
            .in_flight
            .iter()
            .filter(|(_, h)| now_ms.saturating_sub(h.started_ms) > self.timeout_ms)
            .map(|(key, _)| *key)
            .collect();

        let mut outcomes = Vec::new();
        for key in timed_out {
            if let Some(handoff) = self.in_flight.remove(&key) {
                self.record_completed(key);
                outcomes.push(HandoffOutcome::Failure {
                    entity_id: handoff.request.entity_id,
                    reason: "handoff timed out".to_string(),
                });
            }
        }
        outcomes
    }

    /// Number of currently in-flight handoffs.
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    /// Get the current phase of an in-flight handoff.
    pub fn get_phase(&self, idempotency_key: u128) -> Option<&HandoffPhase> {
        self.in_flight.get(&idempotency_key).map(|h| &h.phase)
    }

    /// Allocate the next sequence number for a handoff.
    pub fn next_sequence(&mut self) -> u64 {
        let seq = self.next_sequence;
        self.next_sequence += 1;
        seq
    }

    fn transition(
        &mut self,
        idempotency_key: u128,
        target_phase: HandoffPhase,
        now_ms: u64,
    ) -> Option<HandoffOutcome> {
        let handoff = self.in_flight.get_mut(&idempotency_key)?;

        // Validate transition order
        let valid = match (&handoff.phase, &target_phase) {
            (HandoffPhase::Initiated, HandoffPhase::Preparing) => true,
            (HandoffPhase::Preparing, HandoffPhase::Acknowledged) => true,
            (HandoffPhase::Acknowledged, HandoffPhase::Transferring) => true,
            _ => false,
        };

        if !valid {
            let reason = format!(
                "invalid transition from {:?} to {:?}",
                handoff.phase, target_phase
            );
            let entity_id = handoff.request.entity_id;
            self.in_flight.remove(&idempotency_key);
            self.record_completed(idempotency_key);
            return Some(HandoffOutcome::Failure {
                entity_id,
                reason,
            });
        }

        handoff.phase = target_phase.clone();
        handoff.last_transition_ms = now_ms;

        Some(HandoffOutcome::InProgress {
            idempotency_key,
            phase: target_phase,
        })
    }

    fn record_completed(&mut self, key: u128) {
        self.completed_keys.insert(key);
        self.completed_order.push_back(key);

        // Evict oldest if over capacity
        while self.completed_keys.len() > self.idempotency_capacity {
            if let Some(oldest) = self.completed_order.pop_front() {
                self.completed_keys.remove(&oldest);
            }
        }
    }
}

impl Default for HandoffCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(entity_id: u64, key: u128) -> HandoffRequest {
        HandoffRequest {
            entity_id,
            source_zone: "zone-a".to_string(),
            target_zone: "zone-b".to_string(),
            position: [10.0, 0.0, 20.0],
            state_snapshot: vec![1, 2, 3],
            idempotency_key: key,
            sequence: 1,
        }
    }

    #[test]
    fn full_handoff_lifecycle() {
        let mut coord = HandoffCoordinator::new();
        let req = make_request(42, 1000);

        // Initiate
        let result = coord.initiate(req, 0);
        assert!(matches!(
            result,
            HandoffOutcome::InProgress {
                phase: HandoffPhase::Initiated,
                ..
            }
        ));

        // Preparing
        let result = coord.mark_preparing(1000, 100).unwrap();
        assert!(matches!(
            result,
            HandoffOutcome::InProgress {
                phase: HandoffPhase::Preparing,
                ..
            }
        ));

        // Acknowledged
        let result = coord.mark_acknowledged(1000, 200).unwrap();
        assert!(matches!(
            result,
            HandoffOutcome::InProgress {
                phase: HandoffPhase::Acknowledged,
                ..
            }
        ));

        // Transferring
        let result = coord.mark_transferring(1000, 300).unwrap();
        assert!(matches!(
            result,
            HandoffOutcome::InProgress {
                phase: HandoffPhase::Transferring,
                ..
            }
        ));

        // Completed
        let result = coord.mark_completed(1000, 400).unwrap();
        assert!(matches!(result, HandoffOutcome::Success { entity_id: 42, .. }));

        assert_eq!(coord.in_flight_count(), 0);
    }

    #[test]
    fn idempotency_dedup_after_completion() {
        let mut coord = HandoffCoordinator::new();
        let req = make_request(42, 2000);
        coord.initiate(req.clone(), 0);
        coord.mark_preparing(2000, 50);
        coord.mark_acknowledged(2000, 100);
        coord.mark_transferring(2000, 150);
        coord.mark_completed(2000, 200);

        // Duplicate initiate should be ignored
        let result = coord.initiate(req, 300);
        assert!(matches!(
            result,
            HandoffOutcome::DuplicateIgnored {
                idempotency_key: 2000
            }
        ));
    }

    #[test]
    fn idempotency_dedup_while_in_flight() {
        let mut coord = HandoffCoordinator::new();
        let req = make_request(42, 3000);
        coord.initiate(req.clone(), 0);

        // Duplicate initiate while in-flight should return InProgress
        let result = coord.initiate(req, 100);
        assert!(matches!(
            result,
            HandoffOutcome::InProgress {
                idempotency_key: 3000,
                phase: HandoffPhase::Initiated,
            }
        ));
    }

    #[test]
    fn invalid_transition_fails() {
        let mut coord = HandoffCoordinator::new();
        let req = make_request(42, 4000);
        coord.initiate(req, 0);

        // Skip Preparing -> go straight to Acknowledged (invalid)
        let result = coord.mark_acknowledged(4000, 100).unwrap();
        assert!(matches!(result, HandoffOutcome::Failure { .. }));
        assert_eq!(coord.in_flight_count(), 0);
    }

    #[test]
    fn timeout_detection() {
        let mut coord = HandoffCoordinator::new().with_timeout(1000);
        let req = make_request(42, 5000);
        coord.initiate(req, 0);

        // Not yet timed out
        let outcomes = coord.tick_timeouts(500);
        assert!(outcomes.is_empty());
        assert_eq!(coord.in_flight_count(), 1);

        // Now timed out
        let outcomes = coord.tick_timeouts(1500);
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0], HandoffOutcome::Failure { entity_id: 42, .. }));
        assert_eq!(coord.in_flight_count(), 0);
    }

    #[test]
    fn mark_failed_removes_from_flight() {
        let mut coord = HandoffCoordinator::new();
        let req = make_request(42, 6000);
        coord.initiate(req, 0);
        coord.mark_preparing(6000, 100);

        let result = coord
            .mark_failed(6000, "zone unavailable".to_string(), 200)
            .unwrap();
        assert!(matches!(result, HandoffOutcome::Failure { entity_id: 42, .. }));
        assert_eq!(coord.in_flight_count(), 0);
    }

    #[test]
    fn nonexistent_key_returns_none() {
        let mut coord = HandoffCoordinator::new();
        assert!(coord.mark_preparing(9999, 0).is_none());
        assert!(coord.mark_acknowledged(9999, 0).is_none());
        assert!(coord.mark_transferring(9999, 0).is_none());
        assert!(coord.mark_completed(9999, 0).is_none());
        assert!(coord.mark_failed(9999, "x".to_string(), 0).is_none());
    }

    #[test]
    fn get_phase() {
        let mut coord = HandoffCoordinator::new();
        let req = make_request(42, 7000);
        coord.initiate(req, 0);

        assert_eq!(coord.get_phase(7000), Some(&HandoffPhase::Initiated));
        coord.mark_preparing(7000, 100);
        assert_eq!(coord.get_phase(7000), Some(&HandoffPhase::Preparing));

        assert_eq!(coord.get_phase(9999), None);
    }

    #[test]
    fn idempotency_capacity_eviction() {
        let mut coord = HandoffCoordinator::new().with_idempotency_capacity(3);

        // Complete 4 handoffs
        for i in 0..4u128 {
            let req = make_request(i as u64, i);
            coord.initiate(req, 0);
            coord.mark_preparing(i, 10);
            coord.mark_acknowledged(i, 20);
            coord.mark_transferring(i, 30);
            coord.mark_completed(i, 40);
        }

        // Key 0 should have been evicted (capacity=3, 1,2,3 remain)
        let req = make_request(0, 0);
        let result = coord.initiate(req, 50);
        assert!(matches!(
            result,
            HandoffOutcome::InProgress {
                phase: HandoffPhase::Initiated,
                ..
            }
        ));

        // Key 1 should still be deduped
        let req = make_request(1, 1);
        let result = coord.initiate(req, 60);
        assert!(matches!(result, HandoffOutcome::DuplicateIgnored { .. }));
    }

    #[test]
    fn next_sequence_monotonic() {
        let mut coord = HandoffCoordinator::new();
        assert_eq!(coord.next_sequence(), 1);
        assert_eq!(coord.next_sequence(), 2);
        assert_eq!(coord.next_sequence(), 3);
    }

    #[test]
    fn multiple_concurrent_handoffs() {
        let mut coord = HandoffCoordinator::new();
        let req_a = make_request(1, 100);
        let req_b = make_request(2, 200);

        coord.initiate(req_a, 0);
        coord.initiate(req_b, 0);
        assert_eq!(coord.in_flight_count(), 2);

        coord.mark_preparing(100, 10);
        coord.mark_acknowledged(100, 20);
        coord.mark_transferring(100, 30);
        coord.mark_completed(100, 40);
        assert_eq!(coord.in_flight_count(), 1);

        // Second handoff still in flight
        assert_eq!(coord.get_phase(200), Some(&HandoffPhase::Initiated));
    }
}
