//! Multi-region routing and failover logic.
//!
//! Given a player's region and a set of available regions with health
//! status, selects the best target region for routing.

use serde::Serialize;
use std::collections::HashMap;

/// Configuration for multi-region routing.
#[derive(Debug, Clone, Serialize)]
pub struct RegionRoutingConfig {
    /// Map of region code to region endpoint.
    pub regions: Vec<RegionEndpoint>,
    /// Latency matrix: (from_region, to_region) -> latency_ms.
    pub latency_matrix: HashMap<(String, String), u32>,
    /// Maximum acceptable latency before considering a region "too far".
    pub max_acceptable_latency_ms: u32,
}

/// A single region endpoint with health status.
#[derive(Debug, Clone, Serialize)]
pub struct RegionEndpoint {
    pub code: String,
    pub endpoint: String,
    pub healthy: bool,
    pub current_load_percent: u8,
}

/// Maximum load percentage before a region is considered overloaded.
const MAX_LOAD_PERCENT: u8 = 90;

/// Result of a routing decision.
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    pub target_region: String,
    pub target_endpoint: String,
    pub estimated_latency_ms: Option<u32>,
    pub is_failover: bool,
    pub reason: String,
}

/// Error returned when no suitable region is available.
#[derive(Debug, Clone, PartialEq)]
pub struct NoAvailableRegionError {
    pub player_region: String,
    pub message: String,
}

impl RegionRoutingConfig {
    /// Routes a player to the best available region.
    ///
    /// Priority:
    /// 1. Same region if healthy and not overloaded.
    /// 2. Lowest-latency healthy region within acceptable latency.
    /// 3. Any healthy region as last resort.
    /// 4. Error if no healthy regions exist.
    pub fn route_player(
        &self,
        player_region: &str,
    ) -> Result<RoutingDecision, NoAvailableRegionError> {
        let healthy_regions: Vec<&RegionEndpoint> = self
            .regions
            .iter()
            .filter(|r| r.healthy && r.current_load_percent < MAX_LOAD_PERCENT)
            .collect();

        if healthy_regions.is_empty() {
            return Err(NoAvailableRegionError {
                player_region: player_region.to_string(),
                message: "no healthy regions available".to_string(),
            });
        }

        // 1. Try same region
        if let Some(same) = healthy_regions.iter().find(|r| r.code == player_region) {
            return Ok(RoutingDecision {
                target_region: same.code.clone(),
                target_endpoint: same.endpoint.clone(),
                estimated_latency_ms: Some(0),
                is_failover: false,
                reason: "routed to home region".to_string(),
            });
        }

        // 2. Find lowest-latency healthy region
        let mut candidates: Vec<(&RegionEndpoint, u32)> = healthy_regions
            .iter()
            .filter_map(|r| {
                let key = (player_region.to_string(), r.code.clone());
                self.latency_matrix.get(&key).map(|&lat| (*r, lat))
            })
            .filter(|(_, lat)| *lat <= self.max_acceptable_latency_ms)
            .collect();

        candidates.sort_by_key(|(_, lat)| *lat);

        if let Some((best, latency)) = candidates.first() {
            return Ok(RoutingDecision {
                target_region: best.code.clone(),
                target_endpoint: best.endpoint.clone(),
                estimated_latency_ms: Some(*latency),
                is_failover: true,
                reason: format!("failover to closest region ({}ms)", latency),
            });
        }

        // 3. Any healthy region as last resort
        let fallback = healthy_regions.first().unwrap();
        let latency = self
            .latency_matrix
            .get(&(player_region.to_string(), fallback.code.clone()));

        Ok(RoutingDecision {
            target_region: fallback.code.clone(),
            target_endpoint: fallback.endpoint.clone(),
            estimated_latency_ms: latency.copied(),
            is_failover: true,
            reason: "fallback to any healthy region".to_string(),
        })
    }

    /// Returns all healthy region codes.
    pub fn healthy_regions(&self) -> Vec<String> {
        self.regions
            .iter()
            .filter(|r| r.healthy)
            .map(|r| r.code.clone())
            .collect()
    }

