//! Streaming engine: orchestrates chunk loading, prefetch, LOD selection, and occlusion gating.

use std::collections::HashMap;

use super::coord::{ChunkCoord, ChunkId};
use super::eviction::{EvictionCandidate, EvictionPolicy};
use super::manifest::ChunkManifest;
use super::state::{ChunkEntry, ChunkState};

/// Default active radius (in chunk units) around the player.
pub const DEFAULT_ACTIVE_RADIUS: u32 = 3;

/// Default cache radius (in chunk units) around the player.
pub const DEFAULT_CACHE_RADIUS: u32 = 5;

/// Default maximum number of inflight (Requested + Loading) chunks.
pub const DEFAULT_MAX_INFLIGHT_REQUESTS: usize = 16;

/// Default prefetch lookahead time in seconds.
pub const DEFAULT_PREFETCH_TIME_SECS: f32 = 2.0;

/// Default LOD distance thresholds (in world units) for LOD 1, 2, 3.
pub const DEFAULT_LOD_DISTANCES: [f32; 3] = [128.0, 256.0, 512.0];

/// Configuration for the streaming engine.
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    /// Chunk size in world units.
    pub chunk_size: f32,
    /// Radius (Chebyshev) around the player to keep chunks Active.
    pub active_radius: u32,
    /// Radius (Chebyshev) around the player to keep chunks Cached.
    pub cache_radius: u32,
    /// Maximum inflight (Requested + Loading) requests.
    pub max_inflight: usize,
    /// Prefetch lookahead time in seconds.
    pub prefetch_time_secs: f32,
    /// LOD distance thresholds in world units.
    pub lod_distances: Vec<f32>,
    /// Eviction policy configuration.
    pub eviction_policy: EvictionPolicy,
    /// Whether occlusion portal gating is enabled.
    pub portal_gating_enabled: bool,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            chunk_size: super::coord::DEFAULT_CHUNK_SIZE,
            active_radius: DEFAULT_ACTIVE_RADIUS,
            cache_radius: DEFAULT_CACHE_RADIUS,
            max_inflight: DEFAULT_MAX_INFLIGHT_REQUESTS,
            prefetch_time_secs: DEFAULT_PREFETCH_TIME_SECS,
            lod_distances: DEFAULT_LOD_DISTANCES.to_vec(),
            eviction_policy: EvictionPolicy::default(),
            portal_gating_enabled: true,
        }
    }
}

/// Player state snapshot used by the streaming engine.
#[derive(Debug, Clone)]
pub struct PlayerView {
    /// Current world-space position.
    pub position: [f32; 3],
    /// Current velocity in world units per second.
    pub velocity: [f32; 3],
}

/// Events emitted by the streaming engine during a tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamingEvent {
    /// A chunk should begin loading.
    ChunkRequested { id: ChunkId, coord: ChunkCoord, lod: u8 },
    /// A chunk was activated (ready for rendering).
    ChunkActivated { id: ChunkId, coord: ChunkCoord },
    /// A chunk was deactivated (no longer rendered but still cached).
    ChunkDeactivated { id: ChunkId, coord: ChunkCoord },
    /// A chunk eviction was initiated.
    ChunkEvicting { id: ChunkId, coord: ChunkCoord },
    /// A load request was dropped because inflight limit was reached.
    RequestDropped { id: ChunkId },
}

/// The streaming engine manages chunk lifecycle based on player position.
#[derive(Debug)]
pub struct StreamingEngine {
    config: StreamingConfig,
    chunks: HashMap<ChunkId, ChunkEntry>,
    coord_to_id: HashMap<ChunkCoord, ChunkId>,
}

impl StreamingEngine {
    pub fn new(config: StreamingConfig) -> Self {
        Self {
            config,
            chunks: HashMap::new(),
            coord_to_id: HashMap::new(),
        }
    }

    /// Initialize the engine from a chunk manifest.
    pub fn load_manifest(&mut self, manifest: &ChunkManifest) {
        for chunk_ref in &manifest.chunks {
            let entry = ChunkEntry::new(chunk_ref.id, chunk_ref.coord, chunk_ref.asset_path.clone());
            self.coord_to_id.insert(chunk_ref.coord, chunk_ref.id);
            self.chunks.insert(chunk_ref.id, entry);
        }
    }

