//! Group management: create, invite, join, leave, disband.

use std::collections::{HashMap, HashSet};

use crate::blocking::BlockList;
use crate::error::{SocialError, SocialResult};

/// Configuration for a group.
#[derive(Debug, Clone)]
pub struct GroupConfig {
    pub name: String,
    pub max_members: u32,
    pub invite_only: bool,
    pub public_listing: bool,
}

/// Lifecycle status of a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupStatus {
    Created,
    Active,
    Disbanded,
    Archived,
}

/// A social group with owner, members, and configuration.
#[derive(Debug, Clone)]
pub struct Group {
    pub group_id: String,
    pub owner_id: u64,
    pub members: Vec<u64>,
    pub config: GroupConfig,
    pub status: GroupStatus,
}

/// Group invite action envelopes.
#[derive(Debug, Clone)]
pub enum GroupInvite {
    Sent { group_id: String, inviter: u64, invitee: u64 },
    Accepted { group_id: String, invitee: u64 },
    Declined { group_id: String, invitee: u64 },
}

/// Manages group lifecycle and membership.
#[derive(Debug, Default)]
pub struct GroupManager {
    groups: HashMap<String, Group>,
    /// group_id -> set of invited user IDs
    invites: HashMap<String, HashSet<u64>>,
    /// user_id -> set of group IDs they belong to
    user_groups: HashMap<u64, HashSet<String>>,
    next_id: u64,
}

