#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldStatus {
    Draft,
    Published,
    Deprecated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldCategory {
    Social,
    Game,
    Education,
    Commerce,
    Art,
    Music,
    PvE,
    PvP,
    Simulation,
    Creative,
    Sandbox,
    Other(String),
}

#[derive(Debug, Clone)]
pub struct WorldManifest {
    pub world_id: String,
    pub slug: String,
    pub name: String,
    pub owner_id: u64,
    pub category: WorldCategory,
    pub featured: bool,
    pub max_players: u32,
    pub region_preference: Vec<String>,
    pub status: WorldStatus,
    pub version: u16,
    pub portal: String,
}

#[derive(Debug)]
pub enum WorldManifestError {
    InvalidSlug,
    MaxPlayersZero,
    MissingName,
    InvalidVersion,
}

pub fn validate_manifest(manifest: &WorldManifest) -> Result<(), WorldManifestError> {
    if manifest.slug.trim().is_empty() || !manifest.slug.is_ascii() {
        return Err(WorldManifestError::InvalidSlug);
    }
    if manifest.name.trim().is_empty() {
        return Err(WorldManifestError::MissingName);
    }
    if manifest.max_players == 0 {
        return Err(WorldManifestError::MaxPlayersZero);
    }
    if manifest.version == 0 {
        return Err(WorldManifestError::InvalidVersion);
    }
    Ok(())
}
