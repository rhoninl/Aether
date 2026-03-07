use crate::manifest::WorldManifest;

#[derive(Debug, Clone)]
pub enum DiscoverySort {
    FeaturedFirst,
    PlayerCountDesc,
    RecentlyUpdated,
    RegionNearest,
}

#[derive(Debug, Clone)]
pub struct MatchCriteria {
    pub search: Option<String>,
    pub categories: Vec<String>,
    pub featured_only: bool,
    pub min_players: u32,
    pub max_players: u32,
}

#[derive(Debug, Clone)]
pub struct DiscoveryFilter {
    pub criteria: MatchCriteria,
    pub sort: DiscoverySort,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug)]
pub struct DiscoveryResult {
    pub worlds: Vec<WorldManifest>,
    pub page: u32,
    pub page_size: u32,
    pub total: usize,
}

