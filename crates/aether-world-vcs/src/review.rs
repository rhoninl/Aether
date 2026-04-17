//! Review surface for humans + agents.
//!
//! A [`Review`] gates a diff CID behind one or more reviewers. Each
//! reviewer is either a [`ReviewerRef::Human`] or a
//! [`ReviewerRef::Agent`] — they are first-class peers, which is the
//! whole point of the "agent-native engine" reposition.
//!
//! The review flow is:
//!
//! ```text
//!   request_review(diff_cid, reviewers) -> Review
//!   review(review, reviewer, status, comment)
//!   merge_when_approved(review, policy) -> MergeDecision
//! ```

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::diff::{cid_to_hex, Cid};
use crate::error::{Result, VcsError};

/// A reviewer identity.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReviewerRef {
    /// A human reviewer identified by opaque user id.
    Human {
        /// Opaque user id.
        user_id: String,
    },
    /// An agent reviewer identified by opaque service account.
    Agent {
        /// Opaque service-account id.
        service_account: String,
    },
}

/// A reviewer slot on a review: the reference + a current status.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reviewer {
    /// Who this slot belongs to.
    pub who: ReviewerRef,
    /// Their current status on this review.
    pub status: ReviewStatus,
}

/// Current state of a reviewer (or a review as a whole).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    /// No decision yet.
    Pending,
    /// Approved by this reviewer.
    Approved,
    /// Rejected by this reviewer.
    Rejected,
    /// Reviewer has withdrawn from the review (neither approve nor
    /// reject). Treated like `Pending` for policy purposes.
    Withdrawn,
}

/// A comment left by a reviewer on a review.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Comment {
    /// Who left the comment.
    pub by: ReviewerRef,
    /// Free-form comment text.
    pub text: String,
    /// When the comment was left (unix ms).
    pub at_unix_ms: u64,
}

/// A review gate in front of merging a diff.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Review {
    /// Diff this review gates.
    pub diff_cid: Cid,
    /// Reviewer slots + their statuses.
    pub reviewers: Vec<Reviewer>,
    /// Aggregate status of the review.
    pub status: ReviewStatus,
    /// Comment thread.
    pub comments: Vec<Comment>,
}

impl Review {
    /// Whether `who` appears among this review's reviewers.
    pub fn has_reviewer(&self, who: &ReviewerRef) -> bool {
        self.reviewers.iter().any(|r| &r.who == who)
    }
}

/// Merge policy: when is a review considered "ready"?
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MergePolicy {
    /// Every reviewer must approve.
    AllReviewers,
    /// Strict majority of reviewers approved (>50%). Rejected counts
    /// negatively; pending/withdrawn do not.
    Majority,
    /// Any one of the listed reviewers approved.
    AnyOneOf {
        /// Whitelist.
        any_of: Vec<ReviewerRef>,
    },
}

/// Outcome of applying a merge policy to a review.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MergeDecision {
    /// Merge is approved under the policy.
    Approved {
        /// Diff CID cleared for merge.
        diff_cid: Cid,
    },
    /// Merge is blocked; the reason is human-readable.
    Blocked {
        /// Human-readable reason why the decision blocked.
        reason: String,
    },
}

/// A review store: keyed by diff CID.
pub trait ReviewStore {
    /// Create a review gate for a diff with the given reviewers.
    fn request_review(
        &mut self,
        diff_cid: Cid,
        reviewers: Vec<ReviewerRef>,
    ) -> Result<Review>;

    /// Record a review action by `who`.
    fn review(
        &mut self,
        diff_cid: Cid,
        who: &ReviewerRef,
        status: ReviewStatus,
        comment: Option<Comment>,
    ) -> Result<Review>;

    /// Fetch a review by diff CID.
    fn get(&self, diff_cid: &Cid) -> Result<Review>;

    /// Apply `policy` to a review and return the decision.
    fn merge_when_approved(&self, diff_cid: &Cid, policy: &MergePolicy) -> Result<MergeDecision>;
}

/// In-memory [`ReviewStore`].
#[derive(Debug, Default)]
pub struct MemoryReviewStore {
    reviews: BTreeMap<Cid, Review>,
}

