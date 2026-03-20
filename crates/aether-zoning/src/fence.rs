//! Sequence fence tracker for ordering guarantees during cross-zone handoff.
//!
//! Ensures messages between zone pairs are processed in order, with gap
//! detection and buffering for out-of-order arrivals.

use std::collections::{BTreeMap, HashMap};

/// Maximum number of pending out-of-order messages before rejecting new ones.
const DEFAULT_MAX_GAP: u64 = 64;

/// A buffered message waiting for earlier sequences to arrive.
#[derive(Debug, Clone)]
pub struct PendingMessage {
    pub sequence: u64,
    pub payload: Vec<u8>,
    pub received_ms: u64,
}

/// Tracks per-zone-pair sequence fences and buffers out-of-order messages.
#[derive(Debug)]
pub struct SequenceFenceTracker {
    /// (source_zone, target_zone) -> last delivered sequence number
    fences: HashMap<(String, String), u64>,
    /// (source_zone, target_zone) -> sequence -> pending message
    pending: HashMap<(String, String), BTreeMap<u64, PendingMessage>>,
    /// Maximum allowed gap between expected and received sequence
    max_gap: u64,
}

/// Result of submitting a sequenced message to the tracker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FenceResult {
    /// Message was in order and delivered, along with any buffered successors.
    Delivered { count: usize },
    /// Message is out of order but within gap tolerance; buffered for later.
    Buffered,
    /// Message sequence is too far ahead of the expected sequence.
    GapExceeded { expected: u64, received: u64 },
    /// Message is a duplicate (sequence already processed).
    Duplicate,
}

impl SequenceFenceTracker {
    pub fn new() -> Self {
        Self {
            fences: HashMap::new(),
            pending: HashMap::new(),
            max_gap: DEFAULT_MAX_GAP,
        }
    }

    pub fn with_max_gap(mut self, max_gap: u64) -> Self {
        self.max_gap = max_gap;
        self
    }

    /// Returns the next expected sequence for a zone pair.
    pub fn expected_sequence(&self, source: &str, target: &str) -> u64 {
        let key = (source.to_string(), target.to_string());
        self.fences.get(&key).copied().map(|s| s + 1).unwrap_or(1)
    }

    /// Submit a sequenced message. Returns the result and any deliverable payloads.
    pub fn submit(
        &mut self,
        source: &str,
        target: &str,
        sequence: u64,
        payload: Vec<u8>,
        now_ms: u64,
    ) -> (FenceResult, Vec<PendingMessage>) {
        let key = (source.to_string(), target.to_string());
        let last_delivered = self.fences.get(&key).copied().unwrap_or(0);
        let expected = last_delivered + 1;

        if sequence <= last_delivered {
            return (FenceResult::Duplicate, vec![]);
        }

        if sequence == expected {
            // In order -- deliver this message plus any buffered successors
            self.fences.insert(key.clone(), sequence);
            let mut delivered = vec![PendingMessage {
                sequence,
                payload,
                received_ms: now_ms,
            }];

            // Drain consecutive buffered messages
            if let Some(buffer) = self.pending.get_mut(&key) {
                let mut next = sequence + 1;
                while let Some(msg) = buffer.remove(&next) {
                    self.fences.insert(key.clone(), next);
                    delivered.push(msg);
                    next += 1;
                }
            }

            let count = delivered.len();
            (FenceResult::Delivered { count }, delivered)
        } else if sequence - expected > self.max_gap {
            (
                FenceResult::GapExceeded {
                    expected,
                    received: sequence,
                },
                vec![],
            )
        } else {
            // Out of order but within tolerance -- buffer it
            let buffer = self.pending.entry(key).or_default();
            buffer.insert(
                sequence,
                PendingMessage {
                    sequence,
                    payload,
                    received_ms: now_ms,
                },
            );
            (FenceResult::Buffered, vec![])
        }
    }

    /// Returns the number of pending (buffered) messages for a zone pair.
    pub fn pending_count(&self, source: &str, target: &str) -> usize {
        let key = (source.to_string(), target.to_string());
        self.pending.get(&key).map(|b| b.len()).unwrap_or(0)
    }

    /// Reset tracking for a zone pair (e.g., after zone restart).
    pub fn reset_pair(&mut self, source: &str, target: &str) {
        let key = (source.to_string(), target.to_string());
        self.fences.remove(&key);
        self.pending.remove(&key);
    }
}

