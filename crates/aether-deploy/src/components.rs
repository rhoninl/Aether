#[derive(Debug, Clone)]
pub enum DatabaseTopology {
    SinglePrimary,
    CitusShard,
}

#[derive(Debug, Clone)]
pub struct Cache {
    pub technology: String,
    pub memory_gb: u16,
    pub cluster: bool,
}

#[derive(Debug, Clone)]
pub struct MessageBus {
    pub name: String,
    pub supercluster: bool,
}

#[derive(Debug, Clone)]
pub struct AssetStorage {
    pub object_store: String,
    pub encryption_enabled: bool,
    pub multi_region_replication: bool,
}

#[derive(Debug, Clone)]
pub struct InfraComponent {
    pub name: String,
    pub component_type: String,
    pub replicas: u16,
    pub enabled: bool,
}

