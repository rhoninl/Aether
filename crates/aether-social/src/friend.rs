//! Friend system: request, accept, reject, unfriend flows.

use std::collections::HashMap;

use crate::blocking::BlockList;
use crate::error::{SocialError, SocialResult};

/// The state of a friendship between two users.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FriendState {
    Pending,
    Accepted,
    Blocked,
    Rejected,
}

/// A record of the friendship status between two users.
#[derive(Debug, Clone)]
pub struct FriendStatus {
    pub user_a: u64,
    pub user_b: u64,
    pub state: FriendState,
    pub initiated_ms: u64,
}

/// Friend request action envelopes.
#[derive(Debug, Clone)]
pub enum FriendRequest {
    Send {
        from: u64,
        to: u64,
        message: Option<String>,
    },
    Accept {
        from: u64,
        to: u64,
    },
    Reject {
        from: u64,
        to: u64,
    },
    Block {
        from: u64,
        to: u64,
    },
}

/// Summary statistics for a user's friend list.
#[derive(Debug)]
pub struct FriendSummary {
    pub user_id: u64,
    pub total_friends: u32,
    pub pending_requests: u32,
    pub blocked_users: u32,
}

/// Manages friend relationships between users.
///
/// Stores friendship state as a directed map: `from_user -> { to_user -> state }`.
/// For accepted friendships, both directions are stored.
#[derive(Debug, Default)]
pub struct FriendManager {
    /// from_user -> (to_user -> state)
    friendships: HashMap<u64, HashMap<u64, FriendState>>,
}

