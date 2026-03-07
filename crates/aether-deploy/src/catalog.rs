#[derive(Debug, Clone)]
pub enum EnvType {
    Production,
    Staging,
    Dev,
}

#[derive(Debug, Clone)]
pub struct Region {
    pub code: String,
    pub name: String,
    pub endpoint: String,
    pub zones: u32,
}

#[derive(Debug, Clone)]
pub struct Datacenter {
    pub region_code: String,
    pub cloud: String,
    pub edge_nodes: u32,
}

#[derive(Debug, Clone)]
pub struct DeploymentTopology {
    pub regions: Vec<Region>,
    pub datacenters: Vec<Datacenter>,
    pub env: EnvType,
}

