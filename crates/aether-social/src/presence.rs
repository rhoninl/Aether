//! Presence tracking: online/offline/in-world status transitions.

use std::collections::HashMap;

use crate::blocking::BlockList;
use crate::error::{SocialError, SocialResult};

/// The kind of presence a user currently has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenceKind {
    Offline,
    Online,
    InWorld,
}

/// Visibility settings controlling who can see the user's presence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenceVisibility {
    Visible,
    Hidden,
    Busy,
    Away,
}

/// Location data when a user is in a world.
#[derive(Debug, Clone, PartialEq)]
pub struct InWorldLocation {
    pub world_id: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub zone: Option<String>,
}

/// The full presence state for a user.
#[derive(Debug, Clone)]
pub struct PresenceState {
    pub user_id: u64,
    pub kind: PresenceKind,
    pub visibility: PresenceVisibility,
    pub in_world: Option<InWorldLocation>,
    pub updated_ms: u64,
}

/// Tracks online/offline/in-world state for all users.
#[derive(Debug, Default)]
pub struct PresenceTracker {
    states: HashMap<u64, PresenceState>,
    /// Simulated timestamp counter.
    current_ts_ms: u64,
}

impl PresenceTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set user to online. Creates entry if not yet tracked.
    pub fn set_online(&mut self, user_id: u64) -> PresenceState {
        self.current_ts_ms += 1;
        let state = self.states.entry(user_id).or_insert_with(|| PresenceState {
            user_id,
            kind: PresenceKind::Offline,
            visibility: PresenceVisibility::Visible,
            in_world: None,
            updated_ms: 0,
        });
        state.kind = PresenceKind::Online;
        state.in_world = None;
        state.updated_ms = self.current_ts_ms;
        state.clone()
    }

    /// Set user to offline.
    pub fn set_offline(&mut self, user_id: u64) -> PresenceState {
        self.current_ts_ms += 1;
        let state = self.states.entry(user_id).or_insert_with(|| PresenceState {
            user_id,
            kind: PresenceKind::Offline,
            visibility: PresenceVisibility::Visible,
            in_world: None,
            updated_ms: 0,
        });
        state.kind = PresenceKind::Offline;
        state.in_world = None;
        state.updated_ms = self.current_ts_ms;
        state.clone()
    }

    /// Set user as in-world at the given location.
    pub fn enter_world(&mut self, user_id: u64, location: InWorldLocation) -> PresenceState {
        self.current_ts_ms += 1;
        let state = self.states.entry(user_id).or_insert_with(|| PresenceState {
            user_id,
            kind: PresenceKind::Offline,
            visibility: PresenceVisibility::Visible,
            in_world: None,
            updated_ms: 0,
        });
        state.kind = PresenceKind::InWorld;
        state.in_world = Some(location);
        state.updated_ms = self.current_ts_ms;
        state.clone()
    }

    /// Remove in-world location, reverting to Online.
    pub fn leave_world(&mut self, user_id: u64) -> PresenceState {
        self.current_ts_ms += 1;
        let state = self.states.entry(user_id).or_insert_with(|| PresenceState {
            user_id,
            kind: PresenceKind::Offline,
            visibility: PresenceVisibility::Visible,
            in_world: None,
            updated_ms: 0,
        });
        state.kind = PresenceKind::Online;
        state.in_world = None;
        state.updated_ms = self.current_ts_ms;
        state.clone()
    }

    /// Change a user's visibility setting.
    pub fn set_visibility(
        &mut self,
        user_id: u64,
        visibility: PresenceVisibility,
    ) -> SocialResult<()> {
        let state = self.states.get_mut(&user_id).ok_or(SocialError::UserNotFound)?;
        self.current_ts_ms += 1;
        state.visibility = visibility;
        state.updated_ms = self.current_ts_ms;
        Ok(())
    }

    /// Get the raw presence state for a user (no block filtering).
    pub fn get_presence(&self, user_id: u64) -> Option<&PresenceState> {
        self.states.get(&user_id)
    }

    /// Get presence as seen by a viewer. Returns None if:
    /// - User is not tracked
    /// - Viewer is blocked by the user (mutual invisibility)
    /// - User's visibility is Hidden
    pub fn get_visible_presence(
        &self,
        user_id: u64,
        viewer_id: u64,
        block_list: &BlockList,
    ) -> Option<PresenceState> {
        let state = self.states.get(&user_id)?;
        if block_list.is_blocked_either(user_id, viewer_id) {
            return None;
        }
        if state.visibility == PresenceVisibility::Hidden {
            return None;
        }
        Some(state.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_block_list() -> BlockList {
        BlockList::new()
    }

    fn sample_location() -> InWorldLocation {
        InWorldLocation {
            world_id: "world-1".to_string(),
            x: 1.0,
            y: 2.0,
            z: 3.0,
            zone: Some("spawn".to_string()),
        }
    }

    #[test]
    fn set_online() {
        let mut pt = PresenceTracker::new();
        let state = pt.set_online(1);
        assert_eq!(state.kind, PresenceKind::Online);
        assert_eq!(state.visibility, PresenceVisibility::Visible);
    }

    #[test]
    fn set_offline() {
        let mut pt = PresenceTracker::new();
        pt.set_online(1);
        let state = pt.set_offline(1);
        assert_eq!(state.kind, PresenceKind::Offline);
    }

    #[test]
    fn enter_world() {
        let mut pt = PresenceTracker::new();
        pt.set_online(1);
        let state = pt.enter_world(1, sample_location());
        assert_eq!(state.kind, PresenceKind::InWorld);
        assert!(state.in_world.is_some());
        let loc = state.in_world.unwrap();
        assert_eq!(loc.world_id, "world-1");
        assert_eq!(loc.zone, Some("spawn".to_string()));
    }

    #[test]
    fn leave_world() {
        let mut pt = PresenceTracker::new();
        pt.set_online(1);
        pt.enter_world(1, sample_location());
        let state = pt.leave_world(1);
        assert_eq!(state.kind, PresenceKind::Online);
        assert!(state.in_world.is_none());
    }

    #[test]
    fn set_visibility() {
        let mut pt = PresenceTracker::new();
        pt.set_online(1);
        pt.set_visibility(1, PresenceVisibility::Busy).unwrap();
        let state = pt.get_presence(1).unwrap();
        assert_eq!(state.visibility, PresenceVisibility::Busy);
    }

    #[test]
    fn set_visibility_unknown_user_errors() {
        let mut pt = PresenceTracker::new();
        assert_eq!(
            pt.set_visibility(1, PresenceVisibility::Busy),
            Err(SocialError::UserNotFound)
        );
    }

    #[test]
    fn get_presence() {
        let mut pt = PresenceTracker::new();
        assert!(pt.get_presence(1).is_none());
        pt.set_online(1);
        assert!(pt.get_presence(1).is_some());
    }

    #[test]
    fn blocked_user_invisible() {
        let mut bl = BlockList::new();
        bl.block(1, 2).unwrap();
        let mut pt = PresenceTracker::new();
        pt.set_online(1);
        // User 2 tries to see user 1's presence but is blocked
        assert!(pt.get_visible_presence(1, 2, &bl).is_none());
        // User 1 also can't see user 2
        pt.set_online(2);
        assert!(pt.get_visible_presence(2, 1, &bl).is_none());
    }

    #[test]
    fn hidden_user_invisible() {
        let bl = empty_block_list();
        let mut pt = PresenceTracker::new();
        pt.set_online(1);
        pt.set_visibility(1, PresenceVisibility::Hidden).unwrap();
        assert!(pt.get_visible_presence(1, 2, &bl).is_none());
    }

    #[test]
    fn visible_user_seen() {
        let bl = empty_block_list();
        let mut pt = PresenceTracker::new();
        pt.set_online(1);
        let state = pt.get_visible_presence(1, 2, &bl).unwrap();
        assert_eq!(state.kind, PresenceKind::Online);
    }

    #[test]
    fn timestamps_increase() {
        let mut pt = PresenceTracker::new();
        let s1 = pt.set_online(1);
        let s2 = pt.set_offline(1);
        assert!(s2.updated_ms > s1.updated_ms);
    }

    #[test]
    fn offline_by_default() {
        let mut pt = PresenceTracker::new();
        // set_offline on untracked user creates with Offline
        let state = pt.set_offline(1);
        assert_eq!(state.kind, PresenceKind::Offline);
    }

    #[test]
    fn enter_world_without_online_first() {
        let mut pt = PresenceTracker::new();
        // entering world directly should work
        let state = pt.enter_world(1, sample_location());
        assert_eq!(state.kind, PresenceKind::InWorld);
    }

    #[test]
    fn away_visibility_still_visible_to_others() {
        let bl = empty_block_list();
        let mut pt = PresenceTracker::new();
        pt.set_online(1);
        pt.set_visibility(1, PresenceVisibility::Away).unwrap();
        // Away is not Hidden, so still visible
        let state = pt.get_visible_presence(1, 2, &bl).unwrap();
        assert_eq!(state.visibility, PresenceVisibility::Away);
    }

    #[test]
    fn busy_visibility_still_visible_to_others() {
        let bl = empty_block_list();
        let mut pt = PresenceTracker::new();
        pt.set_online(1);
        pt.set_visibility(1, PresenceVisibility::Busy).unwrap();
        let state = pt.get_visible_presence(1, 2, &bl).unwrap();
        assert_eq!(state.visibility, PresenceVisibility::Busy);
    }
}
