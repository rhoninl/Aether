//! Auto-scaling logic and HPA configuration for Aether world servers.
//!
//! Provides player-count-based scaling decisions rather than relying
//! solely on CPU utilisation, which is a poor proxy for VR world capacity.

use serde::Serialize;

/// Configuration for the auto-scaling policy.
#[derive(Debug, Clone, Serialize)]
pub struct ScalingConfig {
    pub min_replicas: u32,
    pub max_replicas: u32,
    pub target_players_per_pod: u32,
    pub scale_up_cooldown_secs: u32,
    pub scale_down_cooldown_secs: u32,
}

impl Default for ScalingConfig {
    fn default() -> Self {
        Self {
            min_replicas: 1,
            max_replicas: 10,
            target_players_per_pod: 100,
            scale_up_cooldown_secs: 60,
            scale_down_cooldown_secs: 300,
        }
    }
}

/// The action recommended by the scaling engine.
#[derive(Debug, Clone, PartialEq)]
pub enum ScalingAction {
    ScaleUp,
    ScaleDown,
    Hold,
}

/// Result of a scaling computation.
#[derive(Debug, Clone)]
pub struct ScalingDecision {
    pub action: ScalingAction,
    pub desired_replicas: u32,
    pub reason: String,
}

/// A custom metric definition for HPA configuration.
#[derive(Debug, Clone, Serialize)]
pub struct CustomMetric {
    pub name: String,
    pub target_value: u32,
    pub metric_type: MetricType,
}

/// Type of HPA metric source.
#[derive(Debug, Clone, Serialize)]
pub enum MetricType {
    Pods,
    Object,
    External,
}

/// Scale-down threshold factor: scale down when utilisation drops below
/// `target * SCALE_DOWN_THRESHOLD_FACTOR`.
const SCALE_DOWN_THRESHOLD_FACTOR: f64 = 0.5;

impl ScalingConfig {
    /// Computes the desired number of replicas given the current player count
    /// and how long since the last scaling event.
    ///
    /// - Scales up when `current_players / current_replicas > target_players_per_pod`.
    /// - Scales down when utilisation drops below 50% of target.
    /// - Respects cooldown periods and min/max bounds.
    pub fn compute_desired_replicas(
        &self,
        current_players: u32,
        current_replicas: u32,
        secs_since_last_scale: u32,
    ) -> ScalingDecision {
        if self.target_players_per_pod == 0 {
            return ScalingDecision {
                action: ScalingAction::Hold,
                desired_replicas: current_replicas,
                reason: "target_players_per_pod is zero".to_string(),
            };
        }

        let effective_replicas = current_replicas.max(1);
        let players_per_pod = current_players as f64 / effective_replicas as f64;
        let target = self.target_players_per_pod as f64;

        // Check scale-up
        if players_per_pod > target {
            if secs_since_last_scale < self.scale_up_cooldown_secs {
                return ScalingDecision {
                    action: ScalingAction::Hold,
                    desired_replicas: current_replicas,
                    reason: format!(
                        "scale-up needed but cooldown active ({secs_since_last_scale}s < {}s)",
                        self.scale_up_cooldown_secs
                    ),
                };
            }

            let raw = (current_players as f64 / target).ceil() as u32;
            let desired = raw.min(self.max_replicas);
            return ScalingDecision {
                action: ScalingAction::ScaleUp,
                desired_replicas: desired,
                reason: format!(
                    "players_per_pod={players_per_pod:.1} > target={target}, scaling to {desired}"
                ),
            };
        }

        // Check scale-down
        let scale_down_threshold = target * SCALE_DOWN_THRESHOLD_FACTOR;
        if players_per_pod < scale_down_threshold {
            if secs_since_last_scale < self.scale_down_cooldown_secs {
                return ScalingDecision {
                    action: ScalingAction::Hold,
                    desired_replicas: current_replicas,
                    reason: format!(
                        "scale-down possible but cooldown active ({secs_since_last_scale}s < {}s)",
                        self.scale_down_cooldown_secs
                    ),
                };
            }

            let raw = (current_players as f64 / target).ceil().max(1.0) as u32;
            let desired = raw.max(self.min_replicas);
            return ScalingDecision {
                action: ScalingAction::ScaleDown,
                desired_replicas: desired,
                reason: format!(
                    "players_per_pod={players_per_pod:.1} < threshold={scale_down_threshold:.1}, scaling to {desired}"
                ),
            };
        }

        ScalingDecision {
            action: ScalingAction::Hold,
            desired_replicas: current_replicas,
            reason: format!("players_per_pod={players_per_pod:.1} within target range"),
        }
    }

