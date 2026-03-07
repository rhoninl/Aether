#[derive(Debug, Clone)]
pub struct GroupConfig {
    pub name: String,
    pub max_members: u32,
    pub invite_only: bool,
    pub public_listing: bool,
}

#[derive(Debug, Clone)]
pub enum GroupStatus {
    Created,
    Active,
    Disbanded,
    Archived,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub group_id: String,
    pub owner_id: u64,
    pub members: Vec<u64>,
    pub config: GroupConfig,
    pub status: GroupStatus,
}

#[derive(Debug, Clone)]
pub enum GroupInvite {
    Sent { group_id: String, inviter: u64, invitee: u64 },
    Accepted { group_id: String, invitee: u64 },
    Declined { group_id: String, invitee: u64 },
}

