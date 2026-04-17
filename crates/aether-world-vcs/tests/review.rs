//! Review surface: workflow + policy permutations.

use aether_world_vcs::{
    Cid, MemoryReviewStore, MergeDecision, MergePolicy, ReviewStatus, ReviewStore, ReviewerRef,
};

fn cid_of_nth(n: u8) -> Cid {
    let mut c = [0u8; 32];
    c[0] = n;
    c
}

fn human(id: &str) -> ReviewerRef {
    ReviewerRef::Human {
        user_id: id.into(),
    }
}

fn agent(id: &str) -> ReviewerRef {
    ReviewerRef::Agent {
        service_account: id.into(),
    }
}

#[test]
fn all_reviewers_policy_requires_unanimous_approval() {
    let mut store = MemoryReviewStore::new();
    let diff = cid_of_nth(1);
    let reviewers = vec![human("alice"), agent("agent.one")];
    store.request_review(diff, reviewers.clone()).unwrap();

    // Single approval -> blocked.
    store
        .review(diff, &reviewers[0], ReviewStatus::Approved, None)
        .unwrap();
    match store
        .merge_when_approved(&diff, &MergePolicy::AllReviewers)
        .unwrap()
    {
        MergeDecision::Blocked { .. } => {}
        other => panic!("expected Blocked, got {other:?}"),
    }

    // Both approve -> approved.
    store
        .review(diff, &reviewers[1], ReviewStatus::Approved, None)
        .unwrap();
    match store
        .merge_when_approved(&diff, &MergePolicy::AllReviewers)
        .unwrap()
    {
        MergeDecision::Approved { diff_cid } => assert_eq!(diff_cid, diff),
        other => panic!("expected Approved, got {other:?}"),
    }
}

#[test]
fn majority_policy_counts_strict_majority() {
    let mut store = MemoryReviewStore::new();
    let diff = cid_of_nth(2);
    let reviewers = vec![human("a"), human("b"), human("c")];
    store.request_review(diff, reviewers.clone()).unwrap();

    // 1/3 approved -> blocked.
    store
        .review(diff, &reviewers[0], ReviewStatus::Approved, None)
        .unwrap();
    assert!(matches!(
        store
            .merge_when_approved(&diff, &MergePolicy::Majority)
            .unwrap(),
        MergeDecision::Blocked { .. }
    ));

    // 2/3 approved -> approved.
    store
        .review(diff, &reviewers[1], ReviewStatus::Approved, None)
        .unwrap();
    assert!(matches!(
        store
            .merge_when_approved(&diff, &MergePolicy::Majority)
            .unwrap(),
        MergeDecision::Approved { .. }
    ));
}

#[test]
fn any_one_of_policy_triggers_on_whitelisted_approver() {
    let mut store = MemoryReviewStore::new();
    let diff = cid_of_nth(3);
    let alice = human("alice");
    let bob = human("bob");
    let reviewers = vec![alice.clone(), bob.clone()];
    store.request_review(diff, reviewers).unwrap();

    // Bob approves, but policy only lists Alice -> blocked.
    store
        .review(diff, &bob, ReviewStatus::Approved, None)
        .unwrap();
    let policy = MergePolicy::AnyOneOf {
        any_of: vec![alice.clone()],
    };
    assert!(matches!(
        store.merge_when_approved(&diff, &policy).unwrap(),
        MergeDecision::Blocked { .. }
    ));

    // Alice approves -> approved.
    store
        .review(diff, &alice, ReviewStatus::Approved, None)
        .unwrap();
    assert!(matches!(
        store.merge_when_approved(&diff, &policy).unwrap(),
        MergeDecision::Approved { .. }
    ));
}

#[test]
fn rejection_shortcircuits_every_policy() {
    let mut store = MemoryReviewStore::new();
    let diff = cid_of_nth(4);
    let reviewers = vec![human("a"), human("b"), human("c")];
    store.request_review(diff, reviewers.clone()).unwrap();

    // Two approve, one rejects -> blocked under every policy.
    store
        .review(diff, &reviewers[0], ReviewStatus::Approved, None)
        .unwrap();
    store
        .review(diff, &reviewers[1], ReviewStatus::Approved, None)
        .unwrap();
    store
        .review(diff, &reviewers[2], ReviewStatus::Rejected, None)
        .unwrap();

    for policy in [
        MergePolicy::AllReviewers,
        MergePolicy::Majority,
        MergePolicy::AnyOneOf {
            any_of: vec![reviewers[0].clone()],
        },
    ] {
        assert!(matches!(
            store.merge_when_approved(&diff, &policy).unwrap(),
            MergeDecision::Blocked { .. }
        ));
    }
}

#[test]
fn withdrawn_review_is_treated_as_pending() {
    let mut store = MemoryReviewStore::new();
    let diff = cid_of_nth(5);
    let reviewers = vec![human("alice"), human("bob")];
    store.request_review(diff, reviewers.clone()).unwrap();

    store
        .review(diff, &reviewers[0], ReviewStatus::Approved, None)
        .unwrap();
    store
        .review(diff, &reviewers[1], ReviewStatus::Withdrawn, None)
        .unwrap();

    // AllReviewers -> blocked (not everyone approved).
    assert!(matches!(
        store
            .merge_when_approved(&diff, &MergePolicy::AllReviewers)
            .unwrap(),
        MergeDecision::Blocked { .. }
    ));
    // Majority -> 1/2 approved is not strict majority -> blocked.
    assert!(matches!(
        store
            .merge_when_approved(&diff, &MergePolicy::Majority)
            .unwrap(),
        MergeDecision::Blocked { .. }
    ));
}

#[test]
fn unknown_reviewer_is_rejected() {
    let mut store = MemoryReviewStore::new();
    let diff = cid_of_nth(6);
    store.request_review(diff, vec![human("alice")]).unwrap();
    let err = store
        .review(diff, &human("stranger"), ReviewStatus::Approved, None)
        .unwrap_err();
    assert!(matches!(
        err,
        aether_world_vcs::VcsError::UnknownReviewer(_)
    ));
}