impl GroupManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new group. The creator becomes the owner and first member.
    pub fn create_group(&mut self, owner_id: u64, config: GroupConfig) -> String {
        self.next_id += 1;
        let group_id = format!("group-{}", self.next_id);
        let group = Group {
            group_id: group_id.clone(),
            owner_id,
            members: vec![owner_id],
            config,
            status: GroupStatus::Active,
        };
        self.groups.insert(group_id.clone(), group);
        self.user_groups
            .entry(owner_id)
            .or_default()
            .insert(group_id.clone());
        group_id
    }

    /// Invite a user to a group. Only existing members can invite.
    pub fn invite_user(
        &mut self,
        group_id: &str,
        inviter: u64,
        invitee: u64,
        block_list: &BlockList,
    ) -> SocialResult<()> {
        let group = self.groups.get(group_id).ok_or(SocialError::GroupNotFound)?;
        if group.status == GroupStatus::Disbanded {
            return Err(SocialError::GroupDisbanded);
        }
        if !group.members.contains(&inviter) {
            return Err(SocialError::NotGroupMember);
        }
        if group.members.contains(&invitee) {
            return Err(SocialError::AlreadyInGroup);
        }
        if block_list.is_blocked_either(inviter, invitee) {
            return Err(SocialError::UserBlocked);
        }
        if group.members.len() as u32 >= group.config.max_members {
            return Err(SocialError::GroupFull);
        }
        self.invites
            .entry(group_id.to_string())
            .or_default()
            .insert(invitee);
        Ok(())
    }

    /// Accept an invite and join the group.
    pub fn accept_invite(&mut self, group_id: &str, user_id: u64) -> SocialResult<()> {
        let has_invite = self
            .invites
            .get(group_id)
            .map_or(false, |set| set.contains(&user_id));
        if !has_invite {
            return Err(SocialError::InviteNotFound);
        }
        self.add_member_internal(group_id, user_id)?;
        self.invites
            .get_mut(group_id)
            .map(|set| set.remove(&user_id));
        Ok(())
    }

    /// Decline an invite.
    pub fn decline_invite(&mut self, group_id: &str, user_id: u64) -> SocialResult<()> {
        let has_invite = self
            .invites
            .get(group_id)
            .map_or(false, |set| set.contains(&user_id));
        if !has_invite {
            return Err(SocialError::InviteNotFound);
        }
        self.invites
            .get_mut(group_id)
            .map(|set| set.remove(&user_id));
        Ok(())
    }

    /// Join a public (non-invite-only) group directly.
    pub fn join_group(&mut self, group_id: &str, user_id: u64) -> SocialResult<()> {
        let group = self.groups.get(group_id).ok_or(SocialError::GroupNotFound)?;
        if group.status == GroupStatus::Disbanded {
            return Err(SocialError::GroupDisbanded);
        }
        if group.config.invite_only {
            return Err(SocialError::InviteNotFound);
        }
        if group.members.contains(&user_id) {
            return Err(SocialError::AlreadyInGroup);
        }
        self.add_member_internal(group_id, user_id)
    }

    /// Leave a group. If the owner leaves, the group is disbanded.
    pub fn leave_group(&mut self, group_id: &str, user_id: u64) -> SocialResult<()> {
        let group = self.groups.get(group_id).ok_or(SocialError::GroupNotFound)?;
        if group.status == GroupStatus::Disbanded {
            return Err(SocialError::GroupDisbanded);
        }
        if !group.members.contains(&user_id) {
            return Err(SocialError::NotGroupMember);
        }
        if group.owner_id == user_id {
            return self.disband_group(group_id, user_id);
        }
        let group = self.groups.get_mut(group_id).unwrap();
        group.members.retain(|&m| m != user_id);
        self.user_groups
            .get_mut(&user_id)
            .map(|set| set.remove(group_id));
        Ok(())
    }

    /// Disband a group. Only the owner can do this.
    pub fn disband_group(&mut self, group_id: &str, requester: u64) -> SocialResult<()> {
        let group = self.groups.get(group_id).ok_or(SocialError::GroupNotFound)?;
        if group.status == GroupStatus::Disbanded {
            return Err(SocialError::GroupDisbanded);
        }
        if group.owner_id != requester {
            return Err(SocialError::NotGroupOwner);
        }
        let members: Vec<u64> = group.members.clone();
        let group = self.groups.get_mut(group_id).unwrap();
        group.status = GroupStatus::Disbanded;
        group.members.clear();
        for member in members {
            self.user_groups
                .get_mut(&member)
                .map(|set| set.remove(group_id));
        }
        self.invites.remove(group_id);
        Ok(())
    }

    /// Get a reference to a group by ID.
    pub fn get_group(&self, group_id: &str) -> Option<&Group> {
        self.groups.get(group_id)
    }

    /// Get all group IDs a user belongs to.
    pub fn get_user_groups(&self, user_id: u64) -> Vec<String> {
        self.user_groups
            .get(&user_id)
            .map_or_else(Vec::new, |set| set.iter().cloned().collect())
    }

    fn add_member_internal(&mut self, group_id: &str, user_id: u64) -> SocialResult<()> {
        let group = self.groups.get(group_id).ok_or(SocialError::GroupNotFound)?;
        if group.status == GroupStatus::Disbanded {
            return Err(SocialError::GroupDisbanded);
        }
        if group.members.contains(&user_id) {
            return Err(SocialError::AlreadyInGroup);
        }
        if group.members.len() as u32 >= group.config.max_members {
            return Err(SocialError::GroupFull);
        }
        let group = self.groups.get_mut(group_id).unwrap();
        group.members.push(user_id);
        self.user_groups
            .entry(user_id)
            .or_default()
            .insert(group_id.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(name: &str) -> GroupConfig {
        GroupConfig {
            name: name.to_string(),
            max_members: 10,
            invite_only: false,
            public_listing: true,
        }
    }

    fn empty_block_list() -> BlockList {
        BlockList::new()
    }

    fn invite_only_config(name: &str) -> GroupConfig {
        GroupConfig {
            name: name.to_string(),
            max_members: 10,
            invite_only: true,
            public_listing: false,
        }
    }

    fn small_config(name: &str, max: u32) -> GroupConfig {
        GroupConfig {
            name: name.to_string(),
            max_members: max,
            invite_only: false,
            public_listing: true,
        }
    }

    #[test]
    fn create_group() {
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Test"));
        let group = gm.get_group(&gid).unwrap();
        assert_eq!(group.owner_id, 1);
        assert_eq!(group.members, vec![1]);
        assert_eq!(group.status, GroupStatus::Active);
    }

    #[test]
    fn invite_and_accept() {
        let bl = empty_block_list();
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Test"));
        gm.invite_user(&gid, 1, 2, &bl).unwrap();
        gm.accept_invite(&gid, 2).unwrap();
        let group = gm.get_group(&gid).unwrap();
        assert!(group.members.contains(&2));
    }

    #[test]
    fn invite_and_decline() {
        let bl = empty_block_list();
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Test"));
        gm.invite_user(&gid, 1, 2, &bl).unwrap();
        gm.decline_invite(&gid, 2).unwrap();
        let group = gm.get_group(&gid).unwrap();
        assert!(!group.members.contains(&2));
    }

    #[test]
    fn join_public_group() {
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Public"));
        gm.join_group(&gid, 2).unwrap();
        let group = gm.get_group(&gid).unwrap();
        assert!(group.members.contains(&2));
    }

    #[test]
    fn join_invite_only_group_errors() {
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, invite_only_config("Private"));
        assert_eq!(gm.join_group(&gid, 2), Err(SocialError::InviteNotFound));
    }

    #[test]
    fn leave_group() {
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Test"));
        gm.join_group(&gid, 2).unwrap();
        gm.leave_group(&gid, 2).unwrap();
        let group = gm.get_group(&gid).unwrap();
        assert!(!group.members.contains(&2));
    }

    #[test]
    fn owner_leave_disbands() {
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Test"));
        gm.join_group(&gid, 2).unwrap();
        gm.leave_group(&gid, 1).unwrap();
        let group = gm.get_group(&gid).unwrap();
        assert_eq!(group.status, GroupStatus::Disbanded);
        assert!(group.members.is_empty());
    }

    #[test]
    fn disband_group() {
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Test"));
        gm.disband_group(&gid, 1).unwrap();
        let group = gm.get_group(&gid).unwrap();
        assert_eq!(group.status, GroupStatus::Disbanded);
    }

    #[test]
    fn disband_non_owner_errors() {
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Test"));
        gm.join_group(&gid, 2).unwrap();
        assert_eq!(gm.disband_group(&gid, 2), Err(SocialError::NotGroupOwner));
    }

    #[test]
    fn group_full_errors() {
        let bl = empty_block_list();
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, small_config("Small", 2));
        gm.join_group(&gid, 2).unwrap();
        assert_eq!(gm.join_group(&gid, 3), Err(SocialError::GroupFull));
        assert_eq!(
            gm.invite_user(&gid, 1, 4, &bl),
            Err(SocialError::GroupFull)
        );
    }

    #[test]
    fn blocked_user_cannot_be_invited() {
        let mut bl = BlockList::new();
        bl.block(1, 2).unwrap();
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Test"));
        assert_eq!(
            gm.invite_user(&gid, 1, 2, &bl),
            Err(SocialError::UserBlocked)
        );
    }

    #[test]
    fn invite_nonexistent_group_errors() {
        let bl = empty_block_list();
        let mut gm = GroupManager::new();
        assert_eq!(
            gm.invite_user("nope", 1, 2, &bl),
            Err(SocialError::GroupNotFound)
        );
    }

    #[test]
    fn non_member_cannot_invite() {
        let bl = empty_block_list();
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Test"));
        assert_eq!(
            gm.invite_user(&gid, 99, 2, &bl),
            Err(SocialError::NotGroupMember)
        );
    }

    #[test]
    fn already_in_group_errors() {
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Test"));
        assert_eq!(gm.join_group(&gid, 1), Err(SocialError::AlreadyInGroup));
    }

    #[test]
    fn accept_nonexistent_invite_errors() {
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Test"));
        assert_eq!(gm.accept_invite(&gid, 2), Err(SocialError::InviteNotFound));
    }

    #[test]
    fn decline_nonexistent_invite_errors() {
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Test"));
        assert_eq!(
            gm.decline_invite(&gid, 2),
            Err(SocialError::InviteNotFound)
        );
    }

    #[test]
    fn get_user_groups() {
        let mut gm = GroupManager::new();
        let g1 = gm.create_group(1, test_config("A"));
        let g2 = gm.create_group(2, test_config("B"));
        gm.join_group(&g2, 1).unwrap();
        let mut groups = gm.get_user_groups(1);
        groups.sort();
        let mut expected = vec![g1, g2];
        expected.sort();
        assert_eq!(groups, expected);
    }

    #[test]
    fn invite_already_member_errors() {
        let bl = empty_block_list();
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Test"));
        gm.join_group(&gid, 2).unwrap();
        assert_eq!(
            gm.invite_user(&gid, 1, 2, &bl),
            Err(SocialError::AlreadyInGroup)
        );
    }

    #[test]
    fn leave_nonmember_errors() {
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Test"));
        assert_eq!(gm.leave_group(&gid, 99), Err(SocialError::NotGroupMember));
    }

    #[test]
    fn disbanded_group_rejects_actions() {
        let bl = empty_block_list();
        let mut gm = GroupManager::new();
        let gid = gm.create_group(1, test_config("Test"));
        gm.disband_group(&gid, 1).unwrap();
        assert_eq!(gm.join_group(&gid, 2), Err(SocialError::GroupDisbanded));
        assert_eq!(
            gm.invite_user(&gid, 1, 2, &bl),
            Err(SocialError::GroupDisbanded)
        );
        assert_eq!(gm.leave_group(&gid, 1), Err(SocialError::GroupDisbanded));
    }

    #[test]
    fn get_nonexistent_group() {
        let gm = GroupManager::new();
        assert!(gm.get_group("nope").is_none());
    }
}