impl MemoryReviewStore {
    /// Empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

fn recompute_aggregate(review: &mut Review) {
    // Any outright rejection shortcircuits to Rejected.
    if review
        .reviewers
        .iter()
        .any(|r| r.status == ReviewStatus::Rejected)
    {
        review.status = ReviewStatus::Rejected;
        return;
    }
    // Every reviewer decided approved -> Approved.
    if !review.reviewers.is_empty()
        && review
            .reviewers
            .iter()
            .all(|r| r.status == ReviewStatus::Approved)
    {
        review.status = ReviewStatus::Approved;
        return;
    }
    review.status = ReviewStatus::Pending;
}

impl ReviewStore for MemoryReviewStore {
    fn request_review(
        &mut self,
        diff_cid: Cid,
        reviewers: Vec<ReviewerRef>,
    ) -> Result<Review> {
        let review = Review {
            diff_cid,
            reviewers: reviewers
                .into_iter()
                .map(|who| Reviewer {
                    who,
                    status: ReviewStatus::Pending,
                })
                .collect(),
            status: ReviewStatus::Pending,
            comments: Vec::new(),
        };
        self.reviews.insert(diff_cid, review.clone());
        Ok(review)
    }

    fn review(
        &mut self,
        diff_cid: Cid,
        who: &ReviewerRef,
        status: ReviewStatus,
        comment: Option<Comment>,
    ) -> Result<Review> {
        let review = self
            .reviews
            .get_mut(&diff_cid)
            .ok_or_else(|| VcsError::UnknownDiff(cid_to_hex(&diff_cid)))?;
        let slot = review
            .reviewers
            .iter_mut()
            .find(|r| &r.who == who)
            .ok_or_else(|| VcsError::UnknownReviewer(cid_to_hex(&diff_cid)))?;
        slot.status = status;
        if let Some(c) = comment {
            review.comments.push(c);
        }
        recompute_aggregate(review);
        Ok(review.clone())
    }

    fn get(&self, diff_cid: &Cid) -> Result<Review> {
        self.reviews
            .get(diff_cid)
            .cloned()
            .ok_or_else(|| VcsError::UnknownDiff(cid_to_hex(diff_cid)))
    }

    fn merge_when_approved(&self, diff_cid: &Cid, policy: &MergePolicy) -> Result<MergeDecision> {
        let review = self.get(diff_cid)?;
        // Any rejection shortcircuits every policy.
        if review
            .reviewers
            .iter()
            .any(|r| r.status == ReviewStatus::Rejected)
        {
            return Ok(MergeDecision::Blocked {
                reason: "at least one reviewer rejected".into(),
            });
        }
        let approved_count = review
            .reviewers
            .iter()
            .filter(|r| r.status == ReviewStatus::Approved)
            .count();
        let total = review.reviewers.len();
        let decision = match policy {
            MergePolicy::AllReviewers => {
                if total > 0 && approved_count == total {
                    MergeDecision::Approved {
                        diff_cid: *diff_cid,
                    }
                } else {
                    MergeDecision::Blocked {
                        reason: format!(
                            "{approved_count}/{total} reviewers approved; policy requires all"
                        ),
                    }
                }
            }
            MergePolicy::Majority => {
                if approved_count * 2 > total {
                    MergeDecision::Approved {
                        diff_cid: *diff_cid,
                    }
                } else {
                    MergeDecision::Blocked {
                        reason: format!(
                            "{approved_count}/{total} reviewers approved; policy requires majority"
                        ),
                    }
                }
            }
            MergePolicy::AnyOneOf { any_of } => {
                let ok = review.reviewers.iter().any(|r| {
                    r.status == ReviewStatus::Approved && any_of.iter().any(|w| w == &r.who)
                });
                if ok {
                    MergeDecision::Approved {
                        diff_cid: *diff_cid,
                    }
                } else {
                    MergeDecision::Blocked {
                        reason: "no whitelisted reviewer has approved yet".into(),
                    }
                }
            }
        };
        Ok(decision)
    }
}
