use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelId(pub u64);

#[derive(Debug, Clone, Copy)]
pub enum ChannelKind {
    Proximity,
    Private,
    World,
}

#[derive(Debug, Clone)]
pub struct VoiceZone {
    pub zone_id: u64,
    pub center_player_id: u64,
    pub radius_m: f32,
    pub listeners: HashSet<u64>,
}

#[derive(Debug, Clone)]
pub struct ChannelConfig {
    pub id: ChannelId,
    pub kind: ChannelKind,
    pub world_id: u64,
}

#[derive(Debug, Clone)]
pub enum ZoneEvent {
    Entered { player_id: u64, channel_id: ChannelId },
    Left { player_id: u64, channel_id: ChannelId },
}

#[derive(Debug, Clone)]
pub struct RoutingRequest {
    pub player_id: u64,
    pub source_channel: ChannelId,
    pub target_player_id: u64,
    pub target_zone: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
pub enum RoutingPolicy {
    Allow,
    Drop,
    Defer,
}

#[derive(Debug, Default)]
pub struct VoiceChannelManager {
    pub channels: HashMap<ChannelId, ChannelConfig>,
    pub proximity_zones: HashMap<u64, VoiceZone>,
    pub memberships: HashMap<u64, HashSet<ChannelId>>,
    pub next_channel: u64,
}

impl VoiceChannelManager {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            proximity_zones: HashMap::new(),
            memberships: HashMap::new(),
            next_channel: 1,
        }
    }

    pub fn open_channel(&mut self, kind: ChannelKind, world_id: u64) -> ChannelId {
        let id = ChannelId(self.next_channel);
        self.next_channel += 1;
        self.channels.insert(
            id,
            ChannelConfig {
                id,
                kind,
                world_id,
            },
        );
        id
    }

    pub fn add_player(&mut self, player_id: u64, channel: ChannelId) {
        let entry = self.memberships.entry(player_id).or_default();
        entry.insert(channel);
    }

    pub fn remove_player(&mut self, player_id: u64, channel: ChannelId) {
        if let Some(ch) = self.memberships.get_mut(&player_id) {
            ch.remove(&channel);
        }
    }

    pub fn route(&self, request: &RoutingRequest) -> RoutingPolicy {
        match self.channels.get(&request.source_channel) {
            None => RoutingPolicy::Drop,
            Some(cfg) => match cfg.kind {
                ChannelKind::Private => {
                    if self
                        .memberships
                        .get(&request.target_player_id)
                        .is_some_and(|channels| channels.contains(&request.source_channel))
                    {
                        RoutingPolicy::Allow
                    } else {
                        RoutingPolicy::Drop
                    }
                }
                ChannelKind::Proximity => {
                    if request.target_zone.is_some() {
                        RoutingPolicy::Allow
                    } else {
                        RoutingPolicy::Defer
                    }
                }
                ChannelKind::World => RoutingPolicy::Allow,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_channel_requires_membership() {
        let mut manager = VoiceChannelManager::new();
        let c = manager.open_channel(ChannelKind::Private, 1);
        manager.add_player(10, c);
        let req = RoutingRequest {
            player_id: 1,
            source_channel: c,
            target_player_id: 10,
            target_zone: None,
        };
        assert!(matches!(manager.route(&req), RoutingPolicy::Allow));
        let deny = RoutingRequest {
            player_id: 1,
            source_channel: c,
            target_player_id: 11,
            target_zone: None,
        };
        assert!(matches!(manager.route(&deny), RoutingPolicy::Drop));
    }

    #[test]
    fn proximity_without_zone_is_deferred() {
        let mut manager = VoiceChannelManager::new();
        let c = manager.open_channel(ChannelKind::Proximity, 1);
        let req = RoutingRequest {
            player_id: 1,
            source_channel: c,
            target_player_id: 2,
            target_zone: None,
        };
        assert!(matches!(manager.route(&req), RoutingPolicy::Defer));
    }
}
