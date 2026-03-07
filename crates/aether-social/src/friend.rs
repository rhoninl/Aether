#[derive(Debug, Clone)]
pub enum FriendState {
    Pending,
    Accepted,
    Blocked,
    Rejected,
}

#[derive(Debug, Clone)]
pub struct FriendStatus {
    pub user_a: u64,
    pub user_b: u64,
    pub state: FriendState,
    pub initiated_ms: u64,
}

#[derive(Debug, Clone)]
pub enum FriendRequest {
    Send { from: u64, to: u64, message: Option<String> },
    Accept { from: u64, to: u64 },
    Reject { from: u64, to: u64 },
    Block { from: u64, to: u64 },
}

#[derive(Debug)]
pub struct FriendSummary {
    pub user_id: u64,
    pub total_friends: u32,
    pub pending_requests: u32,
    pub blocked_users: u32,
}