    /// Returns all overloaded region codes (load >= MAX_LOAD_PERCENT).
    pub fn overloaded_regions(&self) -> Vec<String> {
        self.regions
            .iter()
            .filter(|r| r.current_load_percent >= MAX_LOAD_PERCENT)
            .map(|r| r.code.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> RegionRoutingConfig {
        let regions = vec![
            RegionEndpoint {
                code: "us-east".to_string(),
                endpoint: "https://us-east.aether.io".to_string(),
                healthy: true,
                current_load_percent: 50,
            },
            RegionEndpoint {
                code: "us-west".to_string(),
                endpoint: "https://us-west.aether.io".to_string(),
                healthy: true,
                current_load_percent: 30,
            },
            RegionEndpoint {
                code: "eu-west".to_string(),
                endpoint: "https://eu-west.aether.io".to_string(),
                healthy: true,
                current_load_percent: 60,
            },
            RegionEndpoint {
                code: "ap-east".to_string(),
                endpoint: "https://ap-east.aether.io".to_string(),
                healthy: false,
                current_load_percent: 0,
            },
        ];

        let mut latency_matrix = HashMap::new();
        // From us-east
        latency_matrix.insert(("us-east".to_string(), "us-west".to_string()), 60);
        latency_matrix.insert(("us-east".to_string(), "eu-west".to_string()), 90);
        latency_matrix.insert(("us-east".to_string(), "ap-east".to_string()), 200);
        // From us-west
        latency_matrix.insert(("us-west".to_string(), "us-east".to_string()), 60);
        latency_matrix.insert(("us-west".to_string(), "eu-west".to_string()), 130);
        latency_matrix.insert(("us-west".to_string(), "ap-east".to_string()), 150);
        // From eu-west
        latency_matrix.insert(("eu-west".to_string(), "us-east".to_string()), 90);
        latency_matrix.insert(("eu-west".to_string(), "us-west".to_string()), 130);
        latency_matrix.insert(("eu-west".to_string(), "ap-east".to_string()), 180);
        // From ap-east
        latency_matrix.insert(("ap-east".to_string(), "us-east".to_string()), 200);
        latency_matrix.insert(("ap-east".to_string(), "us-west".to_string()), 150);
        latency_matrix.insert(("ap-east".to_string(), "eu-west".to_string()), 180);

        RegionRoutingConfig {
            regions,
            latency_matrix,
            max_acceptable_latency_ms: 150,
        }
    }

    #[test]
    fn routes_to_home_region_when_healthy() {
        let cfg = test_config();
        let decision = cfg.route_player("us-east").unwrap();
        assert_eq!(decision.target_region, "us-east");
        assert!(!decision.is_failover);
    }

    #[test]
    fn failover_to_closest_when_home_unhealthy() {
        let cfg = test_config();
        // ap-east is unhealthy, so it should failover
        let decision = cfg.route_player("ap-east").unwrap();
        assert!(decision.is_failover);
        // us-west has 150ms latency from ap-east, which is <= max 150ms
        assert_eq!(decision.target_region, "us-west");
    }

    #[test]
    fn failover_selects_lowest_latency() {
        let mut cfg = test_config();
        // Make us-east unhealthy
        cfg.regions[0].healthy = false;
        let decision = cfg.route_player("us-east").unwrap();
        assert!(decision.is_failover);
        // us-west is 60ms from us-east, eu-west is 90ms
        assert_eq!(decision.target_region, "us-west");
    }

    #[test]
    fn error_when_no_healthy_regions() {
        let mut cfg = test_config();
        for r in &mut cfg.regions {
            r.healthy = false;
        }
        let result = cfg.route_player("us-east");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.player_region, "us-east");
    }

    #[test]
    fn overloaded_region_excluded() {
        let mut cfg = test_config();
        // Make us-east overloaded
        cfg.regions[0].current_load_percent = 95;
        let decision = cfg.route_player("us-east").unwrap();
        // Should failover because us-east is overloaded
        assert!(decision.is_failover);
        assert_ne!(decision.target_region, "us-east");
    }

    #[test]
    fn healthy_regions_returns_correct_list() {
        let cfg = test_config();
        let healthy = cfg.healthy_regions();
        assert_eq!(healthy.len(), 3);
        assert!(healthy.contains(&"us-east".to_string()));
        assert!(healthy.contains(&"us-west".to_string()));
        assert!(healthy.contains(&"eu-west".to_string()));
        assert!(!healthy.contains(&"ap-east".to_string()));
    }

    #[test]
    fn overloaded_regions_returns_correct_list() {
        let mut cfg = test_config();
        cfg.regions[0].current_load_percent = 95;
        let overloaded = cfg.overloaded_regions();
        assert_eq!(overloaded.len(), 1);
        assert!(overloaded.contains(&"us-east".to_string()));
    }

    #[test]
    fn routing_decision_includes_endpoint() {
        let cfg = test_config();
        let decision = cfg.route_player("us-east").unwrap();
        assert_eq!(decision.target_endpoint, "https://us-east.aether.io");
    }

    #[test]
    fn routing_to_home_has_zero_latency() {
        let cfg = test_config();
        let decision = cfg.route_player("us-east").unwrap();
        assert_eq!(decision.estimated_latency_ms, Some(0));
    }

    #[test]
    fn failover_routing_includes_latency_estimate() {
        let cfg = test_config();
        let decision = cfg.route_player("ap-east").unwrap();
        assert!(decision.estimated_latency_ms.is_some());
        assert!(decision.estimated_latency_ms.unwrap() > 0);
    }

    #[test]
    fn unknown_player_region_falls_back() {
        let cfg = test_config();
        let decision = cfg.route_player("sa-east").unwrap();
        // No latency info for sa-east, so falls back to any healthy region
        assert!(decision.is_failover);
        assert!(decision.reason.contains("fallback"));
    }

    #[test]
    fn all_regions_overloaded_returns_error() {
        let mut cfg = test_config();
        for r in &mut cfg.regions {
            r.current_load_percent = 95;
        }
        let result = cfg.route_player("us-east");
        assert!(result.is_err());
    }

    #[test]
    fn respects_max_acceptable_latency() {
        let mut cfg = test_config();
        cfg.max_acceptable_latency_ms = 50;
        // Make us-east unhealthy -- all other regions are >50ms from us-east
        cfg.regions[0].healthy = false;
        let decision = cfg.route_player("us-east").unwrap();
        // No region within 50ms, should fallback
        assert!(decision.reason.contains("fallback"));
    }

    #[test]
    fn routing_reason_is_descriptive() {
        let cfg = test_config();
        let decision = cfg.route_player("us-east").unwrap();
        assert!(!decision.reason.is_empty());
        assert!(decision.reason.contains("home region"));
    }

    #[test]
    fn failover_reason_mentions_latency() {
        let mut cfg = test_config();
        cfg.regions[0].healthy = false;
        let decision = cfg.route_player("us-east").unwrap();
        assert!(decision.reason.contains("ms") || decision.reason.contains("closest"));
    }
}
