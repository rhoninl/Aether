//! Social and chat contracts (friends, groups, presence, chat channels).

pub mod chat;
pub mod friend;
pub mod group;
pub mod presence;
pub mod sharding;

pub use chat::{ChatChannel, ChatMessage, ChatType, MessageKind};
pub use friend::{FriendRequest, FriendState, FriendStatus, FriendSummary};
pub use group::{Group, GroupConfig, GroupInvite, GroupStatus};
pub use presence::{InWorldLocation, PresenceKind, PresenceState, PresenceVisibility};
pub use sharding::ShardMapPolicy;

