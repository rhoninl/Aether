use std::collections::BTreeMap;

/// Session lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Requested,
    Provisioning,
    Running,
    Draining,
    Retired,
    Failed(String),
}

/// A running server instance for a world.
#[derive(Debug, Clone)]
pub struct ServerInstance {
    pub world_id: String,
    pub instance_id: String,
    pub region: String,
    pub host: String,
    pub port: u16,
    pub population: u32,
    pub capacity: u32,
    pub state: SessionState,
}

impl ServerInstance {
    /// Whether this instance can accept more players.
    pub fn has_capacity(&self) -> bool {
        self.state == SessionState::Running && self.population < self.capacity
    }

    /// Remaining slots before capacity.
    pub fn available_slots(&self) -> u32 {
        if self.has_capacity() {
            self.capacity - self.population
        } else {
            0
        }
    }
}

/// Region assignment policy.
#[derive(Debug, Clone)]
pub struct RegionPolicy {
    pub preferred_regions: Vec<String>,
    pub allow_cross_region_failover: bool,
    pub latency_budget_ms: u32,
}

/// Result of a session assignment attempt.
#[derive(Debug, PartialEq, Eq)]
pub enum MatchOutcome {
    Assigned {
        world_id: String,
        instance_id: String,
        region: String,
    },
    NotFound,
    Busy,
}

/// Configuration for session manager scaling behavior.
#[derive(Debug)]
pub struct SessionManagerPolicy {
    pub max_instances_per_region: u32,
    pub scale_up_threshold: f32,
    pub scale_down_threshold: f32,
    pub instance_idle_timeout_ms: u64,
    pub region_policy: RegionPolicy,
}

/// Manages server instances for a world and assigns players.
#[derive(Debug)]
pub struct SessionManager {
    pub world_id: String,
    pub instances: BTreeMap<String, ServerInstance>,
}

impl SessionManager {
    pub fn new(world_id: impl Into<String>) -> Self {
        Self {
            world_id: world_id.into(),
            instances: BTreeMap::new(),
        }
    }

    /// Add a server instance to the pool.
    pub fn add_instance(&mut self, instance: ServerInstance) {
        self.instances
            .insert(instance.instance_id.clone(), instance);
    }

    /// Remove a server instance from the pool.
    pub fn remove_instance(&mut self, instance_id: &str) -> Option<ServerInstance> {
        self.instances.remove(instance_id)
    }

    /// Route a player to the least-loaded running instance in the given region.
    pub fn route_player(&self, region: &str) -> MatchOutcome {
        let candidate = self
            .instances
            .values()
            .filter(|i| i.region == region && i.has_capacity())
            .min_by_key(|i| i.population);

        match candidate {
            Some(instance) => MatchOutcome::Assigned {
                world_id: self.world_id.clone(),
                instance_id: instance.instance_id.clone(),
                region: instance.region.clone(),
            },
            None => {
                // Check if there are instances in this region at all
                let has_instances = self
                    .instances
                    .values()
                    .any(|i| i.region == region && i.state == SessionState::Running);
                if has_instances {
                    MatchOutcome::Busy
                } else {
                    MatchOutcome::NotFound
                }
            }
        }
    }

    /// Route a player with cross-region failover support.
    pub fn route_player_with_failover(&self, policy: &RegionPolicy) -> MatchOutcome {
        // Try preferred regions in order
        for region in &policy.preferred_regions {
            let result = self.route_player(region);
            if let MatchOutcome::Assigned { .. } = &result {
                return result;
            }
        }

        // If failover allowed, try any region
        if policy.allow_cross_region_failover {
            let candidate = self
                .instances
                .values()
                .filter(|i| i.has_capacity())
                .min_by_key(|i| i.population);

            if let Some(instance) = candidate {
                return MatchOutcome::Assigned {
                    world_id: self.world_id.clone(),
                    instance_id: instance.instance_id.clone(),
                    region: instance.region.clone(),
                };
            }
        }

        // Check if anything exists at all to distinguish Busy from NotFound
        let any_running = self
            .instances
            .values()
            .any(|i| i.state == SessionState::Running);
        if any_running {
            MatchOutcome::Busy
        } else {
            MatchOutcome::NotFound
        }
    }

    /// Get the total population across all instances.
    pub fn total_population(&self) -> u32 {
        self.instances
            .values()
            .filter(|i| i.state == SessionState::Running)
            .map(|i| i.population)
            .sum()
    }

    /// Get the number of running instances.
    pub fn running_instance_count(&self) -> usize {
        self.instances
            .values()
            .filter(|i| i.state == SessionState::Running)
            .count()
    }

