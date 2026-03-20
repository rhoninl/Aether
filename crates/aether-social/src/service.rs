//! Social service facade that composes all managers with cross-cutting concerns.

use crate::blocking::BlockList;
use crate::chat::{ChatManager, ChatMessage, ChatType, MessageKind};
use crate::error::SocialResult;
use crate::friend::{FriendManager, FriendSummary};
use crate::group::{GroupConfig, GroupManager};
use crate::presence::{InWorldLocation, PresenceState, PresenceTracker, PresenceVisibility};

/// Top-level social service that integrates friends, groups, chat,
/// presence, and blocking with cross-cutting enforcement.
#[derive(Debug, Default)]
pub struct SocialService {
    pub block_list: BlockList,
    pub friends: FriendManager,
    pub groups: GroupManager,
    pub chat: ChatManager,
    pub presence: PresenceTracker,
}

impl SocialService {
    pub fn new() -> Self {
        Self::default()
    }

    // ── Blocking ──────────────────────────────────────────────────────

    /// Block a user. Also removes any friendship and pending requests.
    pub fn block_user(&mut self, user_id: u64, target_id: u64) -> SocialResult<()> {
        self.block_list.block(user_id, target_id)?;
        // Remove friendship in both directions
        self.friends.remove_friendship(user_id, target_id);
        Ok(())
    }

    /// Unblock a user.
    pub fn unblock_user(&mut self, user_id: u64, target_id: u64) -> SocialResult<()> {
        self.block_list.unblock(user_id, target_id)
    }

    /// Check if either user has blocked the other.
    pub fn is_blocked(&self, a: u64, b: u64) -> bool {
        self.block_list.is_blocked_either(a, b)
    }

    // ── Friends ───────────────────────────────────────────────────────

    /// Send a friend request (block-list checked).
    pub fn send_friend_request(&mut self, from: u64, to: u64) -> SocialResult<()> {
        self.friends.send_request(from, to, &self.block_list)
    }

    /// Accept a friend request.
    pub fn accept_friend_request(&mut self, requester: u64, acceptor: u64) -> SocialResult<()> {
        self.friends.accept_request(requester, acceptor)
    }

    /// Reject a friend request.
    pub fn reject_friend_request(&mut self, requester: u64, rejector: u64) -> SocialResult<()> {
        self.friends.reject_request(requester, rejector)
    }

    /// Remove friendship.
    pub fn unfriend(&mut self, a: u64, b: u64) -> SocialResult<()> {
        self.friends.unfriend(a, b)
    }

    /// Get all friends.
    pub fn get_friends(&self, user_id: u64) -> Vec<u64> {
        self.friends.get_friends(user_id)
    }

    /// Get pending requests to a user.
    pub fn get_pending_requests(&self, user_id: u64) -> Vec<u64> {
        self.friends.get_pending_requests(user_id)
    }

    /// Get friend summary.
    pub fn friend_summary(&self, user_id: u64) -> FriendSummary {
        self.friends.summary(user_id, &self.block_list)
    }

    // ── Groups ────────────────────────────────────────────────────────

    /// Create a group.
    pub fn create_group(&mut self, owner_id: u64, config: GroupConfig) -> String {
        self.groups.create_group(owner_id, config)
    }

    /// Invite a user to a group (block-list checked).
    pub fn invite_to_group(
        &mut self,
        group_id: &str,
        inviter: u64,
        invitee: u64,
    ) -> SocialResult<()> {
        self.groups
            .invite_user(group_id, inviter, invitee, &self.block_list)
    }

    /// Accept a group invite.
    pub fn accept_group_invite(&mut self, group_id: &str, user_id: u64) -> SocialResult<()> {
        self.groups.accept_invite(group_id, user_id)
    }

    /// Decline a group invite.
    pub fn decline_group_invite(&mut self, group_id: &str, user_id: u64) -> SocialResult<()> {
        self.groups.decline_invite(group_id, user_id)
    }

    /// Join a public group.
    pub fn join_group(&mut self, group_id: &str, user_id: u64) -> SocialResult<()> {
        self.groups.join_group(group_id, user_id)
    }

    /// Leave a group.
    pub fn leave_group(&mut self, group_id: &str, user_id: u64) -> SocialResult<()> {
        self.groups.leave_group(group_id, user_id)
    }

