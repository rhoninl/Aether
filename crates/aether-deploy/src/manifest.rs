//! Kubernetes manifest generation for world-server workloads.
//!
//! Produces StatefulSet YAML for durable worlds (with PVC for WAL)
//! and Deployment YAML for stateless services.

use serde::Serialize;

use crate::probes::ProbeConfig;
use crate::scaling::ScalingConfig;
use crate::topology::TopologyConfig;

/// Determines whether the workload is stateful (needs persistent storage)
/// or stateless.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum WorkloadKind {
    StatefulSet,
    Deployment,
}

/// CPU and memory resource requests/limits for a container.
#[derive(Debug, Clone, Serialize)]
pub struct ResourceRequirements {
    pub cpu_request: String,
    pub cpu_limit: String,
    pub memory_request: String,
    pub memory_limit: String,
}

impl Default for ResourceRequirements {
    fn default() -> Self {
        Self {
            cpu_request: "250m".to_string(),
            cpu_limit: "1000m".to_string(),
            memory_request: "256Mi".to_string(),
            memory_limit: "1Gi".to_string(),
        }
    }
}

/// Persistent Volume Claim configuration for WAL storage.
#[derive(Debug, Clone, Serialize)]
pub struct PvcConfig {
    pub storage_class: String,
    pub size: String,
    pub access_mode: String,
}

impl Default for PvcConfig {
    fn default() -> Self {
        Self {
            storage_class: "standard".to_string(),
            size: "10Gi".to_string(),
            access_mode: "ReadWriteOnce".to_string(),
        }
    }
}

/// Top-level deployment configuration that drives manifest generation.
#[derive(Debug, Clone, Serialize)]
pub struct DeploymentConfig {
    pub name: String,
    pub namespace: String,
    pub replicas: u32,
    pub container_image: String,
    pub container_port: u16,
    pub resources: ResourceRequirements,
    pub scaling: ScalingConfig,
    pub probes: ProbeConfig,
    pub topology: Option<TopologyConfig>,
    pub pvc: Option<PvcConfig>,
    pub labels: Vec<(String, String)>,
}

impl DeploymentConfig {
    /// Infers the workload kind from presence of PVC config.
    pub fn workload_kind(&self) -> WorkloadKind {
        if self.pvc.is_some() {
            WorkloadKind::StatefulSet
        } else {
            WorkloadKind::Deployment
        }
    }

    /// Renders a complete K8s manifest as a YAML string.
    pub fn render_yaml(&self) -> Result<String, serde_yaml::Error> {
        let manifest = self.build_manifest();
        serde_yaml::to_string(&manifest)
    }

    fn build_manifest(&self) -> serde_yaml::Value {
        use serde_yaml::Value;

        let kind = self.workload_kind();
        let kind_str = match kind {
            WorkloadKind::StatefulSet => "StatefulSet",
            WorkloadKind::Deployment => "Deployment",
        };

        let mut labels = serde_yaml::Mapping::new();
        labels.insert(
            Value::String("app".to_string()),
            Value::String(self.name.clone()),
        );
        for (k, v) in &self.labels {
            labels.insert(Value::String(k.clone()), Value::String(v.clone()));
        }

        let container = self.build_container();
        let pod_spec = self.build_pod_spec(&container, &labels);

        let mut spec = serde_yaml::Mapping::new();
        spec.insert(
            Value::String("replicas".to_string()),
            Value::Number(self.replicas.into()),
        );
        spec.insert(
            Value::String("selector".to_string()),
            Value::Mapping({
                let mut sel = serde_yaml::Mapping::new();
                sel.insert(
                    Value::String("matchLabels".to_string()),
                    Value::Mapping(labels.clone()),
                );
                sel
            }),
        );
        spec.insert(
            Value::String("template".to_string()),
            Value::Mapping({
                let mut tmpl = serde_yaml::Mapping::new();
                tmpl.insert(
                    Value::String("metadata".to_string()),
                    Value::Mapping({
                        let mut meta = serde_yaml::Mapping::new();
                        meta.insert(
                            Value::String("labels".to_string()),
                            Value::Mapping(labels.clone()),
                        );
                        meta
                    }),
                );
                tmpl.insert(Value::String("spec".to_string()), Value::Mapping(pod_spec));
                tmpl
            }),
        );

        if let Some(pvc) = &self.pvc {
            spec.insert(
                Value::String("volumeClaimTemplates".to_string()),
                Value::Sequence(vec![self.build_pvc_template(pvc)]),
            );
        }

        let mut root = serde_yaml::Mapping::new();
        root.insert(
            Value::String("apiVersion".to_string()),
            Value::String("apps/v1".to_string()),
        );
        root.insert(
            Value::String("kind".to_string()),
            Value::String(kind_str.to_string()),
        );
        root.insert(
            Value::String("metadata".to_string()),
            Value::Mapping({
                let mut meta = serde_yaml::Mapping::new();
                meta.insert(
                    Value::String("name".to_string()),
                    Value::String(self.name.clone()),
                );
                meta.insert(
                    Value::String("namespace".to_string()),
                    Value::String(self.namespace.clone()),
                );
                meta.insert(
                    Value::String("annotations".to_string()),
                    Value::Mapping(self.build_scaling_annotations()),
                );
                meta
            }),
        );
        root.insert(Value::String("spec".to_string()), Value::Mapping(spec));

        Value::Mapping(root)
    }

