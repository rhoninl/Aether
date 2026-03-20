//! User blocking with mutual invisibility enforcement.

use std::collections::{HashMap, HashSet};

use crate::error::{SocialError, SocialResult};

/// Manages directional user blocks with bidirectional visibility checks.
#[derive(Debug, Default)]
pub struct BlockList {
    /// Map from blocker -> set of blocked user IDs.
    blocked: HashMap<u64, HashSet<u64>>,
}

impl BlockList {
    pub fn new() -> Self {
        Self::default()
    }

    /// Block a target user. Returns error if already blocked or self-action.
    pub fn block(&mut self, user_id: u64, target_id: u64) -> SocialResult<()> {
        if user_id == target_id {
            return Err(SocialError::SelfAction);
        }
        let set = self.blocked.entry(user_id).or_default();
        if !set.insert(target_id) {
            return Err(SocialError::AlreadyBlocked);
        }
        Ok(())
    }

    /// Unblock a target user. Returns error if not currently blocked.
    pub fn unblock(&mut self, user_id: u64, target_id: u64) -> SocialResult<()> {
        if user_id == target_id {
            return Err(SocialError::SelfAction);
        }
        let set = self
            .blocked
            .get_mut(&user_id)
            .ok_or(SocialError::NotBlocked)?;
        if !set.remove(&target_id) {
            return Err(SocialError::NotBlocked);
        }
        if set.is_empty() {
            self.blocked.remove(&user_id);
        }
        Ok(())
    }

    /// Check if `user_id` has blocked `target_id` (one-directional).
    pub fn has_blocked(&self, user_id: u64, target_id: u64) -> bool {
        self.blocked
            .get(&user_id)
            .is_some_and(|set| set.contains(&target_id))
    }

    /// Check if either user has blocked the other (bidirectional).
    pub fn is_blocked_either(&self, a: u64, b: u64) -> bool {
        self.has_blocked(a, b) || self.has_blocked(b, a)
    }

    /// Return all users blocked by `user_id`.
    pub fn blocked_by(&self, user_id: u64) -> Vec<u64> {
        self.blocked
            .get(&user_id)
            .map_or_else(Vec::new, |set| set.iter().copied().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_and_check() {
        let mut bl = BlockList::new();
        assert!(!bl.has_blocked(1, 2));
        bl.block(1, 2).unwrap();
        assert!(bl.has_blocked(1, 2));
        assert!(!bl.has_blocked(2, 1));
    }

    #[test]
    fn block_is_bidirectional_check() {
        let mut bl = BlockList::new();
        bl.block(1, 2).unwrap();
        assert!(bl.is_blocked_either(1, 2));
        assert!(bl.is_blocked_either(2, 1));
    }

    #[test]
    fn unblock() {
        let mut bl = BlockList::new();
        bl.block(1, 2).unwrap();
        bl.unblock(1, 2).unwrap();
        assert!(!bl.has_blocked(1, 2));
        assert!(!bl.is_blocked_either(1, 2));
    }

    #[test]
    fn unblock_not_blocked_errors() {
        let mut bl = BlockList::new();
        assert_eq!(bl.unblock(1, 2), Err(SocialError::NotBlocked));
    }

    #[test]
    fn block_self_errors() {
        let mut bl = BlockList::new();
        assert_eq!(bl.block(1, 1), Err(SocialError::SelfAction));
    }

    #[test]
    fn double_block_errors() {
        let mut bl = BlockList::new();
        bl.block(1, 2).unwrap();
        assert_eq!(bl.block(1, 2), Err(SocialError::AlreadyBlocked));
    }

    #[test]
    fn blocked_by_returns_all_blocked() {
        let mut bl = BlockList::new();
        bl.block(1, 2).unwrap();
        bl.block(1, 3).unwrap();
        let mut list = bl.blocked_by(1);
        list.sort();
        assert_eq!(list, vec![2, 3]);
    }

    #[test]
    fn blocked_by_empty() {
        let bl = BlockList::new();
        assert!(bl.blocked_by(1).is_empty());
    }

    #[test]
    fn unblock_self_errors() {
        let mut bl = BlockList::new();
        assert_eq!(bl.unblock(1, 1), Err(SocialError::SelfAction));
    }

    #[test]
    fn mutual_block() {
        let mut bl = BlockList::new();
        bl.block(1, 2).unwrap();
        bl.block(2, 1).unwrap();
        assert!(bl.has_blocked(1, 2));
        assert!(bl.has_blocked(2, 1));
        bl.unblock(1, 2).unwrap();
        // 2 still blocks 1
        assert!(bl.is_blocked_either(1, 2));
        bl.unblock(2, 1).unwrap();
        assert!(!bl.is_blocked_either(1, 2));
    }
}
