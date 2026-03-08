//! Deployment topology contracts for multi-region and autoscaling.

pub mod catalog;
pub mod components;
pub mod failover;
pub mod k8s;
pub mod manifest;
pub mod probes;
pub mod region;
pub mod scaling;
pub mod topology;

pub use catalog::{Datacenter, DeploymentTopology, EnvType, Region};
pub use components::{AssetStorage, Cache, DatabaseTopology, InfraComponent, MessageBus};
pub use failover::{DatabaseFailoverPolicy, PatroniConfig};
pub use k8s::{AutoscalePolicy, HpaProfile, WorldServerAutoscaler, WorldServerRuntime};
pub use manifest::{DeploymentConfig, PvcConfig, ResourceRequirements, WorkloadKind};
pub use probes::ProbeConfig;
pub use region::{RegionEndpoint, RegionRoutingConfig, RoutingDecision};
pub use scaling::{ScalingAction, ScalingConfig, ScalingDecision};
pub use topology::{AffinityRule, TopologyConfig};