    fn build_container(&self) -> serde_yaml::Value {
        use serde_yaml::Value;

        let mut container = serde_yaml::Mapping::new();
        container.insert(
            Value::String("name".to_string()),
            Value::String(self.name.clone()),
        );
        container.insert(
            Value::String("image".to_string()),
            Value::String(self.container_image.clone()),
        );
        container.insert(
            Value::String("ports".to_string()),
            Value::Sequence(vec![Value::Mapping({
                let mut port = serde_yaml::Mapping::new();
                port.insert(
                    Value::String("containerPort".to_string()),
                    Value::Number(self.container_port.into()),
                );
                port
            })]),
        );

        // Resources
        container.insert(
            Value::String("resources".to_string()),
            Value::Mapping({
                let mut res = serde_yaml::Mapping::new();
                res.insert(
                    Value::String("requests".to_string()),
                    Value::Mapping({
                        let mut req = serde_yaml::Mapping::new();
                        req.insert(
                            Value::String("cpu".to_string()),
                            Value::String(self.resources.cpu_request.clone()),
                        );
                        req.insert(
                            Value::String("memory".to_string()),
                            Value::String(self.resources.memory_request.clone()),
                        );
                        req
                    }),
                );
                res.insert(
                    Value::String("limits".to_string()),
                    Value::Mapping({
                        let mut lim = serde_yaml::Mapping::new();
                        lim.insert(
                            Value::String("cpu".to_string()),
                            Value::String(self.resources.cpu_limit.clone()),
                        );
                        lim.insert(
                            Value::String("memory".to_string()),
                            Value::String(self.resources.memory_limit.clone()),
                        );
                        lim
                    }),
                );
                res
            }),
        );

        // Probes
        container.insert(
            Value::String("livenessProbe".to_string()),
            self.probes.liveness_probe_value(),
        );
        container.insert(
            Value::String("readinessProbe".to_string()),
            self.probes.readiness_probe_value(),
        );

        // Volume mounts
        if self.pvc.is_some() {
            container.insert(
                Value::String("volumeMounts".to_string()),
                Value::Sequence(vec![Value::Mapping({
                    let mut vm = serde_yaml::Mapping::new();
                    vm.insert(
                        Value::String("name".to_string()),
                        Value::String("wal-storage".to_string()),
                    );
                    vm.insert(
                        Value::String("mountPath".to_string()),
                        Value::String("/data/wal".to_string()),
                    );
                    vm
                })]),
            );
        }

        Value::Mapping(container)
    }

    fn build_pod_spec(
        &self,
        container: &serde_yaml::Value,
        _labels: &serde_yaml::Mapping,
    ) -> serde_yaml::Mapping {
        use serde_yaml::Value;

        let mut pod_spec = serde_yaml::Mapping::new();
        pod_spec.insert(
            Value::String("containers".to_string()),
            Value::Sequence(vec![container.clone()]),
        );

        if let Some(topo) = &self.topology {
            pod_spec.insert(
                Value::String("affinity".to_string()),
                topo.affinity_value(&self.name),
            );
        }

        pod_spec
    }

