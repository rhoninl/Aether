//! Chat system: channels, message sending, and history retrieval.

use std::collections::HashMap;

use crate::blocking::BlockList;
use crate::error::{SocialError, SocialResult};

/// The type of a chat channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatType {
    DirectMessage,
    Group,
    World,
    SpatialVoice,
}

/// The payload kind of a chat message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageKind {
    Text(String),
    SystemAnnouncement(String),
    Emote(String),
    Command(String),
}

/// A chat channel with members.
#[derive(Debug, Clone)]
pub struct ChatChannel {
    pub channel_id: String,
    pub kind: ChatType,
    pub members: Vec<u64>,
}

/// A single chat message.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatMessage {
    pub message_id: String,
    pub from_user: u64,
    pub channel_id: String,
    pub kind: MessageKind,
    pub server_ts_ms: u64,
}

/// Manages chat channels and message delivery.
#[derive(Debug, Default)]
pub struct ChatManager {
    channels: HashMap<String, ChatChannel>,
    messages: HashMap<String, Vec<ChatMessage>>,
    next_channel_id: u64,
    next_msg_id: u64,
    /// Simulated server timestamp counter (monotonically increasing).
    current_ts_ms: u64,
}

impl ChatManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new chat channel with the given type and initial members.
    pub fn create_channel(&mut self, kind: ChatType, members: Vec<u64>) -> String {
        self.next_channel_id += 1;
        let channel_id = format!("chan-{}", self.next_channel_id);
        let channel = ChatChannel {
            channel_id: channel_id.clone(),
            kind,
            members,
        };
        self.channels.insert(channel_id.clone(), channel);
        self.messages.insert(channel_id.clone(), Vec::new());
        channel_id
    }

    /// Send a message to a channel. The sender must be a member and must not
    /// be blocked by any channel member.
    pub fn send_message(
        &mut self,
        channel_id: &str,
        from_user: u64,
        kind: MessageKind,
        block_list: &BlockList,
    ) -> SocialResult<ChatMessage> {
        let channel = self
            .channels
            .get(channel_id)
            .ok_or(SocialError::ChannelNotFound)?;
        if !channel.members.contains(&from_user) {
            return Err(SocialError::NotChannelMember);
        }
        // Check if sender is blocked by any other member
        for &member in &channel.members {
            if member != from_user && block_list.is_blocked_either(from_user, member) {
                return Err(SocialError::UserBlocked);
            }
        }
        self.next_msg_id += 1;
        self.current_ts_ms += 1;
        let msg = ChatMessage {
            message_id: format!("msg-{}", self.next_msg_id),
            from_user,
            channel_id: channel_id.to_string(),
            kind,
            server_ts_ms: self.current_ts_ms,
        };
        self.messages.get_mut(channel_id).unwrap().push(msg.clone());
        Ok(msg)
    }

    /// Retrieve the last `limit` messages from a channel.
    pub fn get_messages(&self, channel_id: &str, limit: usize) -> SocialResult<Vec<&ChatMessage>> {
        let msgs = self
            .messages
            .get(channel_id)
            .ok_or(SocialError::ChannelNotFound)?;
        let start = msgs.len().saturating_sub(limit);
        Ok(msgs[start..].iter().collect())
    }

    /// Get a reference to a channel by ID.
    pub fn get_channel(&self, channel_id: &str) -> Option<&ChatChannel> {
        self.channels.get(channel_id)
    }

    /// Add a member to a channel.
    pub fn add_member(&mut self, channel_id: &str, user_id: u64) -> SocialResult<()> {
        let channel = self
            .channels
            .get_mut(channel_id)
            .ok_or(SocialError::ChannelNotFound)?;
        if channel.members.contains(&user_id) {
            return Ok(()); // idempotent
        }
        channel.members.push(user_id);
        Ok(())
    }

    /// Remove a member from a channel.
    pub fn remove_member(&mut self, channel_id: &str, user_id: u64) -> SocialResult<()> {
        let channel = self
            .channels
            .get_mut(channel_id)
            .ok_or(SocialError::ChannelNotFound)?;
        if !channel.members.contains(&user_id) {
            return Err(SocialError::NotChannelMember);
        }
        channel.members.retain(|&m| m != user_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_block_list() -> BlockList {
        BlockList::new()
    }

    #[test]
    fn create_channel_and_send_message() {
        let bl = empty_block_list();
        let mut cm = ChatManager::new();
        let cid = cm.create_channel(ChatType::Group, vec![1, 2]);
        let msg = cm
            .send_message(&cid, 1, MessageKind::Text("hello".into()), &bl)
            .unwrap();
        assert_eq!(msg.from_user, 1);
        assert_eq!(msg.channel_id, cid);
    }

    #[test]
    fn get_messages_with_limit() {
        let bl = empty_block_list();
        let mut cm = ChatManager::new();
        let cid = cm.create_channel(ChatType::Group, vec![1, 2]);
        for i in 0..5 {
            cm.send_message(&cid, 1, MessageKind::Text(format!("msg-{i}")), &bl)
                .unwrap();
        }
        let msgs = cm.get_messages(&cid, 3).unwrap();
        assert_eq!(msgs.len(), 3);
        // Should be the last 3 messages
        assert_eq!(msgs[0].kind, MessageKind::Text("msg-2".into()));
        assert_eq!(msgs[2].kind, MessageKind::Text("msg-4".into()));
    }

    #[test]
    fn get_messages_limit_exceeds_count() {
        let bl = empty_block_list();
        let mut cm = ChatManager::new();
        let cid = cm.create_channel(ChatType::Group, vec![1]);
        cm.send_message(&cid, 1, MessageKind::Text("a".into()), &bl)
            .unwrap();
        let msgs = cm.get_messages(&cid, 100).unwrap();
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn non_member_cannot_send() {
        let bl = empty_block_list();
        let mut cm = ChatManager::new();
        let cid = cm.create_channel(ChatType::Group, vec![1, 2]);
        assert_eq!(
            cm.send_message(&cid, 99, MessageKind::Text("hi".into()), &bl),
            Err(SocialError::NotChannelMember)
        );
    }

    #[test]
    fn blocked_user_cannot_send() {
        let mut bl = BlockList::new();
        bl.block(2, 1).unwrap(); // user 2 blocks user 1
        let mut cm = ChatManager::new();
        let cid = cm.create_channel(ChatType::DirectMessage, vec![1, 2]);
        assert_eq!(
            cm.send_message(&cid, 1, MessageKind::Text("hi".into()), &bl),
            Err(SocialError::UserBlocked)
        );
    }

    #[test]
    fn send_to_nonexistent_channel_errors() {
        let bl = empty_block_list();
        let mut cm = ChatManager::new();
        assert_eq!(
            cm.send_message("nope", 1, MessageKind::Text("hi".into()), &bl),
            Err(SocialError::ChannelNotFound)
        );
    }

    #[test]
    fn add_member() {
        let mut cm = ChatManager::new();
        let cid = cm.create_channel(ChatType::Group, vec![1]);
        cm.add_member(&cid, 2).unwrap();
        let ch = cm.get_channel(&cid).unwrap();
        assert!(ch.members.contains(&2));
    }

    #[test]
    fn add_member_idempotent() {
        let mut cm = ChatManager::new();
        let cid = cm.create_channel(ChatType::Group, vec![1]);
        cm.add_member(&cid, 1).unwrap(); // already a member
        let ch = cm.get_channel(&cid).unwrap();
        assert_eq!(ch.members.len(), 1); // not duplicated
    }

    #[test]
    fn remove_member() {
        let mut cm = ChatManager::new();
        let cid = cm.create_channel(ChatType::Group, vec![1, 2]);
        cm.remove_member(&cid, 2).unwrap();
        let ch = cm.get_channel(&cid).unwrap();
        assert!(!ch.members.contains(&2));
    }

    #[test]
    fn remove_non_member_errors() {
        let mut cm = ChatManager::new();
        let cid = cm.create_channel(ChatType::Group, vec![1]);
        assert_eq!(
            cm.remove_member(&cid, 99),
            Err(SocialError::NotChannelMember)
        );
    }

    #[test]
    fn get_channel_nonexistent() {
        let cm = ChatManager::new();
        assert!(cm.get_channel("nope").is_none());
    }

    #[test]
    fn get_messages_nonexistent_channel() {
        let cm = ChatManager::new();
        assert_eq!(
            cm.get_messages("nope", 10),
            Err(SocialError::ChannelNotFound)
        );
    }

    #[test]
    fn message_timestamps_increase() {
        let bl = empty_block_list();
        let mut cm = ChatManager::new();
        let cid = cm.create_channel(ChatType::Group, vec![1]);
        let m1 = cm
            .send_message(&cid, 1, MessageKind::Text("a".into()), &bl)
            .unwrap();
        let m2 = cm
            .send_message(&cid, 1, MessageKind::Text("b".into()), &bl)
            .unwrap();
        assert!(m2.server_ts_ms > m1.server_ts_ms);
    }

    #[test]
    fn dm_channel() {
        let bl = empty_block_list();
        let mut cm = ChatManager::new();
        let cid = cm.create_channel(ChatType::DirectMessage, vec![1, 2]);
        let ch = cm.get_channel(&cid).unwrap();
        assert_eq!(ch.kind, ChatType::DirectMessage);
        assert_eq!(ch.members.len(), 2);
        // Both can send
        cm.send_message(&cid, 1, MessageKind::Text("hi".into()), &bl)
            .unwrap();
        cm.send_message(&cid, 2, MessageKind::Text("hey".into()), &bl)
            .unwrap();
        let msgs = cm.get_messages(&cid, 10).unwrap();
        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn system_announcement() {
        let bl = empty_block_list();
        let mut cm = ChatManager::new();
        let cid = cm.create_channel(ChatType::World, vec![1]);
        let msg = cm
            .send_message(
                &cid,
                1,
                MessageKind::SystemAnnouncement("server restart".into()),
                &bl,
            )
            .unwrap();
        assert_eq!(
            msg.kind,
            MessageKind::SystemAnnouncement("server restart".into())
        );
    }

    #[test]
    fn add_member_nonexistent_channel_errors() {
        let mut cm = ChatManager::new();
        assert_eq!(cm.add_member("nope", 1), Err(SocialError::ChannelNotFound));
    }

    #[test]
    fn remove_member_nonexistent_channel_errors() {
        let mut cm = ChatManager::new();
        assert_eq!(
            cm.remove_member("nope", 1),
            Err(SocialError::ChannelNotFound)
        );
    }
}
