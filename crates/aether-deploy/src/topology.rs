//! Zone-aware pod scheduling and topology spread constraints.
//!
//! Generates Kubernetes affinity and anti-affinity rules to spread
//! world-server pods across availability zones and optionally co-locate
//! with related services.

use serde::Serialize;

/// Topology key for Kubernetes zone-based scheduling.
const ZONE_TOPOLOGY_KEY: &str = "topology.kubernetes.io/zone";

/// Topology key for node-based scheduling.
const HOSTNAME_TOPOLOGY_KEY: &str = "kubernetes.io/hostname";

/// Configuration for zone-aware pod scheduling.
#[derive(Debug, Clone, Serialize)]
pub struct TopologyConfig {
    /// Spread pods across availability zones.
    pub spread_across_zones: bool,
    /// Maximum pods per zone (0 = unlimited).
    pub max_pods_per_zone: u32,
    /// Anti-affinity: avoid co-locating pods on the same node.
    pub anti_affinity_per_node: bool,
    /// Optional preferred co-location label selector.
    /// E.g., co-locate with a cache service sharing this label value.
    pub preferred_colocate_label: Option<String>,
}

impl Default for TopologyConfig {
    fn default() -> Self {
        Self {
            spread_across_zones: true,
            max_pods_per_zone: 0,
            anti_affinity_per_node: true,
            preferred_colocate_label: None,
        }
    }
}

/// A single affinity rule for display or debugging.
#[derive(Debug, Clone)]
pub struct AffinityRule {
    pub rule_type: AffinityRuleType,
    pub topology_key: String,
    pub label_selector: String,
    pub weight: Option<u32>,
}

/// Whether an affinity rule is required or preferred.
#[derive(Debug, Clone, PartialEq)]
pub enum AffinityRuleType {
    RequiredAntiAffinity,
    PreferredAntiAffinity,
    PreferredAffinity,
}

impl TopologyConfig {
    /// Returns the list of affinity rules this configuration produces.
    pub fn affinity_rules(&self, app_name: &str) -> Vec<AffinityRule> {
        let mut rules = Vec::new();

        if self.spread_across_zones {
            rules.push(AffinityRule {
                rule_type: AffinityRuleType::PreferredAntiAffinity,
                topology_key: ZONE_TOPOLOGY_KEY.to_string(),
                label_selector: format!("app={app_name}"),
                weight: Some(100),
            });
        }

        if self.anti_affinity_per_node {
            rules.push(AffinityRule {
                rule_type: AffinityRuleType::RequiredAntiAffinity,
                topology_key: HOSTNAME_TOPOLOGY_KEY.to_string(),
                label_selector: format!("app={app_name}"),
                weight: None,
            });
        }

        if let Some(label) = &self.preferred_colocate_label {
            rules.push(AffinityRule {
                rule_type: AffinityRuleType::PreferredAffinity,
                topology_key: ZONE_TOPOLOGY_KEY.to_string(),
                label_selector: format!("service={label}"),
                weight: Some(50),
            });
        }

        rules
    }

    /// Generates a serde_yaml::Value representing the K8s affinity spec.
    pub fn affinity_value(&self, app_name: &str) -> serde_yaml::Value {
        use serde_yaml::Value;

        let mut affinity = serde_yaml::Mapping::new();
        let mut pod_anti_affinity = serde_yaml::Mapping::new();

        // Zone spread: preferredDuringSchedulingIgnoredDuringExecution
        if self.spread_across_zones {
            let term = self.build_preferred_anti_affinity_term(
                app_name,
                ZONE_TOPOLOGY_KEY,
                100,
            );
            pod_anti_affinity.insert(
                Value::String("preferredDuringSchedulingIgnoredDuringExecution".to_string()),
                Value::Sequence(vec![term]),
            );
        }

        // Node anti-affinity: requiredDuringSchedulingIgnoredDuringExecution
        if self.anti_affinity_per_node {
            let term = self.build_required_anti_affinity_term(
                app_name,
                HOSTNAME_TOPOLOGY_KEY,
            );
            pod_anti_affinity.insert(
                Value::String("requiredDuringSchedulingIgnoredDuringExecution".to_string()),
                Value::Sequence(vec![term]),
            );
        }

        if !pod_anti_affinity.is_empty() {
            affinity.insert(
                Value::String("podAntiAffinity".to_string()),
                Value::Mapping(pod_anti_affinity),
            );
        }

        // Co-location preference
        if let Some(label) = &self.preferred_colocate_label {
            let mut pod_affinity = serde_yaml::Mapping::new();
            let term = self.build_preferred_affinity_term(
                label,
                ZONE_TOPOLOGY_KEY,
                50,
            );
            pod_affinity.insert(
                Value::String("preferredDuringSchedulingIgnoredDuringExecution".to_string()),
                Value::Sequence(vec![term]),
            );
            affinity.insert(
                Value::String("podAffinity".to_string()),
                Value::Mapping(pod_affinity),
            );
        }

        Value::Mapping(affinity)
    }