    fn build_pvc_template(&self, pvc: &PvcConfig) -> serde_yaml::Value {
        use serde_yaml::Value;

        Value::Mapping({
            let mut tmpl = serde_yaml::Mapping::new();
            tmpl.insert(
                Value::String("metadata".to_string()),
                Value::Mapping({
                    let mut meta = serde_yaml::Mapping::new();
                    meta.insert(
                        Value::String("name".to_string()),
                        Value::String("wal-storage".to_string()),
                    );
                    meta
                }),
            );
            tmpl.insert(
                Value::String("spec".to_string()),
                Value::Mapping({
                    let mut spec = serde_yaml::Mapping::new();
                    spec.insert(
                        Value::String("accessModes".to_string()),
                        Value::Sequence(vec![Value::String(pvc.access_mode.clone())]),
                    );
                    spec.insert(
                        Value::String("storageClassName".to_string()),
                        Value::String(pvc.storage_class.clone()),
                    );
                    spec.insert(
                        Value::String("resources".to_string()),
                        Value::Mapping({
                            let mut res = serde_yaml::Mapping::new();
                            res.insert(
                                Value::String("requests".to_string()),
                                Value::Mapping({
                                    let mut req = serde_yaml::Mapping::new();
                                    req.insert(
                                        Value::String("storage".to_string()),
                                        Value::String(pvc.size.clone()),
                                    );
                                    req
                                }),
                            );
                            res
                        }),
                    );
                    spec
                }),
            );
            tmpl
        })
    }

    fn build_scaling_annotations(&self) -> serde_yaml::Mapping {
        use serde_yaml::Value;

        let mut annotations = serde_yaml::Mapping::new();
        annotations.insert(
            Value::String("aether.io/scaling-min-replicas".to_string()),
            Value::String(self.scaling.min_replicas.to_string()),
        );
        annotations.insert(
            Value::String("aether.io/scaling-max-replicas".to_string()),
            Value::String(self.scaling.max_replicas.to_string()),
        );
        annotations.insert(
            Value::String("aether.io/scaling-target-players-per-pod".to_string()),
            Value::String(self.scaling.target_players_per_pod.to_string()),
        );
        annotations
    }
}