    /// Increment the population counter for an instance (after assignment).
    pub fn increment_population(&mut self, instance_id: &str) -> bool {
        if let Some(instance) = self.instances.get_mut(instance_id) {
            if instance.has_capacity() {
                instance.population += 1;
                return true;
            }
        }
        false
    }

    /// Decrement the population counter for an instance (on player leave).
    pub fn decrement_population(&mut self, instance_id: &str) -> bool {
        if let Some(instance) = self.instances.get_mut(instance_id) {
            if instance.population > 0 {
                instance.population -= 1;
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
fn make_instance(
    world_id: &str,
    instance_id: &str,
    region: &str,
    population: u32,
    capacity: u32,
    state: SessionState,
) -> ServerInstance {
    ServerInstance {
        world_id: world_id.to_string(),
        instance_id: instance_id.to_string(),
        region: region.to_string(),
        host: format!("{}.host.aether.gg", instance_id),
        port: 9000,
        population,
        capacity,
        state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_to_least_loaded() {
        let mut mgr = SessionManager::new("world-1");
        mgr.add_instance(make_instance(
            "world-1",
            "inst-a",
            "us-west",
            80,
            100,
            SessionState::Running,
        ));
        mgr.add_instance(make_instance(
            "world-1",
            "inst-b",
            "us-west",
            20,
            100,
            SessionState::Running,
        ));

        let result = mgr.route_player("us-west");
        assert_eq!(
            result,
            MatchOutcome::Assigned {
                world_id: "world-1".to_string(),
                instance_id: "inst-b".to_string(),
                region: "us-west".to_string(),
            }
        );
    }

    #[test]
    fn route_no_instances_returns_not_found() {
        let mgr = SessionManager::new("world-1");
        let result = mgr.route_player("us-west");
        assert_eq!(result, MatchOutcome::NotFound);
    }

    #[test]
    fn route_all_full_returns_busy() {
        let mut mgr = SessionManager::new("world-1");
        mgr.add_instance(make_instance(
            "world-1",
            "inst-a",
            "us-west",
            100,
            100,
            SessionState::Running,
        ));

        let result = mgr.route_player("us-west");
        assert_eq!(result, MatchOutcome::Busy);
    }

    #[test]
    fn route_skips_non_running_instances() {
        let mut mgr = SessionManager::new("world-1");
        mgr.add_instance(make_instance(
            "world-1",
            "inst-a",
            "us-west",
            10,
            100,
            SessionState::Draining,
        ));
        mgr.add_instance(make_instance(
            "world-1",
            "inst-b",
            "us-west",
            50,
            100,
            SessionState::Running,
        ));

        let result = mgr.route_player("us-west");
        assert_eq!(
            result,
            MatchOutcome::Assigned {
                world_id: "world-1".to_string(),
                instance_id: "inst-b".to_string(),
                region: "us-west".to_string(),
            }
        );
    }

    #[test]
    fn route_wrong_region_returns_not_found() {
        let mut mgr = SessionManager::new("world-1");
        mgr.add_instance(make_instance(
            "world-1",
            "inst-a",
            "eu-west",
            10,
            100,
            SessionState::Running,
        ));

        let result = mgr.route_player("us-west");
        assert_eq!(result, MatchOutcome::NotFound);
    }

    #[test]
    fn failover_prefers_preferred_region() {
        let mut mgr = SessionManager::new("world-1");
        mgr.add_instance(make_instance(
            "world-1",
            "inst-a",
            "us-west",
            50,
            100,
            SessionState::Running,
        ));
        mgr.add_instance(make_instance(
            "world-1",
            "inst-b",
            "eu-west",
            10,
            100,
            SessionState::Running,
        ));

        let policy = RegionPolicy {
            preferred_regions: vec!["us-west".to_string()],
            allow_cross_region_failover: true,
            latency_budget_ms: 200,
        };

        let result = mgr.route_player_with_failover(&policy);
        assert_eq!(
            result,
            MatchOutcome::Assigned {
                world_id: "world-1".to_string(),
                instance_id: "inst-a".to_string(),
                region: "us-west".to_string(),
            }
        );
    }

    #[test]
    fn failover_falls_back_to_other_region() {
        let mut mgr = SessionManager::new("world-1");
        mgr.add_instance(make_instance(
            "world-1",
            "inst-a",
            "us-west",
            100,
            100,
            SessionState::Running,
        ));
        mgr.add_instance(make_instance(
            "world-1",
            "inst-b",
            "eu-west",
            10,
            100,
            SessionState::Running,
        ));

        let policy = RegionPolicy {
            preferred_regions: vec!["us-west".to_string()],
            allow_cross_region_failover: true,
            latency_budget_ms: 200,
        };

        let result = mgr.route_player_with_failover(&policy);
        assert_eq!(
            result,
            MatchOutcome::Assigned {
                world_id: "world-1".to_string(),
                instance_id: "inst-b".to_string(),
                region: "eu-west".to_string(),
            }
        );
    }

    #[test]
    fn failover_disabled_no_cross_region() {
        let mut mgr = SessionManager::new("world-1");
        mgr.add_instance(make_instance(
            "world-1",
            "inst-a",
            "us-west",
            100,
            100,
            SessionState::Running,
        ));
        mgr.add_instance(make_instance(
            "world-1",
            "inst-b",
            "eu-west",
            10,
            100,
            SessionState::Running,
        ));

        let policy = RegionPolicy {
            preferred_regions: vec!["us-west".to_string()],
            allow_cross_region_failover: false,
            latency_budget_ms: 200,
        };

        let result = mgr.route_player_with_failover(&policy);
        assert_eq!(result, MatchOutcome::Busy);
    }

    #[test]
    fn failover_no_instances_returns_not_found() {
        let mgr = SessionManager::new("world-1");
        let policy = RegionPolicy {
            preferred_regions: vec!["us-west".to_string()],
            allow_cross_region_failover: true,
            latency_budget_ms: 200,
        };

        let result = mgr.route_player_with_failover(&policy);
        assert_eq!(result, MatchOutcome::NotFound);
    }

    #[test]
    fn total_population() {
        let mut mgr = SessionManager::new("world-1");
        mgr.add_instance(make_instance(
            "world-1",
            "inst-a",
            "us-west",
            30,
            100,
            SessionState::Running,
        ));
        mgr.add_instance(make_instance(
            "world-1",
            "inst-b",
            "us-west",
            20,
            100,
            SessionState::Running,
        ));
        mgr.add_instance(make_instance(
            "world-1",
            "inst-c",
            "us-west",
            10,
            100,
            SessionState::Draining,
        ));

        assert_eq!(mgr.total_population(), 50); // only running
    }

    #[test]
    fn running_instance_count() {
        let mut mgr = SessionManager::new("world-1");
        mgr.add_instance(make_instance(
            "world-1",
            "inst-a",
            "us-west",
            30,
            100,
            SessionState::Running,
        ));
        mgr.add_instance(make_instance(
            "world-1",
            "inst-b",
            "us-west",
            20,
            100,
            SessionState::Retired,
        ));

        assert_eq!(mgr.running_instance_count(), 1);
    }

    #[test]
    fn increment_population() {
        let mut mgr = SessionManager::new("world-1");
        mgr.add_instance(make_instance(
            "world-1",
            "inst-a",
            "us-west",
            99,
            100,
            SessionState::Running,
        ));

        assert!(mgr.increment_population("inst-a"));
        assert!(!mgr.increment_population("inst-a")); // now at capacity
    }

    #[test]
    fn decrement_population() {
        let mut mgr = SessionManager::new("world-1");
        mgr.add_instance(make_instance(
            "world-1",
            "inst-a",
            "us-west",
            1,
            100,
            SessionState::Running,
        ));

        assert!(mgr.decrement_population("inst-a"));
        assert!(!mgr.decrement_population("inst-a")); // already at 0
    }

    #[test]
    fn increment_nonexistent_returns_false() {
        let mut mgr = SessionManager::new("world-1");
        assert!(!mgr.increment_population("ghost"));
    }

    #[test]
    fn decrement_nonexistent_returns_false() {
        let mut mgr = SessionManager::new("world-1");
        assert!(!mgr.decrement_population("ghost"));
    }

    #[test]
    fn remove_instance() {
        let mut mgr = SessionManager::new("world-1");
        mgr.add_instance(make_instance(
            "world-1",
            "inst-a",
            "us-west",
            10,
            100,
            SessionState::Running,
        ));

        let removed = mgr.remove_instance("inst-a");
        assert!(removed.is_some());
        assert_eq!(mgr.running_instance_count(), 0);
    }

    #[test]
    fn server_instance_has_capacity() {
        let running = make_instance("w", "i", "r", 50, 100, SessionState::Running);
        assert!(running.has_capacity());

        let full = make_instance("w", "i", "r", 100, 100, SessionState::Running);
        assert!(!full.has_capacity());

        let draining = make_instance("w", "i", "r", 10, 100, SessionState::Draining);
        assert!(!draining.has_capacity());
    }

    #[test]
    fn server_instance_available_slots() {
        let inst = make_instance("w", "i", "r", 60, 100, SessionState::Running);
        assert_eq!(inst.available_slots(), 40);

        let full = make_instance("w", "i", "r", 100, 100, SessionState::Running);
        assert_eq!(full.available_slots(), 0);

        let draining = make_instance("w", "i", "r", 10, 100, SessionState::Draining);
        assert_eq!(draining.available_slots(), 0);
    }
}
