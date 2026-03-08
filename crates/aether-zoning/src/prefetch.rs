//! Asset prefetch hints and priority queue for portal transitions.
//!
//! When a player approaches a portal, the system emits prefetch hints
//! describing assets that should be downloaded before the transition.
//! The queue deduplicates and orders hints by priority.

use std::collections::HashMap;

/// Default maximum number of hints in the prefetch queue.
const DEFAULT_PREFETCH_QUEUE_CAPACITY: usize = 256;

/// Priority level for prefetch hints (lower number = higher priority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrefetchPriority {
    /// Must be loaded before transition can complete.
    Critical = 0,
    /// Important for initial view at destination.
    High = 1,
    /// Nice to have but not blocking.
    Medium = 2,
    /// Can be loaded lazily after arrival.
    Low = 3,
}

/// The type of asset being prefetched.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetKind {
    /// World geometry / terrain mesh.
    Geometry,
    /// Texture / material.
    Texture,
    /// Audio clip.
    Audio,
    /// Script / WASM module.
    Script,
    /// Avatar model or animation.
    Avatar,
    /// Generic / unknown type.
    Other(String),
}

/// A hint describing an asset that should be prefetched.
#[derive(Debug, Clone)]
pub struct PrefetchHint {
    /// URL or identifier of the asset.
    pub asset_url: String,
    /// Priority level.
    pub priority: PrefetchPriority,
    /// Estimated size in bytes (0 if unknown).
    pub estimated_size_bytes: u64,
    /// Type of asset.
    pub kind: AssetKind,
    /// Portal ID that generated this hint.
    pub source_portal_id: u64,
}

/// A priority queue for asset prefetch hints with deduplication.
#[derive(Debug)]
pub struct PrefetchQueue {
    /// Deduplicated hints keyed by asset URL.
    hints: HashMap<String, PrefetchHint>,
    /// Maximum capacity.
    capacity: usize,
}

impl PrefetchQueue {
    /// Create a new prefetch queue with default capacity.
    pub fn new() -> Self {
        Self {
            hints: HashMap::new(),
            capacity: DEFAULT_PREFETCH_QUEUE_CAPACITY,
        }
    }

    /// Create a queue with a custom capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            hints: HashMap::new(),
            capacity,
        }
    }

    /// Add a prefetch hint. If a hint for the same asset URL already exists,
    /// the higher-priority one is kept. Returns true if the hint was added or updated.
    pub fn add(&mut self, hint: PrefetchHint) -> bool {
        if let Some(existing) = self.hints.get(&hint.asset_url) {
            if hint.priority < existing.priority {
                self.hints.insert(hint.asset_url.clone(), hint);
                return true;
            }
            return false;
        }

        if self.hints.len() >= self.capacity {
            // Queue is full; only add if higher priority than the lowest-priority item
            if let Some(lowest_key) = self.find_lowest_priority_key() {
                let lowest_priority = self.hints[&lowest_key].priority;
                if hint.priority < lowest_priority {
                    self.hints.remove(&lowest_key);
                    self.hints.insert(hint.asset_url.clone(), hint);
                    return true;
                }
            }
            return false;
        }

        self.hints.insert(hint.asset_url.clone(), hint);
        true
    }

    /// Drain all hints sorted by priority (highest priority first), then by asset URL.
    pub fn drain_sorted(&mut self) -> Vec<PrefetchHint> {
        let mut hints: Vec<PrefetchHint> = self.hints.drain().map(|(_, h)| h).collect();
        hints.sort_by(|a, b| {
            a.priority.cmp(&b.priority).then_with(|| a.asset_url.cmp(&b.asset_url))
        });
        hints
    }

    /// Get all hints sorted by priority without removing them.
    pub fn peek_sorted(&self) -> Vec<&PrefetchHint> {
        let mut hints: Vec<&PrefetchHint> = self.hints.values().collect();
        hints.sort_by(|a, b| {
            a.priority.cmp(&b.priority).then_with(|| a.asset_url.cmp(&b.asset_url))
        });
        hints
    }

    /// Number of hints in the queue.
    pub fn len(&self) -> usize {
        self.hints.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.hints.is_empty()
    }

    /// Remove all hints for a specific portal.
    pub fn remove_for_portal(&mut self, portal_id: u64) -> usize {
        let before = self.hints.len();
        self.hints.retain(|_, h| h.source_portal_id != portal_id);
        before - self.hints.len()
    }

    /// Clear all hints.
    pub fn clear(&mut self) {
        self.hints.clear();
    }

    /// Total estimated bytes of all hints.
    pub fn total_estimated_bytes(&self) -> u64 {
        self.hints.values().map(|h| h.estimated_size_bytes).sum()
    }

    /// Get a hint by asset URL.
    pub fn get(&self, asset_url: &str) -> Option<&PrefetchHint> {
        self.hints.get(asset_url)
    }

    /// Find the key of the lowest-priority (highest numeric value) hint.
    fn find_lowest_priority_key(&self) -> Option<String> {
        self.hints
            .iter()
            .max_by_key(|(_, h)| h.priority)
            .map(|(k, _)| k.clone())
    }
}

