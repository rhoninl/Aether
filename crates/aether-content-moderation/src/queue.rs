/// Human review queue with priority ordering and approve/reject workflow.

use crate::severity::ContentSeverity;
use uuid::Uuid;

/// State of a review item in the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewState {
    /// Waiting for a moderator to claim.
    Pending,
    /// Claimed by a moderator.
    InReview,
    /// Approved by a moderator.
    Approved,
    /// Rejected by a moderator.
    Rejected,
}

/// Priority levels for review items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReviewPriority {
    Low,
    Medium,
    High,
    Urgent,
}

impl From<ContentSeverity> for ReviewPriority {
    fn from(severity: ContentSeverity) -> Self {
        match severity {
            ContentSeverity::Clean => ReviewPriority::Low,
            ContentSeverity::Low => ReviewPriority::Low,
            ContentSeverity::Medium => ReviewPriority::Medium,
            ContentSeverity::High => ReviewPriority::High,
            ContentSeverity::Critical => ReviewPriority::Urgent,
        }
    }
}

/// An item in the human review queue.
#[derive(Debug, Clone)]
pub struct ReviewItem {
    /// Unique item identifier.
    pub item_id: Uuid,
    /// The content being reviewed.
    pub content_id: String,
    /// Priority for queue ordering.
    pub priority: ReviewPriority,
    /// Current state.
    pub state: ReviewState,
    /// ID of the moderator who claimed this item (if any).
    pub assigned_moderator: Option<String>,
    /// Reason the content was flagged.
    pub flag_reason: String,
    /// Timestamp when item was submitted (ms since epoch).
    pub submitted_at_ms: u64,
    /// Timestamp when item was claimed (ms since epoch).
    pub claimed_at_ms: Option<u64>,
    /// Timestamp when decision was made (ms since epoch).
    pub decided_at_ms: Option<u64>,
    /// Decision reason (if decided).
    pub decision_reason: Option<String>,
}

/// Actions a moderator can take on a review item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewAction {
    /// Approve the content.
    Approve,
    /// Reject the content with a reason.
    Reject { reason: String },
    /// Escalate to a senior moderator.
    Escalate,
}

/// Error types for queue operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueError {
    /// Item not found in queue.
    ItemNotFound,
    /// Item already claimed by another moderator.
    AlreadyClaimed,
    /// Item not in the correct state for the operation.
    InvalidState {
        expected: ReviewState,
        actual: ReviewState,
    },
    /// Queue is empty.
    EmptyQueue,
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueError::ItemNotFound => write!(f, "review item not found"),
            QueueError::AlreadyClaimed => write!(f, "review item already claimed"),
            QueueError::InvalidState { expected, actual } => {
                write!(f, "expected state {:?}, got {:?}", expected, actual)
            }
            QueueError::EmptyQueue => write!(f, "review queue is empty"),
        }
    }
}

impl std::error::Error for QueueError {}

/// Human review queue with priority-based ordering.
pub struct ReviewQueue {
    items: Vec<ReviewItem>,
}

impl ReviewQueue {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Submit a new item to the review queue. Returns the item ID.
    pub fn submit(
        &mut self,
        content_id: String,
        priority: ReviewPriority,
        flag_reason: String,
        submitted_at_ms: u64,
    ) -> Uuid {
        let item = ReviewItem {
            item_id: Uuid::new_v4(),
            content_id,
            priority,
            state: ReviewState::Pending,
            assigned_moderator: None,
            flag_reason,
            submitted_at_ms,
            claimed_at_ms: None,
            decided_at_ms: None,
            decision_reason: None,
        };
        let id = item.item_id;
        self.items.push(item);
        id
    }

    /// Claim the highest-priority pending item for a moderator.
    /// Returns the item ID on success.
    pub fn claim_next(
        &mut self,
        moderator_id: &str,
        current_time_ms: u64,
    ) -> Result<Uuid, QueueError> {
        // Find the highest-priority pending item (highest priority first, then oldest)
        let idx = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.state == ReviewState::Pending)
            .max_by(|(_, a), (_, b)| {
                a.priority
                    .cmp(&b.priority)
                    .then_with(|| b.submitted_at_ms.cmp(&a.submitted_at_ms))
            })
            .map(|(i, _)| i)
            .ok_or(QueueError::EmptyQueue)?;

