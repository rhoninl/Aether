use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub enum SessionState {
    Requested,
    Provisioning,
    Running,
    Draining,
    Retired,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct ServerInstance {
    pub world_id: String,
    pub instance_id: String,
    pub region: String,
    pub host: String,
    pub port: u16,
    pub population: u32,
    pub state: SessionState,
}

#[derive(Debug, Clone)]
pub struct RegionPolicy {
    pub preferred_regions: Vec<String>,
    pub allow_cross_region_failover: bool,
    pub latency_budget_ms: u32,
}

#[derive(Debug)]
pub enum MatchOutcome {
    Assigned {
        world_id: String,
        instance_id: String,
        region: String,
    },
    NotFound,
    Busy,
}

#[derive(Debug, Clone)]
pub struct SessionManagerPolicy {
    pub max_instances_per_region: u32,
    pub scale_up_threshold: f32,
    pub scale_down_threshold: f32,
    pub instance_idle_timeout_ms: u64,
    pub region_policy: RegionPolicy,
}

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

    pub fn add_instance(&mut self, instance: ServerInstance) {
        self.instances
            .insert(instance.instance_id.clone(), instance);
    }

    pub fn route_player(&self, region: &str) -> MatchOutcome {
        self.instances
            .values()
            .filter(|instance| instance.region == region)
            .min_by_key(|instance| instance.population)
            .map(|instance| MatchOutcome::Assigned {
                world_id: self.world_id.clone(),
                instance_id: instance.instance_id.clone(),
                region: instance.region.clone(),
            })
            .unwrap_or(MatchOutcome::NotFound)
    }
}
