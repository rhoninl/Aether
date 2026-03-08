/// Errors that can occur during social operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocialError {
    /// The target user has blocked the acting user (or vice versa).
    UserBlocked,
    /// The two users are already friends.
    AlreadyFriends,
    /// The two users are not friends.
    NotFriends,
    /// No pending friend request found between these users.
    RequestNotFound,
    /// A user attempted a social action on themselves.
    SelfAction,
    /// The specified group does not exist.
    GroupNotFound,
    /// The user is not a member of the group.
    NotGroupMember,
    /// The user is not the owner of the group.
    NotGroupOwner,
    /// The group has reached its maximum member count.
    GroupFull,
    /// The group has been disbanded and cannot accept new actions.
    GroupDisbanded,
    /// The specified chat channel does not exist.
    ChannelNotFound,
    /// The user is not a member of the chat channel.
    NotChannelMember,
    /// A friend request is already pending between these users.
    AlreadyPending,
    /// The target user is already blocked.
    AlreadyBlocked,
    /// The target user is not blocked.
    NotBlocked,
    /// The user is already a member of the group.
    AlreadyInGroup,
    /// No invite found for this user in the group.
    InviteNotFound,
    /// The user is not found in the presence tracker.
    UserNotFound,
}

impl std::fmt::Display for SocialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserBlocked => write!(f, "user is blocked"),
            Self::AlreadyFriends => write!(f, "already friends"),
            Self::NotFriends => write!(f, "not friends"),
            Self::RequestNotFound => write!(f, "friend request not found"),
            Self::SelfAction => write!(f, "cannot perform social action on self"),
            Self::GroupNotFound => write!(f, "group not found"),
            Self::NotGroupMember => write!(f, "not a group member"),
            Self::NotGroupOwner => write!(f, "not the group owner"),
            Self::GroupFull => write!(f, "group is full"),
            Self::GroupDisbanded => write!(f, "group has been disbanded"),
            Self::ChannelNotFound => write!(f, "channel not found"),
            Self::NotChannelMember => write!(f, "not a channel member"),
            Self::AlreadyPending => write!(f, "friend request already pending"),
            Self::AlreadyBlocked => write!(f, "user already blocked"),
            Self::NotBlocked => write!(f, "user not blocked"),
            Self::AlreadyInGroup => write!(f, "already in group"),
            Self::InviteNotFound => write!(f, "invite not found"),
            Self::UserNotFound => write!(f, "user not found"),
        }
    }
}

impl std::error::Error for SocialError {}

pub type SocialResult<T> = Result<T, SocialError>;