    /// Disband a group (owner only).
    pub fn disband_group(&mut self, group_id: &str, requester: u64) -> SocialResult<()> {
        self.groups.disband_group(group_id, requester)
    }

    // ── Chat ──────────────────────────────────────────────────────────

    /// Create a chat channel.
    pub fn create_chat_channel(&mut self, kind: ChatType, members: Vec<u64>) -> String {
        self.chat.create_channel(kind, members)
    }

    /// Send a chat message (block-list checked).
    pub fn send_chat_message(
        &mut self,
        channel_id: &str,
        from_user: u64,
        kind: MessageKind,
    ) -> SocialResult<ChatMessage> {
        self.chat
            .send_message(channel_id, from_user, kind, &self.block_list)
    }

    /// Get recent messages from a channel.
    pub fn get_chat_messages(
        &self,
        channel_id: &str,
        limit: usize,
    ) -> SocialResult<Vec<&ChatMessage>> {
        self.chat.get_messages(channel_id, limit)
    }

    // ── Presence ──────────────────────────────────────────────────────

    /// Set user online.
    pub fn set_online(&mut self, user_id: u64) -> PresenceState {
        self.presence.set_online(user_id)
    }

    /// Set user offline.
    pub fn set_offline(&mut self, user_id: u64) -> PresenceState {
        self.presence.set_offline(user_id)
    }

    /// Enter a world.
    pub fn enter_world(&mut self, user_id: u64, location: InWorldLocation) -> PresenceState {
        self.presence.enter_world(user_id, location)
    }

    /// Leave a world.
    pub fn leave_world(&mut self, user_id: u64) -> PresenceState {
        self.presence.leave_world(user_id)
    }

    /// Set visibility.
    pub fn set_visibility(
        &mut self,
        user_id: u64,
        visibility: PresenceVisibility,
    ) -> SocialResult<()> {
        self.presence.set_visibility(user_id, visibility)
    }

