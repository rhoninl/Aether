//! Social and chat service for Aether: friends, groups, presence, chat, and blocking.

pub mod blocking;
pub mod chat;
pub mod error;
pub mod friend;
pub mod group;
pub mod presence;
pub mod service;
pub mod sharding;

pub use blocking::BlockList;
pub use chat::{ChatChannel, ChatManager, ChatMessage, ChatType, MessageKind};
pub use error::{SocialError, SocialResult};
pub use friend::{FriendManager, FriendRequest, FriendState, FriendStatus, FriendSummary};
pub use group::{Group, GroupConfig, GroupInvite, GroupManager, GroupStatus};
pub use presence::{
    InWorldLocation, PresenceKind, PresenceState, PresenceTracker, PresenceVisibility,
};
pub use service::SocialService;
pub use sharding::ShardMapPolicy;
