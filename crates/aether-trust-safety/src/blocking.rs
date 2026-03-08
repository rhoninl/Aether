//! User blocking system with mutual invisibility.
//!
//! When user A blocks user B, neither can see the other. This is
//! implemented as a per-user block list with a mutual-check helper.

use std::collections::HashSet;

/// A per-user block list.
#[derive(Debug, Clone, Default)]
pub struct BlockList {
    blocked: HashSet<u64>,
}

impl BlockList {
    /// Create a new empty block list.
    pub fn new() -> Self {
        Self {
            blocked: HashSet::new(),
        }
    }

    /// Block a user. Returns `true` if the user was not already blocked.
    pub fn block(&mut self, user_id: u64) -> bool {
        self.blocked.insert(user_id)
    }

    /// Unblock a user. Returns `true` if the user was previously blocked.
    pub fn unblock(&mut self, user_id: u64) -> bool {
        self.blocked.remove(&user_id)
    }

    /// Check if a specific user is blocked.
    pub fn is_blocked(&self, user_id: u64) -> bool {
        self.blocked.contains(&user_id)
    }

    /// Return the number of blocked users.
    pub fn count(&self) -> usize {
        self.blocked.len()
    }

    /// Filter a list of user IDs, returning only those NOT blocked.
    pub fn filter_visible(&self, user_ids: &[u64]) -> Vec<u64> {
        user_ids
            .iter()
            .copied()
            .filter(|id| !self.blocked.contains(id))
            .collect()
    }
}

/// Check if two users have a mutual block relationship.
///
/// Returns `true` if A blocks B OR B blocks A (either direction
/// triggers mutual invisibility).
pub fn is_mutually_blocked(a_list: &BlockList, b_id: u64, b_list: &BlockList, a_id: u64) -> bool {
    a_list.is_blocked(b_id) || b_list.is_blocked(a_id)
}

/// Given a user's block list and the block lists of all other users,
/// return the IDs of users that should be visible (no mutual block).
///
/// `others` is a slice of `(user_id, their_block_list)` pairs.
pub fn filter_mutually_visible(
    self_id: u64,
    self_list: &BlockList,
    others: &[(u64, &BlockList)],
) -> Vec<u64> {
    others
        .iter()
        .filter(|(other_id, other_list)| {
            !is_mutually_blocked(self_list, *other_id, other_list, self_id)
        })
        .map(|(id, _)| *id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_blocklist_is_empty() {
        let bl = BlockList::new();
        assert_eq!(bl.count(), 0);
        assert!(!bl.is_blocked(1));
    }

    #[test]
    fn block_adds_user() {
        let mut bl = BlockList::new();
        assert!(bl.block(42));
        assert!(bl.is_blocked(42));
        assert_eq!(bl.count(), 1);
    }

    #[test]
    fn block_duplicate_returns_false() {
        let mut bl = BlockList::new();
        bl.block(42);
        assert!(!bl.block(42));
        assert_eq!(bl.count(), 1);
    }

    #[test]
    fn unblock_removes_user() {
        let mut bl = BlockList::new();
        bl.block(42);
        assert!(bl.unblock(42));
        assert!(!bl.is_blocked(42));
        assert_eq!(bl.count(), 0);
    }

    #[test]
    fn unblock_nonexistent_returns_false() {
        let mut bl = BlockList::new();
        assert!(!bl.unblock(99));
    }

    #[test]
    fn filter_visible_removes_blocked() {
        let mut bl = BlockList::new();
        bl.block(2);
        bl.block(4);
        let visible = bl.filter_visible(&[1, 2, 3, 4, 5]);
        assert_eq!(visible, vec![1, 3, 5]);
    }

    #[test]
    fn filter_visible_empty_blocklist() {
        let bl = BlockList::new();
        let visible = bl.filter_visible(&[1, 2, 3]);
        assert_eq!(visible, vec![1, 2, 3]);
    }

    #[test]
    fn filter_visible_all_blocked() {
        let mut bl = BlockList::new();
        bl.block(1);
        bl.block(2);
        let visible = bl.filter_visible(&[1, 2]);
        assert!(visible.is_empty());
    }

    #[test]
    fn mutual_block_a_blocks_b() {
        let mut a_list = BlockList::new();
        let b_list = BlockList::new();
        a_list.block(2);
        assert!(is_mutually_blocked(&a_list, 2, &b_list, 1));
    }

    #[test]
    fn mutual_block_b_blocks_a() {
        let a_list = BlockList::new();
        let mut b_list = BlockList::new();
        b_list.block(1);
        assert!(is_mutually_blocked(&a_list, 2, &b_list, 1));
    }

    #[test]
    fn mutual_block_both_block() {
        let mut a_list = BlockList::new();
        let mut b_list = BlockList::new();
        a_list.block(2);
        b_list.block(1);
        assert!(is_mutually_blocked(&a_list, 2, &b_list, 1));
    }

    #[test]
    fn mutual_block_neither_blocks() {
        let a_list = BlockList::new();
        let b_list = BlockList::new();
        assert!(!is_mutually_blocked(&a_list, 2, &b_list, 1));
    }

    #[test]
    fn filter_mutually_visible_works() {
        let mut my_list = BlockList::new();
        my_list.block(2); // I block user 2

        let other1_list = BlockList::new(); // user 1 doesn't block me
        let mut other3_list = BlockList::new();
        other3_list.block(10); // user 3 blocks me (I am user 10)

        let others: Vec<(u64, &BlockList)> = vec![
            (1, &other1_list),
            (2, &my_list),      // user 2 -- I block them
            (3, &other3_list),  // user 3 -- they block me
        ];

        let visible = filter_mutually_visible(10, &my_list, &others);
        assert_eq!(visible, vec![1]);
    }

    #[test]
    fn filter_mutually_visible_no_blocks() {
        let my_list = BlockList::new();
        let other1 = BlockList::new();
        let other2 = BlockList::new();
        let others: Vec<(u64, &BlockList)> = vec![(1, &other1), (2, &other2)];
        let visible = filter_mutually_visible(10, &my_list, &others);
        assert_eq!(visible, vec![1, 2]);
    }

    #[test]
    fn default_blocklist_is_empty() {
        let bl = BlockList::default();
        assert_eq!(bl.count(), 0);
    }
}