    /// Get presence as seen by a viewer (block-list filtered).
    pub fn get_visible_presence(&self, user_id: u64, viewer_id: u64) -> Option<PresenceState> {
        self.presence
            .get_visible_presence(user_id, viewer_id, &self.block_list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SocialError;

    fn make_group_config(name: &str) -> GroupConfig {
        GroupConfig {
            name: name.to_string(),
            max_members: 10,
            invite_only: false,
            public_listing: true,
        }
    }

    // ── Cross-cutting: block removes friendship ──

    #[test]
    fn block_removes_friendship() {
        let mut svc = SocialService::new();
        svc.send_friend_request(1, 2).unwrap();
        svc.accept_friend_request(1, 2).unwrap();
        assert!(svc.friends.are_friends(1, 2));

        svc.block_user(1, 2).unwrap();
        assert!(!svc.friends.are_friends(1, 2));
        assert!(svc.is_blocked(1, 2));
    }

    #[test]
    fn block_prevents_friend_request() {
        let mut svc = SocialService::new();
        svc.block_user(1, 2).unwrap();
        assert_eq!(svc.send_friend_request(2, 1), Err(SocialError::UserBlocked));
    }

    #[test]
    fn block_prevents_chat() {
        let mut svc = SocialService::new();
        let cid = svc.create_chat_channel(ChatType::DirectMessage, vec![1, 2]);
        svc.block_user(1, 2).unwrap();
        assert_eq!(
            svc.send_chat_message(&cid, 2, MessageKind::Text("hi".into())),
            Err(SocialError::UserBlocked)
        );
    }

    #[test]
    fn block_hides_presence() {
        let mut svc = SocialService::new();
        svc.set_online(1);
        svc.set_online(2);
        // Before block, visible
        assert!(svc.get_visible_presence(1, 2).is_some());

        svc.block_user(1, 2).unwrap();
        // After block, invisible
        assert!(svc.get_visible_presence(1, 2).is_none());
        assert!(svc.get_visible_presence(2, 1).is_none());
    }

    #[test]
    fn block_prevents_group_invite() {
        let mut svc = SocialService::new();
        let gid = svc.create_group(1, make_group_config("Test"));
        svc.block_user(1, 2).unwrap();
        assert_eq!(
            svc.invite_to_group(&gid, 1, 2),
            Err(SocialError::UserBlocked)
        );
    }

    // ── Unblock restores ability ──

    #[test]
    fn unblock_allows_friend_request() {
        let mut svc = SocialService::new();
        svc.block_user(1, 2).unwrap();
        svc.unblock_user(1, 2).unwrap();
        svc.send_friend_request(1, 2).unwrap();
        assert_eq!(svc.get_pending_requests(2), vec![1]);
    }

    #[test]
    fn unblock_restores_presence() {
        let mut svc = SocialService::new();
        svc.set_online(1);
        svc.block_user(1, 2).unwrap();
        assert!(svc.get_visible_presence(1, 2).is_none());
        svc.unblock_user(1, 2).unwrap();
        assert!(svc.get_visible_presence(1, 2).is_some());
    }

    // ── Full friend lifecycle ──

    #[test]
    fn full_friend_lifecycle() {
        let mut svc = SocialService::new();
        // Send, accept, unfriend
        svc.send_friend_request(1, 2).unwrap();
        svc.accept_friend_request(1, 2).unwrap();
        assert_eq!(svc.get_friends(1), vec![2]);
        svc.unfriend(1, 2).unwrap();
        assert!(svc.get_friends(1).is_empty());
    }

    #[test]
    fn friend_summary_integration() {
        let mut svc = SocialService::new();
        svc.send_friend_request(10, 1).unwrap();
        svc.accept_friend_request(10, 1).unwrap();
        svc.send_friend_request(20, 1).unwrap(); // pending
        svc.block_user(1, 30).unwrap();

        let summary = svc.friend_summary(1);
        assert_eq!(summary.total_friends, 1);
        assert_eq!(summary.pending_requests, 1);
        assert_eq!(summary.blocked_users, 1);
    }

    // ── Full group lifecycle ──

    #[test]
    fn full_group_lifecycle() {
        let mut svc = SocialService::new();
        let gid = svc.create_group(1, make_group_config("Party"));
        svc.invite_to_group(&gid, 1, 2).unwrap();
        svc.accept_group_invite(&gid, 2).unwrap();
        let group = svc.groups.get_group(&gid).unwrap();
        assert_eq!(group.members.len(), 2);

        svc.leave_group(&gid, 2).unwrap();
        let group = svc.groups.get_group(&gid).unwrap();
        assert_eq!(group.members.len(), 1);

        svc.disband_group(&gid, 1).unwrap();
        let group = svc.groups.get_group(&gid).unwrap();
        assert_eq!(group.status, crate::group::GroupStatus::Disbanded);
    }

    // ── Full chat lifecycle ──

    #[test]
    fn full_chat_lifecycle() {
        let mut svc = SocialService::new();
        let cid = svc.create_chat_channel(ChatType::Group, vec![1, 2, 3]);
        svc.send_chat_message(&cid, 1, MessageKind::Text("hello everyone".into()))
            .unwrap();
        svc.send_chat_message(&cid, 2, MessageKind::Text("hey!".into()))
            .unwrap();
        let msgs = svc.get_chat_messages(&cid, 10).unwrap();
        assert_eq!(msgs.len(), 2);
    }

    // ── Presence lifecycle ──

    #[test]
    fn full_presence_lifecycle() {
        let mut svc = SocialService::new();
        svc.set_online(1);
        let loc = InWorldLocation {
            world_id: "w-1".to_string(),
            x: 0.0,
            y: 0.0,
            z: 0.0,
            zone: None,
        };
        let state = svc.enter_world(1, loc);
        assert_eq!(state.kind, crate::presence::PresenceKind::InWorld);

        let state = svc.leave_world(1);
        assert_eq!(state.kind, crate::presence::PresenceKind::Online);

        let state = svc.set_offline(1);
        assert_eq!(state.kind, crate::presence::PresenceKind::Offline);
    }

    #[test]
    fn visibility_affects_presence_view() {
        let mut svc = SocialService::new();
        svc.set_online(1);
        svc.set_visibility(1, PresenceVisibility::Hidden).unwrap();
        assert!(svc.get_visible_presence(1, 2).is_none());

        svc.set_visibility(1, PresenceVisibility::Visible).unwrap();
        assert!(svc.get_visible_presence(1, 2).is_some());
    }

    // ── Edge cases ──

    #[test]
    fn self_block_errors() {
        let mut svc = SocialService::new();
        assert_eq!(svc.block_user(1, 1), Err(SocialError::SelfAction));
    }

    #[test]
    fn self_friend_request_errors() {
        let mut svc = SocialService::new();
        assert_eq!(svc.send_friend_request(1, 1), Err(SocialError::SelfAction));
    }
}