    fn build_label_selector(&self, key: &str, value: &str) -> serde_yaml::Value {
        use serde_yaml::Value;

        Value::Mapping({
            let mut sel = serde_yaml::Mapping::new();
            sel.insert(
                Value::String("matchExpressions".to_string()),
                Value::Sequence(vec![Value::Mapping({
                    let mut expr = serde_yaml::Mapping::new();
                    expr.insert(
                        Value::String("key".to_string()),
                        Value::String(key.to_string()),
                    );
                    expr.insert(
                        Value::String("operator".to_string()),
                        Value::String("In".to_string()),
                    );
                    expr.insert(
                        Value::String("values".to_string()),
                        Value::Sequence(vec![Value::String(value.to_string())]),
                    );
                    expr
                })]),
            );
            sel
        })
    }

    fn build_preferred_anti_affinity_term(
        &self,
        app_name: &str,
        topology_key: &str,
        weight: u32,
    ) -> serde_yaml::Value {
        use serde_yaml::Value;

        Value::Mapping({
            let mut term = serde_yaml::Mapping::new();
            term.insert(
                Value::String("weight".to_string()),
                Value::Number(weight.into()),
            );
            term.insert(
                Value::String("podAffinityTerm".to_string()),
                Value::Mapping({
                    let mut pat = serde_yaml::Mapping::new();
                    pat.insert(
                        Value::String("labelSelector".to_string()),
                        self.build_label_selector("app", app_name),
                    );
                    pat.insert(
                        Value::String("topologyKey".to_string()),
                        Value::String(topology_key.to_string()),
                    );
                    pat
                }),
            );
            term
        })
    }

    fn build_required_anti_affinity_term(
        &self,
        app_name: &str,
        topology_key: &str,
    ) -> serde_yaml::Value {
        use serde_yaml::Value;

        Value::Mapping({
            let mut term = serde_yaml::Mapping::new();
            term.insert(
                Value::String("labelSelector".to_string()),
                self.build_label_selector("app", app_name),
            );
            term.insert(
                Value::String("topologyKey".to_string()),
                Value::String(topology_key.to_string()),
            );
            term
        })
    }