        self.items[idx].state = ReviewState::InReview;
        self.items[idx].assigned_moderator = Some(moderator_id.to_string());
        self.items[idx].claimed_at_ms = Some(current_time_ms);

        Ok(self.items[idx].item_id)
    }

    /// Claim a specific item by ID.
    pub fn claim_item(
        &mut self,
        item_id: Uuid,
        moderator_id: &str,
        current_time_ms: u64,
    ) -> Result<(), QueueError> {
        let item = self
            .items
            .iter_mut()
            .find(|i| i.item_id == item_id)
            .ok_or(QueueError::ItemNotFound)?;

        if item.state != ReviewState::Pending {
            if item.state == ReviewState::InReview {
                return Err(QueueError::AlreadyClaimed);
            }
            return Err(QueueError::InvalidState {
                expected: ReviewState::Pending,
                actual: item.state,
            });
        }

        item.state = ReviewState::InReview;
        item.assigned_moderator = Some(moderator_id.to_string());
        item.claimed_at_ms = Some(current_time_ms);
        Ok(())
    }

    /// Make a decision on a claimed item.
    pub fn decide(
        &mut self,
        item_id: Uuid,
        action: ReviewAction,
        current_time_ms: u64,
    ) -> Result<(), QueueError> {
        let item = self
            .items
            .iter_mut()
            .find(|i| i.item_id == item_id)
            .ok_or(QueueError::ItemNotFound)?;

        if item.state != ReviewState::InReview {
            return Err(QueueError::InvalidState {
                expected: ReviewState::InReview,
                actual: item.state,
            });
        }

        match &action {
            ReviewAction::Approve => {
                item.state = ReviewState::Approved;
                item.decision_reason = Some("approved".to_string());
            }
            ReviewAction::Reject { reason } => {
                item.state = ReviewState::Rejected;
                item.decision_reason = Some(reason.clone());
            }
            ReviewAction::Escalate => {
                // Escalation returns the item to pending with elevated priority
                item.state = ReviewState::Pending;
                item.priority = ReviewPriority::Urgent;
                item.assigned_moderator = None;
                item.claimed_at_ms = None;
                item.decision_reason = Some("escalated".to_string());
            }
        }

        item.decided_at_ms = Some(current_time_ms);
        Ok(())
    }

    /// Get a reference to an item by ID.
    pub fn get_item(&self, item_id: Uuid) -> Option<&ReviewItem> {
        self.items.iter().find(|i| i.item_id == item_id)
    }

    /// Returns the number of pending items.
    pub fn pending_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.state == ReviewState::Pending)
            .count()
    }

    /// Returns the total number of items in the queue.
    pub fn total_count(&self) -> usize {
        self.items.len()
    }

    /// Returns all items with a given state.
    pub fn items_with_state(&self, state: ReviewState) -> Vec<&ReviewItem> {
        self.items.iter().filter(|i| i.state == state).collect()
    }
}

