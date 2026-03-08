#[derive(Debug, Clone, Copy)]
pub enum NatMode {
    Stun,
    Turn,
    Direct,
}

#[derive(Debug, Clone)]
pub struct RelayProfile {
    pub service_name: String,
    pub tls_terminated: bool,
    pub nat_mode: NatMode,
}

#[derive(Debug)]
pub struct RelayRegion {
    pub region_code: String,
    pub stun_address: String,
    pub turn_address: String,
}

#[derive(Debug)]
pub struct RelaySession {
    pub call_id: String,
    pub profile: RelayProfile,
    pub relay_region: RelayRegion,
}