/// Helper to build a minimal `DeploymentConfig` for tests or quick prototyping.
pub fn default_deployment_config(name: &str, image: &str) -> DeploymentConfig {
    DeploymentConfig {
        name: name.to_string(),
        namespace: "default".to_string(),
        replicas: 1,
        container_image: image.to_string(),
        container_port: 8080,
        resources: ResourceRequirements::default(),
        scaling: ScalingConfig::default(),
        probes: ProbeConfig::default(),
        topology: None,
        pvc: None,
        labels: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probes::ProbeConfig;
    use crate::scaling::ScalingConfig;

    fn test_config() -> DeploymentConfig {
        DeploymentConfig {
            name: "world-server".to_string(),
            namespace: "aether".to_string(),
            replicas: 3,
            container_image: "aether/world:v1.0".to_string(),
            container_port: 9090,
            resources: ResourceRequirements {
                cpu_request: "500m".to_string(),
                cpu_limit: "2000m".to_string(),
                memory_request: "512Mi".to_string(),
                memory_limit: "2Gi".to_string(),
            },
            scaling: ScalingConfig {
                min_replicas: 2,
                max_replicas: 20,
                target_players_per_pod: 50,
                scale_up_cooldown_secs: 60,
                scale_down_cooldown_secs: 300,
            },
            probes: ProbeConfig {
                liveness_path: "/healthz".to_string(),
                readiness_path: "/ready".to_string(),
                port: 9090,
                initial_delay_secs: 10,
                period_secs: 5,
                failure_threshold: 3,
            },
            topology: None,
            pvc: None,
            labels: vec![("tier".to_string(), "game".to_string())],
        }
    }

    #[test]
    fn stateless_config_produces_deployment_kind() {
        let cfg = test_config();
        assert_eq!(cfg.workload_kind(), WorkloadKind::Deployment);
    }

    #[test]
    fn stateful_config_produces_statefulset_kind() {
        let mut cfg = test_config();
        cfg.pvc = Some(PvcConfig::default());
        assert_eq!(cfg.workload_kind(), WorkloadKind::StatefulSet);
    }

    #[test]
    fn render_yaml_contains_api_version() {
        let cfg = test_config();
        let yaml = cfg.render_yaml().unwrap();
        assert!(yaml.contains("apiVersion: apps/v1"));
    }

    #[test]
    fn render_yaml_deployment_has_kind_deployment() {
        let cfg = test_config();
        let yaml = cfg.render_yaml().unwrap();
        assert!(yaml.contains("kind: Deployment"));
        assert!(!yaml.contains("kind: StatefulSet"));
    }

    #[test]
    fn render_yaml_statefulset_has_kind_statefulset() {
        let mut cfg = test_config();
        cfg.pvc = Some(PvcConfig::default());
        let yaml = cfg.render_yaml().unwrap();
        assert!(yaml.contains("kind: StatefulSet"));
        assert!(!yaml.contains("kind: Deployment"));
    }

    #[test]
    fn render_yaml_contains_container_image() {
        let cfg = test_config();
        let yaml = cfg.render_yaml().unwrap();
        assert!(yaml.contains("aether/world:v1.0"));
    }

    #[test]
    fn render_yaml_contains_replicas() {
        let cfg = test_config();
        let yaml = cfg.render_yaml().unwrap();
        assert!(yaml.contains("replicas: 3"));
    }

    #[test]
    fn render_yaml_contains_namespace() {
        let cfg = test_config();
        let yaml = cfg.render_yaml().unwrap();
        assert!(yaml.contains("namespace: aether"));
    }

    #[test]
    fn render_yaml_contains_resource_limits() {
        let cfg = test_config();
        let yaml = cfg.render_yaml().unwrap();
        assert!(yaml.contains("cpu: '2000m'") || yaml.contains("cpu: 2000m"));
        assert!(yaml.contains("memory: 2Gi"));
    }

    #[test]
    fn render_yaml_contains_liveness_probe() {
        let cfg = test_config();
        let yaml = cfg.render_yaml().unwrap();
        assert!(yaml.contains("livenessProbe"));
        assert!(yaml.contains("/healthz"));
    }

    #[test]
    fn render_yaml_contains_readiness_probe() {
        let cfg = test_config();
        let yaml = cfg.render_yaml().unwrap();
        assert!(yaml.contains("readinessProbe"));
        assert!(yaml.contains("/ready"));
    }

    #[test]
    fn render_yaml_statefulset_contains_volume_claim_templates() {
        let mut cfg = test_config();
        cfg.pvc = Some(PvcConfig {
            storage_class: "gp3".to_string(),
            size: "50Gi".to_string(),
            access_mode: "ReadWriteOnce".to_string(),
        });
        let yaml = cfg.render_yaml().unwrap();
        assert!(yaml.contains("volumeClaimTemplates"));
        assert!(yaml.contains("wal-storage"));
        assert!(yaml.contains("50Gi"));
        assert!(yaml.contains("gp3"));
    }

    #[test]
    fn render_yaml_deployment_has_no_volume_claim_templates() {
        let cfg = test_config();
        let yaml = cfg.render_yaml().unwrap();
        assert!(!yaml.contains("volumeClaimTemplates"));
    }

    #[test]
    fn render_yaml_contains_scaling_annotations() {
        let cfg = test_config();
        let yaml = cfg.render_yaml().unwrap();
        assert!(yaml.contains("aether.io/scaling-min-replicas"));
        assert!(yaml.contains("aether.io/scaling-max-replicas"));
        assert!(yaml.contains("aether.io/scaling-target-players-per-pod"));
    }

    #[test]
    fn render_yaml_contains_custom_labels() {
        let cfg = test_config();
        let yaml = cfg.render_yaml().unwrap();
        assert!(yaml.contains("tier: game"));
    }

    #[test]
    fn render_yaml_contains_container_port() {
        let cfg = test_config();
        let yaml = cfg.render_yaml().unwrap();
        assert!(yaml.contains("containerPort: 9090"));
    }

    #[test]
    fn render_yaml_statefulset_contains_volume_mounts() {
        let mut cfg = test_config();
        cfg.pvc = Some(PvcConfig::default());
        let yaml = cfg.render_yaml().unwrap();
        assert!(yaml.contains("volumeMounts"));
        assert!(yaml.contains("/data/wal"));
    }

    #[test]
    fn default_deployment_config_is_valid() {
        let cfg = default_deployment_config("test-svc", "test:latest");
        assert_eq!(cfg.name, "test-svc");
        assert_eq!(cfg.container_image, "test:latest");
        assert_eq!(cfg.workload_kind(), WorkloadKind::Deployment);
        let yaml = cfg.render_yaml().unwrap();
        assert!(!yaml.is_empty());
    }
}