impl Default for ReviewQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_submit_item() {
        let mut queue = ReviewQueue::new();
        let id = queue.submit(
            "content-1".to_string(),
            ReviewPriority::Medium,
            "flagged for nudity".to_string(),
            1000,
        );
        assert_eq!(queue.total_count(), 1);
        assert_eq!(queue.pending_count(), 1);
        let item = queue.get_item(id).unwrap();
        assert_eq!(item.content_id, "content-1");
        assert_eq!(item.state, ReviewState::Pending);
        assert!(item.assigned_moderator.is_none());
    }

    #[test]
    fn test_claim_next_empty_queue() {
        let mut queue = ReviewQueue::new();
        assert_eq!(
            queue.claim_next("mod-1", 2000),
            Err(QueueError::EmptyQueue)
        );
    }

    #[test]
    fn test_claim_next_single_item() {
        let mut queue = ReviewQueue::new();
        let id = queue.submit(
            "content-1".to_string(),
            ReviewPriority::Medium,
            "test".to_string(),
            1000,
        );
        let claimed_id = queue.claim_next("mod-1", 2000).unwrap();
        assert_eq!(claimed_id, id);
        let item = queue.get_item(id).unwrap();
        assert_eq!(item.state, ReviewState::InReview);
        assert_eq!(item.assigned_moderator.as_deref(), Some("mod-1"));
        assert_eq!(item.claimed_at_ms, Some(2000));
    }

    #[test]
    fn test_claim_next_priority_ordering() {
        let mut queue = ReviewQueue::new();
        let low_id = queue.submit(
            "content-low".to_string(),
            ReviewPriority::Low,
            "test".to_string(),
            1000,
        );
        let high_id = queue.submit(
            "content-high".to_string(),
            ReviewPriority::High,
            "test".to_string(),
            2000,
        );
        let medium_id = queue.submit(
            "content-medium".to_string(),
            ReviewPriority::Medium,
            "test".to_string(),
            3000,
        );

        // Should claim the highest priority first
        let first = queue.claim_next("mod-1", 4000).unwrap();
        assert_eq!(first, high_id);

        let second = queue.claim_next("mod-2", 5000).unwrap();
        assert_eq!(second, medium_id);

        let third = queue.claim_next("mod-3", 6000).unwrap();
        assert_eq!(third, low_id);

        // Queue empty
        assert_eq!(
            queue.claim_next("mod-4", 7000),
            Err(QueueError::EmptyQueue)
        );
    }

    #[test]
    fn test_claim_next_same_priority_oldest_first() {
        let mut queue = ReviewQueue::new();
        let first_id = queue.submit(
            "content-1".to_string(),
            ReviewPriority::Medium,
            "test".to_string(),
            1000,
        );
        let _second_id = queue.submit(
            "content-2".to_string(),
            ReviewPriority::Medium,
            "test".to_string(),
            2000,
        );

        let claimed = queue.claim_next("mod-1", 3000).unwrap();
        assert_eq!(claimed, first_id);
    }

    #[test]
    fn test_claim_specific_item() {
        let mut queue = ReviewQueue::new();
        let id = queue.submit(
            "content-1".to_string(),
            ReviewPriority::Medium,
            "test".to_string(),
            1000,
        );
        queue.claim_item(id, "mod-1", 2000).unwrap();
        let item = queue.get_item(id).unwrap();
        assert_eq!(item.state, ReviewState::InReview);
    }

    #[test]
    fn test_claim_nonexistent_item() {
        let mut queue = ReviewQueue::new();
        let fake_id = Uuid::new_v4();
        assert_eq!(
            queue.claim_item(fake_id, "mod-1", 2000),
            Err(QueueError::ItemNotFound)
        );
    }

    #[test]
    fn test_claim_already_claimed_item() {
        let mut queue = ReviewQueue::new();
        let id = queue.submit(
            "content-1".to_string(),
            ReviewPriority::Medium,
            "test".to_string(),
            1000,
        );
        queue.claim_item(id, "mod-1", 2000).unwrap();
        assert_eq!(
            queue.claim_item(id, "mod-2", 3000),
            Err(QueueError::AlreadyClaimed)
        );
    }

    #[test]
    fn test_decide_approve() {
        let mut queue = ReviewQueue::new();
        let id = queue.submit(
            "content-1".to_string(),
            ReviewPriority::Medium,
            "test".to_string(),
            1000,
        );
        queue.claim_item(id, "mod-1", 2000).unwrap();
        queue.decide(id, ReviewAction::Approve, 3000).unwrap();

        let item = queue.get_item(id).unwrap();
        assert_eq!(item.state, ReviewState::Approved);
        assert_eq!(item.decided_at_ms, Some(3000));
        assert_eq!(item.decision_reason.as_deref(), Some("approved"));
    }

    #[test]
    fn test_decide_reject() {
        let mut queue = ReviewQueue::new();
        let id = queue.submit(
            "content-1".to_string(),
            ReviewPriority::Medium,
            "test".to_string(),
            1000,
        );
        queue.claim_item(id, "mod-1", 2000).unwrap();
        queue
            .decide(
                id,
                ReviewAction::Reject {
                    reason: "inappropriate content".to_string(),
                },
                3000,
            )
            .unwrap();

        let item = queue.get_item(id).unwrap();
        assert_eq!(item.state, ReviewState::Rejected);
        assert_eq!(
            item.decision_reason.as_deref(),
            Some("inappropriate content")
        );
    }

    #[test]
    fn test_decide_escalate() {
        let mut queue = ReviewQueue::new();
        let id = queue.submit(
            "content-1".to_string(),
            ReviewPriority::Medium,
            "test".to_string(),
            1000,
        );
        queue.claim_item(id, "mod-1", 2000).unwrap();
        queue.decide(id, ReviewAction::Escalate, 3000).unwrap();

        let item = queue.get_item(id).unwrap();
        // Escalation returns item to pending with Urgent priority
        assert_eq!(item.state, ReviewState::Pending);
        assert_eq!(item.priority, ReviewPriority::Urgent);
        assert!(item.assigned_moderator.is_none());
    }

    #[test]
    fn test_decide_on_pending_item_fails() {
        let mut queue = ReviewQueue::new();
        let id = queue.submit(
            "content-1".to_string(),
            ReviewPriority::Medium,
            "test".to_string(),
            1000,
        );
        // Try to decide without claiming first
        assert_eq!(
            queue.decide(id, ReviewAction::Approve, 2000),
            Err(QueueError::InvalidState {
                expected: ReviewState::InReview,
                actual: ReviewState::Pending,
            })
        );
    }

    #[test]
    fn test_decide_on_nonexistent_item() {
        let mut queue = ReviewQueue::new();
        assert_eq!(
            queue.decide(Uuid::new_v4(), ReviewAction::Approve, 2000),
            Err(QueueError::ItemNotFound)
        );
    }

    #[test]
    fn test_pending_count() {
        let mut queue = ReviewQueue::new();
        assert_eq!(queue.pending_count(), 0);

        let id1 = queue.submit(
            "c1".to_string(),
            ReviewPriority::Low,
            "test".to_string(),
            1000,
        );
        let _id2 = queue.submit(
            "c2".to_string(),
            ReviewPriority::Low,
            "test".to_string(),
            2000,
        );
        assert_eq!(queue.pending_count(), 2);

        queue.claim_item(id1, "mod-1", 3000).unwrap();
        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn test_items_with_state() {
        let mut queue = ReviewQueue::new();
        let id1 = queue.submit(
            "c1".to_string(),
            ReviewPriority::Low,
            "test".to_string(),
            1000,
        );
        let _id2 = queue.submit(
            "c2".to_string(),
            ReviewPriority::Low,
            "test".to_string(),
            2000,
        );

        assert_eq!(queue.items_with_state(ReviewState::Pending).len(), 2);
        assert_eq!(queue.items_with_state(ReviewState::InReview).len(), 0);

        queue.claim_item(id1, "mod-1", 3000).unwrap();
        assert_eq!(queue.items_with_state(ReviewState::Pending).len(), 1);
        assert_eq!(queue.items_with_state(ReviewState::InReview).len(), 1);
    }

    #[test]
    fn test_priority_from_severity() {
        assert_eq!(
            ReviewPriority::from(ContentSeverity::Clean),
            ReviewPriority::Low
        );
        assert_eq!(
            ReviewPriority::from(ContentSeverity::Low),
            ReviewPriority::Low
        );
        assert_eq!(
            ReviewPriority::from(ContentSeverity::Medium),
            ReviewPriority::Medium
        );
        assert_eq!(
            ReviewPriority::from(ContentSeverity::High),
            ReviewPriority::High
        );
        assert_eq!(
            ReviewPriority::from(ContentSeverity::Critical),
            ReviewPriority::Urgent
        );
    }

    #[test]
    fn test_default_queue() {
        let queue = ReviewQueue::default();
        assert_eq!(queue.total_count(), 0);
    }

    #[test]
    fn test_escalated_item_can_be_reclaimed() {
        let mut queue = ReviewQueue::new();
        let id = queue.submit(
            "content-1".to_string(),
            ReviewPriority::Medium,
            "test".to_string(),
            1000,
        );
        queue.claim_item(id, "mod-1", 2000).unwrap();
        queue.decide(id, ReviewAction::Escalate, 3000).unwrap();

        // Item is back in pending, so another moderator can claim
        queue.claim_item(id, "mod-2", 4000).unwrap();
        let item = queue.get_item(id).unwrap();
        assert_eq!(item.state, ReviewState::InReview);
        assert_eq!(item.assigned_moderator.as_deref(), Some("mod-2"));
    }

    #[test]
    fn test_claim_already_decided_item() {
        let mut queue = ReviewQueue::new();
        let id = queue.submit(
            "content-1".to_string(),
            ReviewPriority::Medium,
            "test".to_string(),
            1000,
        );
        queue.claim_item(id, "mod-1", 2000).unwrap();
        queue.decide(id, ReviewAction::Approve, 3000).unwrap();

        // Cannot claim an approved item
        assert_eq!(
            queue.claim_item(id, "mod-2", 4000),
            Err(QueueError::InvalidState {
                expected: ReviewState::Pending,
                actual: ReviewState::Approved,
            })
        );
    }
}