impl Default for PrefetchQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hint(url: &str, priority: PrefetchPriority) -> PrefetchHint {
        PrefetchHint {
            asset_url: url.to_string(),
            priority,
            estimated_size_bytes: 1024,
            kind: AssetKind::Texture,
            source_portal_id: 1,
        }
    }

    // --- Basic add and retrieve ---

    #[test]
    fn add_and_len() {
        let mut queue = PrefetchQueue::new();
        assert!(queue.is_empty());

        assert!(queue.add(make_hint("a.png", PrefetchPriority::High)));
        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());
    }

    // --- Deduplication ---

    #[test]
    fn duplicate_url_not_added() {
        let mut queue = PrefetchQueue::new();
        queue.add(make_hint("a.png", PrefetchPriority::High));
        let added = queue.add(make_hint("a.png", PrefetchPriority::High));
        assert!(!added);
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn duplicate_url_upgraded_priority() {
        let mut queue = PrefetchQueue::new();
        queue.add(make_hint("a.png", PrefetchPriority::Low));
        let added = queue.add(make_hint("a.png", PrefetchPriority::Critical));
        assert!(added);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.get("a.png").unwrap().priority, PrefetchPriority::Critical);
    }

    #[test]
    fn duplicate_url_not_downgraded() {
        let mut queue = PrefetchQueue::new();
        queue.add(make_hint("a.png", PrefetchPriority::Critical));
        let added = queue.add(make_hint("a.png", PrefetchPriority::Low));
        assert!(!added);
        assert_eq!(queue.get("a.png").unwrap().priority, PrefetchPriority::Critical);
    }

    // --- Priority ordering ---

    #[test]
    fn drain_sorted_by_priority() {
        let mut queue = PrefetchQueue::new();
        queue.add(make_hint("c.wav", PrefetchPriority::Low));
        queue.add(make_hint("a.png", PrefetchPriority::Critical));
        queue.add(make_hint("b.obj", PrefetchPriority::High));

        let hints = queue.drain_sorted();
        assert_eq!(hints.len(), 3);
        assert_eq!(hints[0].priority, PrefetchPriority::Critical);
        assert_eq!(hints[1].priority, PrefetchPriority::High);
        assert_eq!(hints[2].priority, PrefetchPriority::Low);
        assert!(queue.is_empty());
    }

    #[test]
    fn drain_sorted_stable_by_url_within_priority() {
        let mut queue = PrefetchQueue::new();
        queue.add(make_hint("c.png", PrefetchPriority::High));
        queue.add(make_hint("a.png", PrefetchPriority::High));
        queue.add(make_hint("b.png", PrefetchPriority::High));

        let hints = queue.drain_sorted();
        assert_eq!(hints[0].asset_url, "a.png");
        assert_eq!(hints[1].asset_url, "b.png");
        assert_eq!(hints[2].asset_url, "c.png");
    }

    // --- Peek ---

    #[test]
    fn peek_does_not_remove() {
        let mut queue = PrefetchQueue::new();
        queue.add(make_hint("a.png", PrefetchPriority::High));

        let peeked = queue.peek_sorted();
        assert_eq!(peeked.len(), 1);
        assert_eq!(queue.len(), 1);
    }

    // --- Capacity ---

    #[test]
    fn capacity_limit_enforced() {
        let mut queue = PrefetchQueue::with_capacity(2);
        queue.add(make_hint("a.png", PrefetchPriority::Medium));
        queue.add(make_hint("b.png", PrefetchPriority::Medium));

        // Queue full, same priority -> not added
        let added = queue.add(make_hint("c.png", PrefetchPriority::Medium));
        assert!(!added);
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn capacity_evicts_lowest_priority() {
        let mut queue = PrefetchQueue::with_capacity(2);
        queue.add(make_hint("a.png", PrefetchPriority::Low));
        queue.add(make_hint("b.png", PrefetchPriority::Medium));

        // Higher priority should evict the lowest
        let added = queue.add(make_hint("c.png", PrefetchPriority::Critical));
        assert!(added);
        assert_eq!(queue.len(), 2);
        // The Low priority one should be evicted
        assert!(queue.get("a.png").is_none());
        assert!(queue.get("c.png").is_some());
    }

    // --- Remove for portal ---

    #[test]
    fn remove_for_portal() {
        let mut queue = PrefetchQueue::new();
        queue.add(PrefetchHint {
            asset_url: "a.png".to_string(),
            priority: PrefetchPriority::High,
            estimated_size_bytes: 100,
            kind: AssetKind::Texture,
            source_portal_id: 1,
        });
        queue.add(PrefetchHint {
            asset_url: "b.png".to_string(),
            priority: PrefetchPriority::High,
            estimated_size_bytes: 200,
            kind: AssetKind::Texture,
            source_portal_id: 2,
        });
        queue.add(PrefetchHint {
            asset_url: "c.png".to_string(),
            priority: PrefetchPriority::High,
            estimated_size_bytes: 300,
            kind: AssetKind::Texture,
            source_portal_id: 1,
        });

        let removed = queue.remove_for_portal(1);
        assert_eq!(removed, 2);
        assert_eq!(queue.len(), 1);
        assert!(queue.get("b.png").is_some());
    }

    // --- Clear ---

    #[test]
    fn clear_empties_queue() {
        let mut queue = PrefetchQueue::new();
        queue.add(make_hint("a.png", PrefetchPriority::High));
        queue.add(make_hint("b.png", PrefetchPriority::Low));

        queue.clear();
        assert!(queue.is_empty());
    }

    // --- Total estimated bytes ---

    #[test]
    fn total_estimated_bytes() {
        let mut queue = PrefetchQueue::new();
        queue.add(PrefetchHint {
            asset_url: "a.png".to_string(),
            priority: PrefetchPriority::High,
            estimated_size_bytes: 100,
            kind: AssetKind::Texture,
            source_portal_id: 1,
        });
        queue.add(PrefetchHint {
            asset_url: "b.obj".to_string(),
            priority: PrefetchPriority::Medium,
            estimated_size_bytes: 500,
            kind: AssetKind::Geometry,
            source_portal_id: 1,
        });

        assert_eq!(queue.total_estimated_bytes(), 600);
    }

    #[test]
    fn total_estimated_bytes_empty() {
        let queue = PrefetchQueue::new();
        assert_eq!(queue.total_estimated_bytes(), 0);
    }

    // --- Get ---

    #[test]
    fn get_existing() {
        let mut queue = PrefetchQueue::new();
        queue.add(make_hint("a.png", PrefetchPriority::High));
        assert!(queue.get("a.png").is_some());
    }

    #[test]
    fn get_nonexistent() {
        let queue = PrefetchQueue::new();
        assert!(queue.get("missing.png").is_none());
    }

    // --- Priority ordering enum ---

    #[test]
    fn priority_ordering() {
        assert!(PrefetchPriority::Critical < PrefetchPriority::High);
        assert!(PrefetchPriority::High < PrefetchPriority::Medium);
        assert!(PrefetchPriority::Medium < PrefetchPriority::Low);
    }

    // --- AssetKind ---

    #[test]
    fn asset_kind_equality() {
        assert_eq!(AssetKind::Texture, AssetKind::Texture);
        assert_ne!(AssetKind::Texture, AssetKind::Audio);
        assert_eq!(
            AssetKind::Other("custom".to_string()),
            AssetKind::Other("custom".to_string())
        );
        assert_ne!(
            AssetKind::Other("a".to_string()),
            AssetKind::Other("b".to_string())
        );
    }

    // --- Default constants ---

    #[test]
    fn default_constants() {
        assert_eq!(DEFAULT_PREFETCH_QUEUE_CAPACITY, 256);
    }

    // --- Empty drain ---

    #[test]
    fn drain_empty_queue() {
        let mut queue = PrefetchQueue::new();
        let hints = queue.drain_sorted();
        assert!(hints.is_empty());
    }

    // --- Remove for nonexistent portal ---

    #[test]
    fn remove_for_nonexistent_portal() {
        let mut queue = PrefetchQueue::new();
        queue.add(make_hint("a.png", PrefetchPriority::High));
        let removed = queue.remove_for_portal(999);
        assert_eq!(removed, 0);
        assert_eq!(queue.len(), 1);
    }
}