impl Default for SequenceFenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_order_delivery() {
        let mut tracker = SequenceFenceTracker::new();
        let (result, msgs) = tracker.submit("zone-a", "zone-b", 1, vec![1], 100);
        assert_eq!(result, FenceResult::Delivered { count: 1 });
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].sequence, 1);

        let (result, msgs) = tracker.submit("zone-a", "zone-b", 2, vec![2], 200);
        assert_eq!(result, FenceResult::Delivered { count: 1 });
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn out_of_order_buffering() {
        let mut tracker = SequenceFenceTracker::new();

        // Seq 2 arrives before seq 1
        let (result, msgs) = tracker.submit("zone-a", "zone-b", 2, vec![2], 100);
        assert_eq!(result, FenceResult::Buffered);
        assert!(msgs.is_empty());
        assert_eq!(tracker.pending_count("zone-a", "zone-b"), 1);

        // Now seq 1 arrives -- both should deliver
        let (result, msgs) = tracker.submit("zone-a", "zone-b", 1, vec![1], 200);
        assert_eq!(result, FenceResult::Delivered { count: 2 });
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].sequence, 1);
        assert_eq!(msgs[1].sequence, 2);
        assert_eq!(tracker.pending_count("zone-a", "zone-b"), 0);
    }

    #[test]
    fn gap_exceeded() {
        let mut tracker = SequenceFenceTracker::new().with_max_gap(5);

        let (result, _) = tracker.submit("zone-a", "zone-b", 10, vec![], 100);
        assert!(matches!(
            result,
            FenceResult::GapExceeded {
                expected: 1,
                received: 10
            }
        ));
    }

    #[test]
    fn duplicate_detection() {
        let mut tracker = SequenceFenceTracker::new();
        tracker.submit("zone-a", "zone-b", 1, vec![1], 100);

        let (result, _) = tracker.submit("zone-a", "zone-b", 1, vec![1], 200);
        assert_eq!(result, FenceResult::Duplicate);
    }

    #[test]
    fn independent_zone_pairs() {
        let mut tracker = SequenceFenceTracker::new();

        let (r1, _) = tracker.submit("zone-a", "zone-b", 1, vec![], 100);
        let (r2, _) = tracker.submit("zone-c", "zone-d", 1, vec![], 100);

        assert_eq!(r1, FenceResult::Delivered { count: 1 });
        assert_eq!(r2, FenceResult::Delivered { count: 1 });

        assert_eq!(tracker.expected_sequence("zone-a", "zone-b"), 2);
        assert_eq!(tracker.expected_sequence("zone-c", "zone-d"), 2);
    }

    #[test]
    fn reset_pair_clears_state() {
        let mut tracker = SequenceFenceTracker::new();
        tracker.submit("zone-a", "zone-b", 1, vec![], 100);
        tracker.submit("zone-a", "zone-b", 3, vec![], 200); // buffered

        tracker.reset_pair("zone-a", "zone-b");

        assert_eq!(tracker.expected_sequence("zone-a", "zone-b"), 1);
        assert_eq!(tracker.pending_count("zone-a", "zone-b"), 0);
    }

    #[test]
    fn consecutive_buffered_drain() {
        let mut tracker = SequenceFenceTracker::new();

        // Submit 5, 4, 3, then 2, then 1 -- all should eventually deliver
        tracker.submit("a", "b", 5, vec![5], 100);
        tracker.submit("a", "b", 4, vec![4], 200);
        tracker.submit("a", "b", 3, vec![3], 300);
        tracker.submit("a", "b", 2, vec![2], 400);

        assert_eq!(tracker.pending_count("a", "b"), 4);

        let (result, msgs) = tracker.submit("a", "b", 1, vec![1], 500);
        assert_eq!(result, FenceResult::Delivered { count: 5 });
        assert_eq!(msgs.len(), 5);
        for (i, msg) in msgs.iter().enumerate() {
            assert_eq!(msg.sequence, (i + 1) as u64);
        }
    }

    #[test]
    fn expected_sequence_starts_at_one() {
        let tracker = SequenceFenceTracker::new();
        assert_eq!(tracker.expected_sequence("any", "pair"), 1);
    }
}