    fn build_preferred_affinity_term(
        &self,
        label_value: &str,
        topology_key: &str,
        weight: u32,
    ) -> serde_yaml::Value {
        use serde_yaml::Value;

        Value::Mapping({
            let mut term = serde_yaml::Mapping::new();
            term.insert(
                Value::String("weight".to_string()),
                Value::Number(weight.into()),
            );
            term.insert(
                Value::String("podAffinityTerm".to_string()),
                Value::Mapping({
                    let mut pat = serde_yaml::Mapping::new();
                    pat.insert(
                        Value::String("labelSelector".to_string()),
                        self.build_label_selector("service", label_value),
                    );
                    pat.insert(
                        Value::String("topologyKey".to_string()),
                        Value::String(topology_key.to_string()),
                    );
                    pat
                }),
            );
            term
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_spreads_across_zones() {
        let cfg = TopologyConfig::default();
        assert!(cfg.spread_across_zones);
        assert!(cfg.anti_affinity_per_node);
    }

    #[test]
    fn affinity_rules_include_zone_spread() {
        let cfg = TopologyConfig::default();
        let rules = cfg.affinity_rules("world-server");
        assert!(rules.iter().any(|r| {
            r.rule_type == AffinityRuleType::PreferredAntiAffinity
                && r.topology_key == ZONE_TOPOLOGY_KEY
        }));
    }

    #[test]
    fn affinity_rules_include_node_anti_affinity() {
        let cfg = TopologyConfig::default();
        let rules = cfg.affinity_rules("world-server");
        assert!(rules.iter().any(|r| {
            r.rule_type == AffinityRuleType::RequiredAntiAffinity
                && r.topology_key == HOSTNAME_TOPOLOGY_KEY
        }));
    }

    #[test]
    fn affinity_rules_include_colocation_when_set() {
        let cfg = TopologyConfig {
            preferred_colocate_label: Some("cache".to_string()),
            ..TopologyConfig::default()
        };
        let rules = cfg.affinity_rules("world-server");
        assert!(rules.iter().any(|r| {
            r.rule_type == AffinityRuleType::PreferredAffinity
                && r.label_selector.contains("cache")
        }));
    }

    #[test]
    fn no_colocation_rule_when_label_is_none() {
        let cfg = TopologyConfig::default();
        let rules = cfg.affinity_rules("world-server");
        assert!(!rules
            .iter()
            .any(|r| r.rule_type == AffinityRuleType::PreferredAffinity));
    }

    #[test]
    fn affinity_value_contains_zone_topology_key() {
        let cfg = TopologyConfig::default();
        let yaml = serde_yaml::to_string(&cfg.affinity_value("ws")).unwrap();
        assert!(yaml.contains("topology.kubernetes.io/zone"));
    }

    #[test]
    fn affinity_value_contains_hostname_topology_key() {
        let cfg = TopologyConfig::default();
        let yaml = serde_yaml::to_string(&cfg.affinity_value("ws")).unwrap();
        assert!(yaml.contains("kubernetes.io/hostname"));
    }

    #[test]
    fn affinity_value_contains_pod_anti_affinity() {
        let cfg = TopologyConfig::default();
        let yaml = serde_yaml::to_string(&cfg.affinity_value("ws")).unwrap();
        assert!(yaml.contains("podAntiAffinity"));
    }

    #[test]
    fn affinity_value_contains_pod_affinity_when_colocation_set() {
        let cfg = TopologyConfig {
            preferred_colocate_label: Some("cache-layer".to_string()),
            ..TopologyConfig::default()
        };
        let yaml = serde_yaml::to_string(&cfg.affinity_value("ws")).unwrap();
        assert!(yaml.contains("podAffinity"));
        assert!(yaml.contains("cache-layer"));
    }

    #[test]
    fn no_pod_affinity_when_colocation_not_set() {
        let cfg = TopologyConfig::default();
        let yaml = serde_yaml::to_string(&cfg.affinity_value("ws")).unwrap();
        // "podAffinity:" (with colon) distinguishes from "podAntiAffinity:"
        assert!(!yaml.contains("podAffinity:"));
    }

    #[test]
    fn disabled_zone_spread_removes_preferred_anti_affinity() {
        let cfg = TopologyConfig {
            spread_across_zones: false,
            anti_affinity_per_node: false,
            ..TopologyConfig::default()
        };
        let rules = cfg.affinity_rules("ws");
        assert!(rules.is_empty());
    }

    #[test]
    fn affinity_rules_label_selector_contains_app_name() {
        let cfg = TopologyConfig::default();
        let rules = cfg.affinity_rules("my-app");
        for rule in &rules {
            assert!(rule.label_selector.contains("my-app"));
        }
    }

    #[test]
    fn affinity_value_contains_match_expressions() {
        let cfg = TopologyConfig::default();
        let yaml = serde_yaml::to_string(&cfg.affinity_value("ws")).unwrap();
        assert!(yaml.contains("matchExpressions"));
    }

    #[test]
    fn zone_spread_weight_is_100() {
        let cfg = TopologyConfig::default();
        let rules = cfg.affinity_rules("ws");
        let zone_rule = rules
            .iter()
            .find(|r| r.topology_key == ZONE_TOPOLOGY_KEY)
            .unwrap();
        assert_eq!(zone_rule.weight, Some(100));
    }
}