impl FriendManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Send a friend request from `from` to `to`.
    ///
    /// Fails if users are blocked, already friends, request already pending,
    /// or if sending to self.
    pub fn send_request(&mut self, from: u64, to: u64, block_list: &BlockList) -> SocialResult<()> {
        if from == to {
            return Err(SocialError::SelfAction);
        }
        if block_list.is_blocked_either(from, to) {
            return Err(SocialError::UserBlocked);
        }
        // Check if already friends (from perspective of `from`)
        if let Some(state) = self.get_state(from, to) {
            match state {
                FriendState::Accepted => return Err(SocialError::AlreadyFriends),
                FriendState::Pending => return Err(SocialError::AlreadyPending),
                FriendState::Blocked | FriendState::Rejected => {
                    // Allow re-request after rejection or block-then-unblock
                }
            }
        }
        // Check reverse direction too
        if let Some(FriendState::Pending) = self.get_state(to, from) {
            return Err(SocialError::AlreadyPending);
        }
        if let Some(FriendState::Accepted) = self.get_state(to, from) {
            return Err(SocialError::AlreadyFriends);
        }

        self.set_state(from, to, FriendState::Pending);
        Ok(())
    }

    /// Accept a pending friend request. The `acceptor` is the recipient of
    /// the original request, and `requester` is who sent it.
    pub fn accept_request(&mut self, requester: u64, acceptor: u64) -> SocialResult<()> {
        match self.get_state(requester, acceptor) {
            Some(FriendState::Pending) => {}
            _ => return Err(SocialError::RequestNotFound),
        }
        self.set_state(requester, acceptor, FriendState::Accepted);
        self.set_state(acceptor, requester, FriendState::Accepted);
        Ok(())
    }

    /// Reject a pending friend request. The `rejector` is the recipient.
    pub fn reject_request(&mut self, requester: u64, rejector: u64) -> SocialResult<()> {
        match self.get_state(requester, rejector) {
            Some(FriendState::Pending) => {}
            _ => return Err(SocialError::RequestNotFound),
        }
        self.set_state(requester, rejector, FriendState::Rejected);
        Ok(())
    }

    /// Remove an existing friendship between two users.
    pub fn unfriend(&mut self, user_a: u64, user_b: u64) -> SocialResult<()> {
        let is_friends = self.get_state(user_a, user_b) == Some(FriendState::Accepted)
            || self.get_state(user_b, user_a) == Some(FriendState::Accepted);
        if !is_friends {
            return Err(SocialError::NotFriends);
        }
        self.remove_state(user_a, user_b);
        self.remove_state(user_b, user_a);
        Ok(())
    }

    /// Remove all friendship records involving `target` for `user_id`.
    /// Called when a user blocks another.
    pub fn remove_friendship(&mut self, user_id: u64, target: u64) {
        self.remove_state(user_id, target);
        self.remove_state(target, user_id);
    }

    /// Get all accepted friends for a user.
    pub fn get_friends(&self, user_id: u64) -> Vec<u64> {
        self.friendships.get(&user_id).map_or_else(Vec::new, |map| {
            map.iter()
                .filter(|(_, state)| **state == FriendState::Accepted)
                .map(|(id, _)| *id)
                .collect()
        })
    }

    /// Get all users who have sent pending requests to `user_id`.
    pub fn get_pending_requests(&self, user_id: u64) -> Vec<u64> {
        let mut requesters = Vec::new();
        for (from, targets) in &self.friendships {
            if let Some(FriendState::Pending) = targets.get(&user_id) {
                requesters.push(*from);
            }
        }
        requesters
    }

    /// Check if two users are friends.
    pub fn are_friends(&self, a: u64, b: u64) -> bool {
        self.get_state(a, b) == Some(FriendState::Accepted)
    }

    /// Build a summary of a user's social state.
    pub fn summary(&self, user_id: u64, block_list: &BlockList) -> FriendSummary {
        let total_friends = self.get_friends(user_id).len() as u32;
        let pending_requests = self.get_pending_requests(user_id).len() as u32;
        let blocked_users = block_list.blocked_by(user_id).len() as u32;
        FriendSummary {
            user_id,
            total_friends,
            pending_requests,
            blocked_users,
        }
    }

    fn get_state(&self, from: u64, to: u64) -> Option<FriendState> {
        self.friendships.get(&from)?.get(&to).cloned()
    }

    fn set_state(&mut self, from: u64, to: u64, state: FriendState) {
        self.friendships.entry(from).or_default().insert(to, state);
    }

    fn remove_state(&mut self, from: u64, to: u64) {
        if let Some(map) = self.friendships.get_mut(&from) {
            map.remove(&to);
            if map.is_empty() {
                self.friendships.remove(&from);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_block_list() -> BlockList {
        BlockList::new()
    }

    #[test]
    fn send_and_accept_request() {
        let bl = empty_block_list();
        let mut fm = FriendManager::new();
        fm.send_request(1, 2, &bl).unwrap();
        assert!(!fm.are_friends(1, 2));
        assert_eq!(fm.get_pending_requests(2), vec![1]);

        fm.accept_request(1, 2).unwrap();
        assert!(fm.are_friends(1, 2));
        assert!(fm.are_friends(2, 1));
    }

    #[test]
    fn send_and_reject_request() {
        let bl = empty_block_list();
        let mut fm = FriendManager::new();
        fm.send_request(1, 2, &bl).unwrap();
        fm.reject_request(1, 2).unwrap();
        assert!(!fm.are_friends(1, 2));
        assert!(fm.get_pending_requests(2).is_empty());
    }

    #[test]
    fn reject_allows_re_request() {
        let bl = empty_block_list();
        let mut fm = FriendManager::new();
        fm.send_request(1, 2, &bl).unwrap();
        fm.reject_request(1, 2).unwrap();
        // Should be able to send again after rejection
        fm.send_request(1, 2, &bl).unwrap();
        assert_eq!(fm.get_pending_requests(2), vec![1]);
    }

    #[test]
    fn unfriend() {
        let bl = empty_block_list();
        let mut fm = FriendManager::new();
        fm.send_request(1, 2, &bl).unwrap();
        fm.accept_request(1, 2).unwrap();
        fm.unfriend(1, 2).unwrap();
        assert!(!fm.are_friends(1, 2));
        assert!(!fm.are_friends(2, 1));
    }

    #[test]
    fn unfriend_not_friends_errors() {
        let mut fm = FriendManager::new();
        assert_eq!(fm.unfriend(1, 2), Err(SocialError::NotFriends));
    }

    #[test]
    fn send_to_self_errors() {
        let bl = empty_block_list();
        let mut fm = FriendManager::new();
        assert_eq!(fm.send_request(1, 1, &bl), Err(SocialError::SelfAction));
    }

    #[test]
    fn send_when_blocked_errors() {
        let mut bl = BlockList::new();
        bl.block(2, 1).unwrap();
        let mut fm = FriendManager::new();
        assert_eq!(fm.send_request(1, 2, &bl), Err(SocialError::UserBlocked));
    }

    #[test]
    fn send_duplicate_pending_errors() {
        let bl = empty_block_list();
        let mut fm = FriendManager::new();
        fm.send_request(1, 2, &bl).unwrap();
        assert_eq!(fm.send_request(1, 2, &bl), Err(SocialError::AlreadyPending));
    }

    #[test]
    fn send_when_already_friends_errors() {
        let bl = empty_block_list();
        let mut fm = FriendManager::new();
        fm.send_request(1, 2, &bl).unwrap();
        fm.accept_request(1, 2).unwrap();
        assert_eq!(fm.send_request(1, 2, &bl), Err(SocialError::AlreadyFriends));
    }

    #[test]
    fn accept_nonexistent_request_errors() {
        let mut fm = FriendManager::new();
        assert_eq!(fm.accept_request(1, 2), Err(SocialError::RequestNotFound));
    }

    #[test]
    fn reject_nonexistent_request_errors() {
        let mut fm = FriendManager::new();
        assert_eq!(fm.reject_request(1, 2), Err(SocialError::RequestNotFound));
    }

    #[test]
    fn get_friends_multiple() {
        let bl = empty_block_list();
        let mut fm = FriendManager::new();
        fm.send_request(1, 2, &bl).unwrap();
        fm.accept_request(1, 2).unwrap();
        fm.send_request(1, 3, &bl).unwrap();
        fm.accept_request(1, 3).unwrap();

        let mut friends = fm.get_friends(1);
        friends.sort();
        assert_eq!(friends, vec![2, 3]);
    }

    #[test]
    fn remove_friendship_clears_both_directions() {
        let bl = empty_block_list();
        let mut fm = FriendManager::new();
        fm.send_request(1, 2, &bl).unwrap();
        fm.accept_request(1, 2).unwrap();
        fm.remove_friendship(1, 2);
        assert!(!fm.are_friends(1, 2));
        assert!(!fm.are_friends(2, 1));
    }

    #[test]
    fn summary_counts() {
        let bl = empty_block_list();
        let mut fm = FriendManager::new();
        fm.send_request(10, 1, &bl).unwrap();
        fm.send_request(20, 1, &bl).unwrap();
        fm.accept_request(10, 1).unwrap();
        // User 1: 1 friend (10), 1 pending (from 20)
        let s = fm.summary(1, &bl);
        assert_eq!(s.total_friends, 1);
        assert_eq!(s.pending_requests, 1);
        assert_eq!(s.blocked_users, 0);
    }

    #[test]
    fn reverse_pending_detected() {
        let bl = empty_block_list();
        let mut fm = FriendManager::new();
        fm.send_request(1, 2, &bl).unwrap();
        // User 2 tries to also send a request to user 1 - should detect existing pending
        assert_eq!(fm.send_request(2, 1, &bl), Err(SocialError::AlreadyPending));
    }
}