    /// Returns the custom metric definitions for HPA configuration.
    pub fn custom_metrics(&self) -> Vec<CustomMetric> {
        vec![
            CustomMetric {
                name: "aether_player_count".to_string(),
                target_value: self.target_players_per_pod,
                metric_type: MetricType::Pods,
            },
            CustomMetric {
                name: "aether_world_memory_usage_bytes".to_string(),
                target_value: 0, // threshold set externally
                metric_type: MetricType::Pods,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> ScalingConfig {
        ScalingConfig {
            min_replicas: 2,
            max_replicas: 20,
            target_players_per_pod: 50,
            scale_up_cooldown_secs: 60,
            scale_down_cooldown_secs: 300,
        }
    }

    #[test]
    fn scale_up_when_over_target() {
        let cfg = default_config();
        // 200 players on 2 pods = 100 per pod, target is 50 -> scale up
        let decision = cfg.compute_desired_replicas(200, 2, 120);
        assert_eq!(decision.action, ScalingAction::ScaleUp);
        assert_eq!(decision.desired_replicas, 4); // ceil(200/50) = 4
    }

    #[test]
    fn scale_up_clamped_to_max() {
        let cfg = default_config();
        // 5000 players on 5 pods -> wants ceil(5000/50)=100, clamped to 20
        let decision = cfg.compute_desired_replicas(5000, 5, 120);
        assert_eq!(decision.action, ScalingAction::ScaleUp);
        assert_eq!(decision.desired_replicas, 20);
    }

    #[test]
    fn scale_up_blocked_by_cooldown() {
        let cfg = default_config();
        // Over target but only 30s since last scale (cooldown = 60s)
        let decision = cfg.compute_desired_replicas(200, 2, 30);
        assert_eq!(decision.action, ScalingAction::Hold);
        assert_eq!(decision.desired_replicas, 2);
        assert!(decision.reason.contains("cooldown"));
    }

    #[test]
    fn scale_down_when_under_half_target() {
        let cfg = default_config();
        // 40 players on 10 pods = 4 per pod, threshold = 25 -> scale down
        let decision = cfg.compute_desired_replicas(40, 10, 600);
        assert_eq!(decision.action, ScalingAction::ScaleDown);
        // ceil(40/50) = 1, but min_replicas=2 -> 2
        assert_eq!(decision.desired_replicas, 2);
    }

    #[test]
    fn scale_down_blocked_by_cooldown() {
        let cfg = default_config();
        // Under threshold but only 100s since last scale (cooldown = 300s)
        let decision = cfg.compute_desired_replicas(40, 10, 100);
        assert_eq!(decision.action, ScalingAction::Hold);
        assert_eq!(decision.desired_replicas, 10);
    }

    #[test]
    fn scale_down_respects_min_replicas() {
        let cfg = default_config();
        // 10 players on 10 pods -> wants ceil(10/50)=1, clamped to min=2
        let decision = cfg.compute_desired_replicas(10, 10, 600);
        assert_eq!(decision.action, ScalingAction::ScaleDown);
        assert_eq!(decision.desired_replicas, 2);
    }

    #[test]
    fn hold_when_within_target_range() {
        let cfg = default_config();
        // 80 players on 2 pods = 40 per pod, target=50, threshold=25 -> hold
        let decision = cfg.compute_desired_replicas(80, 2, 600);
        assert_eq!(decision.action, ScalingAction::Hold);
        assert_eq!(decision.desired_replicas, 2);
    }

    #[test]
    fn hold_when_exactly_at_target() {
        let cfg = default_config();
        // 100 players on 2 pods = 50 per pod -> exactly at target, hold
        let decision = cfg.compute_desired_replicas(100, 2, 600);
        assert_eq!(decision.action, ScalingAction::Hold);
    }

    #[test]
    fn zero_players_scales_down_to_min() {
        let cfg = default_config();
        let decision = cfg.compute_desired_replicas(0, 5, 600);
        assert_eq!(decision.action, ScalingAction::ScaleDown);
        assert_eq!(decision.desired_replicas, 2); // min_replicas
    }

    #[test]
    fn zero_target_players_per_pod_returns_hold() {
        let mut cfg = default_config();
        cfg.target_players_per_pod = 0;
        let decision = cfg.compute_desired_replicas(100, 2, 600);
        assert_eq!(decision.action, ScalingAction::Hold);
    }

    #[test]
    fn zero_current_replicas_treats_as_one_for_calculation() {
        let cfg = default_config();
        // 200 players, 0 current replicas -> treats as 1 -> 200/1=200 > 50 -> scale up
        let decision = cfg.compute_desired_replicas(200, 0, 120);
        assert_eq!(decision.action, ScalingAction::ScaleUp);
    }

    #[test]
    fn custom_metrics_returns_player_count_metric() {
        let cfg = default_config();
        let metrics = cfg.custom_metrics();
        assert!(!metrics.is_empty());
        assert!(metrics.iter().any(|m| m.name == "aether_player_count"));
        let player_metric = metrics
            .iter()
            .find(|m| m.name == "aether_player_count")
            .unwrap();
        assert_eq!(player_metric.target_value, 50);
    }

    #[test]
    fn default_scaling_config_has_sane_values() {
        let cfg = ScalingConfig::default();
        assert!(cfg.min_replicas >= 1);
        assert!(cfg.max_replicas >= cfg.min_replicas);
        assert!(cfg.target_players_per_pod > 0);
        assert!(cfg.scale_up_cooldown_secs > 0);
        assert!(cfg.scale_down_cooldown_secs > 0);
    }

    #[test]
    fn scale_up_just_above_target() {
        let cfg = default_config();
        // 51 players on 1 pod = 51 > 50 -> scale up
        let decision = cfg.compute_desired_replicas(51, 1, 120);
        assert_eq!(decision.action, ScalingAction::ScaleUp);
        assert_eq!(decision.desired_replicas, 2); // ceil(51/50) = 2
    }

    #[test]
    fn no_scale_down_when_just_above_threshold() {
        let cfg = default_config();
        // 26 players on 1 pod = 26, threshold=25 -> hold (not below threshold)
        let decision = cfg.compute_desired_replicas(26, 1, 600);
        assert_eq!(decision.action, ScalingAction::Hold);
    }

    #[test]
    fn scale_down_at_exactly_cooldown_boundary() {
        let cfg = default_config();
        // Cooldown is 300s, time is exactly 300 -> should scale
        let decision = cfg.compute_desired_replicas(10, 10, 300);
        assert_eq!(decision.action, ScalingAction::ScaleDown);
    }

    #[test]
    fn scale_up_at_exactly_cooldown_boundary() {
        let cfg = default_config();
        // Cooldown is 60s, time is exactly 60 -> should scale
        let decision = cfg.compute_desired_replicas(200, 2, 60);
        assert_eq!(decision.action, ScalingAction::ScaleUp);
    }
}