    /// Get the current config.
    pub fn config(&self) -> &StreamingConfig {
        &self.config
    }

    /// Get a chunk entry by ID.
    pub fn get_chunk(&self, id: &ChunkId) -> Option<&ChunkEntry> {
        self.chunks.get(id)
    }

    /// Get a chunk entry by coordinate.
    pub fn get_chunk_at(&self, coord: &ChunkCoord) -> Option<&ChunkEntry> {
        self.coord_to_id
            .get(coord)
            .and_then(|id| self.chunks.get(id))
    }

    /// Count chunks in a given state.
    pub fn count_in_state(&self, state: ChunkState) -> usize {
        self.chunks.values().filter(|c| c.state == state).count()
    }

    /// Count inflight chunks (Requested + Loading).
    pub fn inflight_count(&self) -> usize {
        self.chunks.values().filter(|c| c.is_inflight()).count()
    }

    /// Count in-memory chunks (Cached + Active + Evicting).
    pub fn in_memory_count(&self) -> usize {
        self.chunks.values().filter(|c| c.is_in_memory()).count()
    }

    /// Total tracked chunks.
    pub fn total_chunks(&self) -> usize {
        self.chunks.len()
    }

    /// Notify the engine that a chunk finished loading.
    pub fn notify_load_complete(
        &mut self,
        id: ChunkId,
        size_bytes: u64,
        now_ms: u64,
    ) -> Result<(), String> {
        let entry = self
            .chunks
            .get_mut(&id)
            .ok_or_else(|| format!("unknown chunk {id}"))?;

        if entry.state != ChunkState::Loading {
            return Err(format!(
                "chunk {} is in state {}, expected Loading",
                id, entry.state
            ));
        }

        entry.size_bytes = size_bytes;
        entry
            .transition(ChunkState::Cached, now_ms)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Notify the engine that a chunk load failed.
    pub fn notify_load_failed(&mut self, id: ChunkId, now_ms: u64) -> Result<(), String> {
        let entry = self
            .chunks
            .get_mut(&id)
            .ok_or_else(|| format!("unknown chunk {id}"))?;

        if entry.state != ChunkState::Loading {
            return Err(format!(
                "chunk {} is in state {}, expected Loading",
                id, entry.state
            ));
        }

        entry
            .transition(ChunkState::Unloaded, now_ms)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Notify the engine that an evicting chunk has been fully evicted.
    pub fn notify_evict_complete(&mut self, id: ChunkId, now_ms: u64) -> Result<(), String> {
        let entry = self
            .chunks
            .get_mut(&id)
            .ok_or_else(|| format!("unknown chunk {id}"))?;

        if entry.state != ChunkState::Evicting {
            return Err(format!(
                "chunk {} is in state {}, expected Evicting",
                id, entry.state
            ));
        }

        entry
            .transition(ChunkState::Unloaded, now_ms)
            .map_err(|e| e.to_string())?;
        entry.size_bytes = 0;
        Ok(())
    }

    /// Select the LOD level for a chunk based on its distance from the player.
    pub fn select_lod(&self, chunk_coord: &ChunkCoord, player_coord: &ChunkCoord) -> u8 {
        let center = chunk_coord.world_center(self.config.chunk_size);
        let player_center = player_coord.world_center(self.config.chunk_size);

        let dx = center[0] - player_center[0];
        let dy = center[1] - player_center[1];
        let dz = center[2] - player_center[2];
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

        for (i, &threshold) in self.config.lod_distances.iter().enumerate() {
            if distance < threshold {
                return i as u8;
            }
        }
        self.config.lod_distances.len() as u8
    }

    /// Compute the predicted player position based on velocity and prefetch time.
    pub fn predicted_position(&self, player: &PlayerView) -> [f32; 3] {
        let t = self.config.prefetch_time_secs;
        [
            player.position[0] + player.velocity[0] * t,
            player.position[1] + player.velocity[1] * t,
            player.position[2] + player.velocity[2] * t,
        ]
    }

    /// Run a streaming tick: request new chunks, activate/deactivate, and evict.
    pub fn tick(
        &mut self,
        player: &PlayerView,
        manifest: &ChunkManifest,
        now_ms: u64,
    ) -> Vec<StreamingEvent> {
        let mut events = Vec::new();

        let player_coord =
            ChunkCoord::from_world_position(player.position[0], player.position[1], player.position[2], self.config.chunk_size);

        let predicted_pos = self.predicted_position(player);
        let predicted_coord =
            ChunkCoord::from_world_position(predicted_pos[0], predicted_pos[1], predicted_pos[2], self.config.chunk_size);

        // Determine desired chunk sets
        let active_coords = player_coord.coords_within(self.config.active_radius);
        let cache_coords = player_coord.coords_within(self.config.cache_radius);
        let prefetch_coords = predicted_coord.coords_within(self.config.active_radius);

        // Merge all desired coords into a set
        let mut desired: HashMap<ChunkCoord, bool> = HashMap::new();
        for c in &active_coords {
            desired.insert(*c, true); // true = should be active
        }
        for c in &cache_coords {
            desired.entry(*c).or_insert(false); // false = cached is enough
        }
        for c in &prefetch_coords {
            desired.entry(*c).or_insert(false);
        }

        // Apply portal gating: filter out chunks not reachable through open portals
        let portal_visible = if self.config.portal_gating_enabled {
            self.compute_portal_visible(&player_coord, manifest)
        } else {
            // All chunks visible if portal gating disabled
            HashMap::new()
        };

        // Phase 1: Request new chunks that are in the desired set but Unloaded
        let coord_map = manifest.coord_map();
        for (coord, should_be_active) in &desired {
            // Skip if portal-gated and not visible
            if self.config.portal_gating_enabled && !portal_visible.is_empty() {
                if let Some(id) = self.coord_to_id.get(coord) {
                    if !portal_visible.contains_key(id) && !*should_be_active {
                        continue;
                    }
                }
            }

            if let Some(&id) = self.coord_to_id.get(coord) {
                if let Some(entry) = self.chunks.get(&id) {
                    if entry.state == ChunkState::Unloaded {
                        if self.inflight_count() >= self.config.max_inflight {
                            events.push(StreamingEvent::RequestDropped { id });
                            continue;
                        }
                        let lod = self.select_lod(coord, &player_coord);
                        // Transition: Unloaded -> Requested
                        if let Some(entry) = self.chunks.get_mut(&id) {
                            if entry.transition(ChunkState::Requested, now_ms).is_ok() {
                                entry.lod = lod;
                                events.push(StreamingEvent::ChunkRequested {
                                    id,
                                    coord: *coord,
                                    lod,
                                });
                            }
                        }
                    }
                }
            } else if coord_map.contains_key(coord) {
                // Chunk is in manifest but not tracked yet -- shouldn't normally happen
                // if load_manifest was called, but handle gracefully
            }
        }

        // Phase 2: Activate cached chunks within active radius
        for coord in &active_coords {
            if let Some(&id) = self.coord_to_id.get(coord) {
                if let Some(entry) = self.chunks.get(&id) {
                    if entry.state == ChunkState::Cached {
                        if let Some(entry) = self.chunks.get_mut(&id) {
                            if entry.transition(ChunkState::Active, now_ms).is_ok() {
                                events.push(StreamingEvent::ChunkActivated {
                                    id,
                                    coord: *coord,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Phase 3: Deactivate active chunks outside active radius
        let active_set: std::collections::HashSet<ChunkCoord> = active_coords.into_iter().collect();
        let to_deactivate: Vec<ChunkId> = self
            .chunks
            .iter()
            .filter(|(_, e)| e.state == ChunkState::Active && !active_set.contains(&e.coord))
            .map(|(id, _)| *id)
            .collect();

        for id in to_deactivate {
            if let Some(entry) = self.chunks.get_mut(&id) {
                let coord = entry.coord;
                if entry.transition(ChunkState::Cached, now_ms).is_ok() {
                    events.push(StreamingEvent::ChunkDeactivated { id, coord });
                }
            }
        }

        // Phase 4: Eviction pass
        let eviction_events = self.run_eviction(&player_coord, now_ms);
        events.extend(eviction_events);

        events
    }

    /// Compute which chunks are visible through open portals from the player's current chunk.
    fn compute_portal_visible(
        &self,
        player_coord: &ChunkCoord,
        manifest: &ChunkManifest,
    ) -> HashMap<ChunkId, bool> {
        let mut visible = HashMap::new();

        // The player's own chunk is always visible
        if let Some(&id) = self.coord_to_id.get(player_coord) {
            visible.insert(id, true);

            // BFS through open portals
            let mut queue = vec![id];
            while let Some(current) = queue.pop() {
                for portal in manifest.portals_from(current) {
                    if portal.default_open && !visible.contains_key(&portal.to_chunk) {
                        visible.insert(portal.to_chunk, true);
                        queue.push(portal.to_chunk);
                    }
                }
            }
        }

        visible
    }

    /// Run the eviction pass, returning events for chunks that begin eviction.
    fn run_eviction(
        &mut self,
        player_coord: &ChunkCoord,
        now_ms: u64,
    ) -> Vec<StreamingEvent> {
        let mut events = Vec::new();

        let candidates: Vec<EvictionCandidate> = self
            .chunks
            .values()
            .filter(|e| matches!(e.state, ChunkState::Cached))
            .map(|e| EvictionCandidate {
                id: e.id,
                coord: e.coord,
                last_access_ms: e.last_access_ms,
                size_bytes: e.size_bytes,
            })
            .collect();

        let to_evict =
            self.config
                .eviction_policy
                .select_evictions(&candidates, player_coord, now_ms);

        for id in to_evict {
            if let Some(entry) = self.chunks.get_mut(&id) {
                let coord = entry.coord;
                if entry.transition(ChunkState::Evicting, now_ms).is_ok() {
                    events.push(StreamingEvent::ChunkEvicting { id, coord });
                }
            }
        }

        events
    }

    /// Advance a requested chunk to the Loading state (called when I/O begins).
    pub fn begin_load(&mut self, id: ChunkId, now_ms: u64) -> Result<(), String> {
        let entry = self
            .chunks
            .get_mut(&id)
            .ok_or_else(|| format!("unknown chunk {id}"))?;

        if entry.state != ChunkState::Requested {
            return Err(format!(
                "chunk {} is in state {}, expected Requested",
                id, entry.state
            ));
        }

        entry
            .transition(ChunkState::Loading, now_ms)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::coord::ChunkCoord;
    use crate::chunk::eviction::EvictionPolicy;
    use crate::chunk::manifest::{ChunkManifest, ChunkReference, PortalDefinition, PortalFace};

    fn make_config() -> StreamingConfig {
        StreamingConfig {
            chunk_size: 64.0,
            active_radius: 1,
            cache_radius: 2,
            max_inflight: 4,
            prefetch_time_secs: 1.0,
            lod_distances: vec![128.0, 256.0],
            eviction_policy: EvictionPolicy::new(10, u64::MAX),
            portal_gating_enabled: false,
        }
    }

    fn make_small_manifest() -> ChunkManifest {
        let mut m = ChunkManifest::new("test".to_string(), 64.0);
        for x in -1..=1 {
            for z in -1..=1 {
                let id = ((x + 2) * 10 + (z + 2)) as u64;
                m.add_chunk(ChunkReference {
                    id: ChunkId(id), coord: ChunkCoord::new(x, 0, z),
                    asset_path: format!("terrain/{x}_0_{z}.bin"),
                    available_lods: vec![0, 1], size_per_lod: vec![1024, 512],
                    label: String::new(),
                });
            }
        }
        m
    }

    /// Helper: transition a chunk through Requested -> Loading -> target state.
    fn load_chunk_to(engine: &mut StreamingEngine, coord: ChunkCoord, target: ChunkState, base_ms: u64) {
        let id = *engine.coord_to_id.get(&coord).unwrap();
        let e = engine.chunks.get_mut(&id).unwrap();
        e.transition(ChunkState::Requested, base_ms).unwrap();
        e.transition(ChunkState::Loading, base_ms + 10).unwrap();
        e.transition(ChunkState::Cached, base_ms + 20).unwrap();
        if target == ChunkState::Active {
            e.transition(ChunkState::Active, base_ms + 30).unwrap();
        }
    }

    #[test]
    fn test_new_engine() {
        let engine = StreamingEngine::new(make_config());
        assert_eq!(engine.total_chunks(), 0);
        assert_eq!(engine.inflight_count(), 0);
        assert_eq!(engine.in_memory_count(), 0);
    }

    #[test]
    fn test_load_manifest() {
        let manifest = make_small_manifest();
        let mut engine = StreamingEngine::new(make_config());
        engine.load_manifest(&manifest);
        assert_eq!(engine.total_chunks(), 9);
        assert_eq!(engine.count_in_state(ChunkState::Unloaded), 9);
    }

    #[test]
    fn test_get_chunk_at() {
        let manifest = make_small_manifest();
        let mut engine = StreamingEngine::new(make_config());
        engine.load_manifest(&manifest);

        let entry = engine.get_chunk_at(&ChunkCoord::new(0, 0, 0));
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().state, ChunkState::Unloaded);

        let missing = engine.get_chunk_at(&ChunkCoord::new(10, 10, 10));
        assert!(missing.is_none());
    }

    #[test]
    fn test_tick_requests_chunks() {
        let manifest = make_small_manifest();
        let mut config = make_config();
        config.active_radius = 0; // Only request the player's own chunk
        config.cache_radius = 0;
        let mut engine = StreamingEngine::new(config);
        engine.load_manifest(&manifest);

        let player = PlayerView {
            position: [32.0, 32.0, 32.0], // Center of chunk (0,0,0)
            velocity: [0.0, 0.0, 0.0],
        };

        let events = engine.tick(&player, &manifest, 1000);
        let requests: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, StreamingEvent::ChunkRequested { .. }))
            .collect();
        assert!(!requests.is_empty());
    }

    #[test]
    fn test_tick_activates_cached_chunks() {
        let manifest = make_small_manifest();
        let mut config = make_config();
        config.active_radius = 0;
        config.cache_radius = 0;
        let mut engine = StreamingEngine::new(config);
        engine.load_manifest(&manifest);
        load_chunk_to(&mut engine, ChunkCoord::new(0, 0, 0), ChunkState::Cached, 100);

        let player = PlayerView { position: [32.0, 32.0, 32.0], velocity: [0.0, 0.0, 0.0] };
        let events = engine.tick(&player, &manifest, 1000);
        let activations: Vec<_> = events.iter()
            .filter(|e| matches!(e, StreamingEvent::ChunkActivated { .. })).collect();
        assert_eq!(activations.len(), 1);
    }

    #[test]
    fn test_tick_deactivates_distant_chunks() {
        let manifest = make_small_manifest();
        let mut config = make_config();
        config.active_radius = 0;
        config.cache_radius = 2;
        let mut engine = StreamingEngine::new(config);
        engine.load_manifest(&manifest);
        load_chunk_to(&mut engine, ChunkCoord::new(1, 0, 1), ChunkState::Active, 100);

        // active_radius=0, so chunk (1,0,1) should be deactivated
        let player = PlayerView { position: [32.0, 32.0, 32.0], velocity: [0.0, 0.0, 0.0] };
        let events = engine.tick(&player, &manifest, 1000);
        let deactivations: Vec<_> = events.iter()
            .filter(|e| matches!(e, StreamingEvent::ChunkDeactivated { .. })).collect();
        assert_eq!(deactivations.len(), 1);
    }

    #[test]
    fn test_inflight_limit_enforced() {
        let manifest = make_small_manifest();
        let mut config = make_config();
        config.max_inflight = 2;
        config.active_radius = 1;
        config.cache_radius = 1;
        let mut engine = StreamingEngine::new(config);
        engine.load_manifest(&manifest);

        let player = PlayerView {
            position: [32.0, 32.0, 32.0],
            velocity: [0.0, 0.0, 0.0],
        };

        let events = engine.tick(&player, &manifest, 1000);
        let requests: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, StreamingEvent::ChunkRequested { .. }))
            .collect();
        // Should not exceed max_inflight
        assert!(requests.len() <= 2);
    }

    #[test]
    fn test_select_lod_near() {
        let config = make_config();
        let engine = StreamingEngine::new(config);
        let player_coord = ChunkCoord::new(0, 0, 0);
        let chunk_coord = ChunkCoord::new(0, 0, 0); // same chunk
        let lod = engine.select_lod(&chunk_coord, &player_coord);
        assert_eq!(lod, 0); // closest = highest detail
    }

    #[test]
    fn test_select_lod_far() {
        let mut config = make_config();
        config.lod_distances = vec![64.0, 128.0];
        let engine = StreamingEngine::new(config);
        let player_coord = ChunkCoord::new(0, 0, 0);
        let chunk_coord = ChunkCoord::new(10, 0, 0); // very far
        let lod = engine.select_lod(&chunk_coord, &player_coord);
        assert_eq!(lod, 2); // beyond all thresholds
    }

    #[test]
    fn test_select_lod_medium() {
        let mut config = make_config();
        config.lod_distances = vec![64.0, 256.0, 512.0];
        config.chunk_size = 64.0;
        let engine = StreamingEngine::new(config);
        let player_coord = ChunkCoord::new(0, 0, 0);
        let chunk_coord = ChunkCoord::new(2, 0, 0);
        // Distance between centers: 2 * 64 = 128 world units
        // 128 >= 64 (lod0 threshold), 128 < 256 (lod1 threshold) -> lod 1
        let lod = engine.select_lod(&chunk_coord, &player_coord);
        assert_eq!(lod, 1);
    }

    #[test]
    fn test_predicted_position_stationary() {
        let config = make_config();
        let engine = StreamingEngine::new(config);
        let player = PlayerView {
            position: [100.0, 200.0, 300.0],
            velocity: [0.0, 0.0, 0.0],
        };
        let predicted = engine.predicted_position(&player);
        assert_eq!(predicted, [100.0, 200.0, 300.0]);
    }

    #[test]
    fn test_predicted_position_moving() {
        let mut config = make_config();
        config.prefetch_time_secs = 2.0;
        let engine = StreamingEngine::new(config);
        let player = PlayerView {
            position: [0.0, 0.0, 0.0],
            velocity: [5.0, 0.0, 10.0],
        };
        let predicted = engine.predicted_position(&player);
        assert!((predicted[0] - 10.0).abs() < f32::EPSILON);
        assert!((predicted[1] - 0.0).abs() < f32::EPSILON);
        assert!((predicted[2] - 20.0).abs() < f32::EPSILON);
    }

    fn get_id(engine: &StreamingEngine, x: i32, y: i32, z: i32) -> ChunkId {
        *engine.coord_to_id.get(&ChunkCoord::new(x, y, z)).unwrap()
    }

    #[test]
    fn test_notify_load_complete() {
        let manifest = make_small_manifest();
        let mut engine = StreamingEngine::new(make_config());
        engine.load_manifest(&manifest);
        let id = get_id(&engine, 0, 0, 0);
        let e = engine.chunks.get_mut(&id).unwrap();
        e.transition(ChunkState::Requested, 100).unwrap();
        e.transition(ChunkState::Loading, 200).unwrap();

        engine.notify_load_complete(id, 4096, 300).unwrap();
        assert_eq!(engine.get_chunk(&id).unwrap().state, ChunkState::Cached);
        assert_eq!(engine.get_chunk(&id).unwrap().size_bytes, 4096);
    }

    #[test]
    fn test_notify_load_complete_wrong_state() {
        let manifest = make_small_manifest();
        let mut engine = StreamingEngine::new(make_config());
        engine.load_manifest(&manifest);
        let id = get_id(&engine, 0, 0, 0);
        assert!(engine.notify_load_complete(id, 4096, 300).is_err());
    }

    #[test]
    fn test_notify_load_failed() {
        let manifest = make_small_manifest();
        let mut engine = StreamingEngine::new(make_config());
        engine.load_manifest(&manifest);
        let id = get_id(&engine, 0, 0, 0);
        let e = engine.chunks.get_mut(&id).unwrap();
        e.transition(ChunkState::Requested, 100).unwrap();
        e.transition(ChunkState::Loading, 200).unwrap();

        engine.notify_load_failed(id, 300).unwrap();
        assert_eq!(engine.get_chunk(&id).unwrap().state, ChunkState::Unloaded);
    }

    #[test]
    fn test_notify_evict_complete() {
        let manifest = make_small_manifest();
        let mut engine = StreamingEngine::new(make_config());
        engine.load_manifest(&manifest);
        let id = get_id(&engine, 0, 0, 0);
        load_chunk_to(&mut engine, ChunkCoord::new(0, 0, 0), ChunkState::Cached, 100);
        let e = engine.chunks.get_mut(&id).unwrap();
        e.size_bytes = 4096;
        e.transition(ChunkState::Evicting, 400).unwrap();

        engine.notify_evict_complete(id, 500).unwrap();
        assert_eq!(engine.get_chunk(&id).unwrap().state, ChunkState::Unloaded);
        assert_eq!(engine.get_chunk(&id).unwrap().size_bytes, 0);
    }

    #[test]
    fn test_begin_load() {
        let manifest = make_small_manifest();
        let mut engine = StreamingEngine::new(make_config());
        engine.load_manifest(&manifest);
        let id = get_id(&engine, 0, 0, 0);
        engine.chunks.get_mut(&id).unwrap().transition(ChunkState::Requested, 100).unwrap();

        engine.begin_load(id, 200).unwrap();
        assert_eq!(engine.get_chunk(&id).unwrap().state, ChunkState::Loading);
    }

    #[test]
    fn test_begin_load_wrong_state() {
        let manifest = make_small_manifest();
        let mut engine = StreamingEngine::new(make_config());
        engine.load_manifest(&manifest);
        let id = get_id(&engine, 0, 0, 0);
        assert!(engine.begin_load(id, 200).is_err());
    }

    #[test]
    fn test_eviction_triggered_over_capacity() {
        let manifest = make_small_manifest();
        let mut config = make_config();
        config.eviction_policy = EvictionPolicy::new(2, u64::MAX);
        config.active_radius = 0;
        config.cache_radius = 3;
        let mut engine = StreamingEngine::new(config);
        engine.load_manifest(&manifest);

        let coords = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 0)];
        for (i, &(x, z)) in coords.iter().enumerate() {
            load_chunk_to(&mut engine, ChunkCoord::new(x, 0, z), ChunkState::Cached, 100 + i as u64 * 10);
            let id = get_id(&engine, x, 0, z);
            engine.chunks.get_mut(&id).unwrap().size_bytes = 1024;
        }
        assert_eq!(engine.count_in_state(ChunkState::Cached), 5);

        let player = PlayerView { position: [32.0, 32.0, 32.0], velocity: [0.0, 0.0, 0.0] };
        let events = engine.tick(&player, &manifest, 1000);
        let evictions: Vec<_> = events.iter()
            .filter(|e| matches!(e, StreamingEvent::ChunkEvicting { .. })).collect();
        assert!(!evictions.is_empty());
    }

    #[test]
    fn test_portal_gating_filters_unreachable_chunks() {
        let mut manifest = ChunkManifest::new("test".to_string(), 64.0);

        // Three chunks in a row: A -> B -> C
        // Portal from A->B is open, but no portal from B->C
        manifest.add_chunk(ChunkReference {
            id: ChunkId(1),
            coord: ChunkCoord::new(0, 0, 0),
            asset_path: "a.bin".to_string(),
            available_lods: vec![0],
            size_per_lod: vec![1024],
            label: String::new(),
        });
        manifest.add_chunk(ChunkReference {
            id: ChunkId(2),
            coord: ChunkCoord::new(1, 0, 0),
            asset_path: "b.bin".to_string(),
            available_lods: vec![0],
            size_per_lod: vec![1024],
            label: String::new(),
        });
        manifest.add_chunk(ChunkReference {
            id: ChunkId(3),
            coord: ChunkCoord::new(2, 0, 0),
            asset_path: "c.bin".to_string(),
            available_lods: vec![0],
            size_per_lod: vec![1024],
            label: String::new(),
        });

        // Only portal from A to B
        manifest.add_portal(PortalDefinition {
            from_chunk: ChunkId(1),
            to_chunk: ChunkId(2),
            face: PortalFace::PositiveX,
            default_open: true,
        });

        let mut config = make_config();
        config.portal_gating_enabled = true;
        config.active_radius = 0;
        config.cache_radius = 2; // Would include all 3 chunks

        let mut engine = StreamingEngine::new(config);
        engine.load_manifest(&manifest);

        let player = PlayerView {
            position: [32.0, 32.0, 32.0], // In chunk (0,0,0) = chunk A
            velocity: [0.0, 0.0, 0.0],
        };

        let events = engine.tick(&player, &manifest, 1000);
        let requested_ids: Vec<ChunkId> = events
            .iter()
            .filter_map(|e| match e {
                StreamingEvent::ChunkRequested { id, .. } => Some(*id),
                _ => None,
            })
            .collect();

        // Chunk A (player's chunk, always requested as active) and B (portal-visible) should be requested
        // Chunk C should NOT be requested (not portal-reachable)
        assert!(requested_ids.contains(&ChunkId(1)));
        assert!(requested_ids.contains(&ChunkId(2)));
        assert!(!requested_ids.contains(&ChunkId(3)));
    }

    #[test]
    fn test_full_streaming_lifecycle() {
        let manifest = make_small_manifest();
        let mut config = make_config();
        config.active_radius = 0;
        config.cache_radius = 0;
        config.prefetch_time_secs = 0.0;
        let mut engine = StreamingEngine::new(config);
        engine.load_manifest(&manifest);

        let player = PlayerView {
            position: [32.0, 32.0, 32.0],
            velocity: [0.0, 0.0, 0.0],
        };

        // Tick 1: Request
        let events = engine.tick(&player, &manifest, 1000);
        assert!(events
            .iter()
            .any(|e| matches!(e, StreamingEvent::ChunkRequested { .. })));

        // Simulate I/O: begin load and complete
        let coord = ChunkCoord::new(0, 0, 0);
        let id = *engine.coord_to_id.get(&coord).unwrap();
        engine.begin_load(id, 1100).unwrap();
        engine.notify_load_complete(id, 2048, 1200).unwrap();

        // Tick 2: Should activate
        let events = engine.tick(&player, &manifest, 1300);
        assert!(events
            .iter()
            .any(|e| matches!(e, StreamingEvent::ChunkActivated { .. })));

        // Move player far away
        let far_player = PlayerView {
            position: [10000.0, 10000.0, 10000.0],
            velocity: [0.0, 0.0, 0.0],
        };

        // Tick 3: Should deactivate
        let events = engine.tick(&far_player, &manifest, 2000);
        assert!(events
            .iter()
            .any(|e| matches!(e, StreamingEvent::ChunkDeactivated { .. })));
    }

    #[test]
    fn test_prefetch_ahead_of_player() {
        let manifest = make_small_manifest();
        let mut config = make_config();
        config.active_radius = 0;
        config.cache_radius = 0;
        config.prefetch_time_secs = 2.0;
        config.chunk_size = 64.0;
        let mut engine = StreamingEngine::new(config);
        engine.load_manifest(&manifest);

        // Player at origin moving towards positive X
        let player = PlayerView {
            position: [32.0, 32.0, 32.0],
            velocity: [64.0, 0.0, 0.0], // 64 units/sec = 1 chunk/sec
        };

        // Predicted position: (32 + 64*2, 32, 32) = (160, 32, 32) = chunk (2,0,0)
        let predicted = engine.predicted_position(&player);
        let predicted_coord = ChunkCoord::from_world_position(predicted[0], predicted[1], predicted[2], 64.0);
        // With prefetch_time=2 and velocity=64, predicted chunk should be (2,0,0)
        assert_eq!(predicted_coord, ChunkCoord::new(2, 0, 0));
    }

    #[test]
    fn test_engine_with_empty_manifest() {
        let manifest = ChunkManifest::new("empty".to_string(), 64.0);
        let mut engine = StreamingEngine::new(make_config());
        engine.load_manifest(&manifest);
        assert_eq!(engine.total_chunks(), 0);

        let player = PlayerView {
            position: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
        };
        let events = engine.tick(&player, &manifest, 1000);
        assert!(events.is_empty());
    }
}
