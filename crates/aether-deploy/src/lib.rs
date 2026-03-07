//! Deployment topology contracts for multi-region and autoscaling.

pub mod catalog;
pub mod components;
pub mod failover;
pub mod k8s;

pub use catalog::{Datacenter, DeploymentTopology, EnvType, Region};
pub use components::{AssetStorage, Cache, DatabaseTopology, InfraComponent, MessageBus};
pub use failover::{DatabaseFailoverPolicy, PatroniConfig};
pub use k8s::{AutoscalePolicy, HpaProfile, WorldServerAutoscaler, WorldServerRuntime};

