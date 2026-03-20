#[derive(Debug, Clone)]
pub struct WorldRuntimeManifest {
    pub world_id: String,
    pub display_name: String,
    pub gravity: f32,
    pub tick_rate_hz: u32,
    pub max_players: u32,
    pub environment_path: String,
    pub terrain_manifest: String,
    pub props_manifest: String,
    pub spawn_points: u32,
}

#[derive(Debug)]
pub enum WorldManifestError {
    MissingTerrain,
    MissingEnvironment,
    InvalidTickRate,
    MaxPlayersZero,
    GravityUnrealistic,
}

pub fn validate_runtime_manifest(
    manifest: &WorldRuntimeManifest,
) -> Result<(), WorldManifestError> {
    if manifest.terrain_manifest.trim().is_empty() {
        return Err(WorldManifestError::MissingTerrain);
    }
    if manifest.environment_path.trim().is_empty() {
        return Err(WorldManifestError::MissingEnvironment);
    }
    if manifest.tick_rate_hz == 0 || manifest.tick_rate_hz > 240 {
        return Err(WorldManifestError::InvalidTickRate);
    }
    if manifest.max_players == 0 {
        return Err(WorldManifestError::MaxPlayersZero);
    }
    if !(-100.0..=100.0).contains(&manifest.gravity) {
        return Err(WorldManifestError::GravityUnrealistic);
    }
    Ok(())
}
