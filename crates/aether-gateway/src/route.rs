#[derive(Debug)]
pub struct RouteId(pub String);

#[derive(Debug)]
pub struct RegionLatencyProfile {
    pub region_code: String,
    pub avg_rtt_ms: u64,
}

#[derive(Debug)]
pub struct GeoRoutingPolicy {
    pub nearest_region_only: bool,
    pub fallback_region: String,
    pub latency_profiles: Vec<RegionLatencyProfile>,
}

#[derive(Debug)]
pub struct RoutedRequest {
    pub request_id: String,
    pub user_id: u64,
    pub route_id: RouteId,
    pub target_region: String,
}

