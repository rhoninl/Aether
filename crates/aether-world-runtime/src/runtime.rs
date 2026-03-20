use std::collections::HashMap;

use crate::{
    chunking::{ChunkDescriptor, ChunkStreamingPolicy},
    lifecycle::{LifecycleEvent, RuntimeState},
    manifest::{validate_runtime_manifest, WorldManifestError, WorldRuntimeManifest},
    props::{LightingSetup, PropInstance, SpawnPoint, TerrainChunk},
    spawn::RuntimeSettingsError,
};

#[derive(Debug, Clone)]
pub struct WorldRuntimeConfig {
    pub stream_budget_bytes_per_tick: u64,
    pub max_visible_chunks: usize,
    pub max_inflight_chunks: usize,
    pub zone_split_threshold: u32,
    pub zone_merge_threshold: u32,
    pub zone_rebalance_cooldown_ms: u64,
    pub spawn_profile_guard_ms: u64,
    pub max_pending_ticks: u64,
    pub max_zone_count: u32,
    pub profile_tolerance_ms: u64,
}

impl Default for WorldRuntimeConfig {
    fn default() -> Self {
        Self {
            stream_budget_bytes_per_tick: 256_000,
            max_visible_chunks: 48,
            max_inflight_chunks: 32,
            zone_split_threshold: 64,
            zone_merge_threshold: 16,
            zone_rebalance_cooldown_ms: 1_000,
            spawn_profile_guard_ms: 250,
            max_pending_ticks: 5,
            max_zone_count: 8,
            profile_tolerance_ms: 5_000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorldRuntimeInput {
    pub now_ms: u64,
    pub commands: Vec<WorldRuntimeCommand>,
}

#[derive(Debug, Clone)]
pub enum WorldRuntimeCommand {
    Boot {
        manifest: WorldRuntimeManifest,
        terrain_chunks: Vec<TerrainChunk>,
        props: Vec<PropInstance>,
        lighting: Vec<LightingSetup>,
        spawn_points: Vec<SpawnPoint>,
    },
    Tick {
        world_id: String,
        player_count: u32,
        requested_lod: u8,
        cpu_ms: u32,
        physics_ms: u32,
        network_ms: u32,
        script_ms: u32,
    },
    LoadChunks {
        world_id: String,
        near_entities: Vec<ChunkDescriptor>,
    },
    RebalanceZones {
        world_id: String,
        now_ms: u64,
        active_player_count: u32,
        requested_cluster_count: Option<u32>,
        now_spawn_points: usize,
    },
    Shutdown {
        world_id: String,
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub struct PerformanceSample {
    pub world_id: String,
    pub render_ms: u32,
    pub physics_ms: u32,
    pub network_ms: u32,
    pub scripting_ms: u32,
}

#[derive(Debug, Default)]
pub struct WorldRuntimeOutput {
    pub events: Vec<LifecycleEvent>,
    pub loaded_chunks: Vec<ChunkDescriptor>,
    pub unload_events: Vec<String>,
    pub error_report: Vec<String>,
    pub samples: Vec<PerformanceSample>,
    pub zone_decisions: Vec<String>,
    pub ghost_handoffs: Vec<String>,
}

#[derive(Debug, Default)]
pub struct WorldRuntimeState {
    cfg: WorldRuntimeConfig,
    world_states: HashMap<String, WorldRuntimeWorldState>,
}

#[derive(Debug)]
struct WorldRuntimeWorldState {
    _manifest: WorldRuntimeManifest,
    state: RuntimeState,
    _terrain_chunks: Vec<TerrainChunk>,
    pending_chunks: Vec<ChunkDescriptor>,
    loaded_chunks: HashMap<u64, ChunkDescriptor>,
    _props: Vec<PropInstance>,
    _lighting: Vec<LightingSetup>,
    spawn_points: Vec<SpawnPoint>,
    last_tick: u64,
    last_tick_ms: u64,
    last_boot_ms: u64,
    last_stream_ms: u64,
    boot_error: Option<String>,
    tick_errors: u32,
    _enforced_gravity: f32,
    _enforced_tick_rate: u32,
    enforced_players: u32,
    zone_count: u32,
    zone_count_target: u32,
    last_zone_rebalance_ms: u64,
    ghost_entities: Vec<u64>,
    chunk_drops: u64,
}

impl WorldRuntime {
    pub fn new(cfg: WorldRuntimeConfig) -> Self {
        Self {
            state: WorldRuntimeState {
                cfg,
                world_states: HashMap::new(),
            },
        }
    }

    pub fn state(&self) -> &WorldRuntimeState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut WorldRuntimeState {
        &mut self.state
    }

    pub fn step(&mut self, input: &WorldRuntimeInput) -> WorldRuntimeOutput {
        let mut output = WorldRuntimeOutput::default();
        for command in &input.commands {
            match command {
                WorldRuntimeCommand::Boot {
                    manifest,
                    terrain_chunks,
                    props,
                    lighting,
                    spawn_points,
                } => {
                    self.boot_world(
                        manifest,
                        terrain_chunks,
                        props,
                        lighting,
                        spawn_points,
                        input.now_ms,
                        &mut output,
                    );
                }
                WorldRuntimeCommand::Tick {
                    world_id,
                    player_count,
                    requested_lod: _,
                    cpu_ms,
                    physics_ms,
                    network_ms,
                    script_ms,
                } => {
                    self.step_world(
                        world_id,
                        *player_count,
                        *cpu_ms,
                        *physics_ms,
                        *network_ms,
                        *script_ms,
                        input.now_ms,
                        &mut output,
                    );
                }
                WorldRuntimeCommand::LoadChunks {
                    world_id,
                    near_entities,
                } => {
                    self.stream_chunks(world_id, near_entities, input.now_ms, &mut output);
                }
                WorldRuntimeCommand::RebalanceZones {
                    world_id,
                    now_ms,
                    active_player_count,
                    requested_cluster_count,
                    now_spawn_points,
                } => {
                    self.rebalance_zones(
                        world_id,
                        *now_ms,
                        *active_player_count,
                        *requested_cluster_count,
                        *now_spawn_points,
                        &mut output,
                    );
                }
                WorldRuntimeCommand::Shutdown { world_id, reason } => {
                    self.shutdown_world(world_id, reason, &mut output);
                }
            }
        }
        output
    }

    #[allow(clippy::too_many_arguments)]
    fn boot_world(
        &mut self,
        manifest: &WorldRuntimeManifest,
        terrain_chunks: &[TerrainChunk],
        props: &[PropInstance],
        lighting: &[LightingSetup],
        spawn_points: &[SpawnPoint],
        now_ms: u64,
        output: &mut WorldRuntimeOutput,
    ) {
        match Self::validate_settings(manifest) {
            Ok(()) => {
                if spawn_points.is_empty() {
                    let state = WorldRuntimeWorldState {
                        _manifest: manifest.clone(),
                        state: RuntimeState::StoppedError("missing_spawn_points".into()),
                        _terrain_chunks: terrain_chunks.to_vec(),
                        pending_chunks: Vec::new(),
                        loaded_chunks: HashMap::new(),
                        _props: props.to_vec(),
                        _lighting: lighting.to_vec(),
                        spawn_points: spawn_points.to_vec(),
                        last_tick: 0,
                        last_tick_ms: now_ms,
                        last_boot_ms: now_ms,
                        last_stream_ms: now_ms,
                        boot_error: Some("missing_spawn_points".into()),
                        tick_errors: 0,
                        _enforced_gravity: manifest.gravity.clamp(-9.8, 9.8),
                        _enforced_tick_rate: manifest.tick_rate_hz.clamp(1, 240),
                        enforced_players: manifest.max_players.max(1),
                        zone_count: 1,
                        zone_count_target: 1,
                        last_zone_rebalance_ms: 0,
                        ghost_entities: Vec::new(),
                        chunk_drops: 0,
                    };
                    self.state
                        .world_states
                        .insert(manifest.world_id.clone(), state);
                    output.events.push(LifecycleEvent {
                        world_id: manifest.world_id.clone(),
                        state: RuntimeState::StoppedError("missing_spawn_points".into()),
                        timestamp_ms: now_ms,
                    });
                    output.error_report.push(format!(
                        "boot failed {}: no spawn points",
                        manifest.world_id
                    ));
                    return;
                }

                let state = WorldRuntimeWorldState {
                    _manifest: manifest.clone(),
                    state: RuntimeState::Running,
                    _terrain_chunks: terrain_chunks.to_vec(),
                    pending_chunks: Vec::new(),
                    loaded_chunks: HashMap::new(),
                    _props: props.to_vec(),
                    _lighting: lighting.to_vec(),
                    spawn_points: spawn_points.to_vec(),
                    last_tick: 0,
                    last_tick_ms: now_ms,
                    last_boot_ms: now_ms,
                    last_stream_ms: now_ms,
                    boot_error: None,
                    tick_errors: 0,
                    _enforced_gravity: manifest.gravity.clamp(-9.8, 9.8),
                    _enforced_tick_rate: manifest.tick_rate_hz.clamp(1, 240),
                    enforced_players: manifest.max_players.max(1),
                    zone_count: 1,
                    zone_count_target: 1,
                    last_zone_rebalance_ms: 0,
                    ghost_entities: Vec::new(),
                    chunk_drops: 0,
                };

                let world_id = manifest.world_id.clone();
                self.state.world_states.insert(world_id.clone(), state);
                output.events.push(LifecycleEvent {
                    world_id: world_id.clone(),
                    state: RuntimeState::Booting,
                    timestamp_ms: now_ms,
                });
                output.events.push(LifecycleEvent {
                    world_id,
                    state: RuntimeState::Running,
                    timestamp_ms: now_ms,
                });
            }
            Err(err) => {
                output
                    .error_report
                    .push(format!("boot {}: {:?}", manifest.world_id, err));
                let state = WorldRuntimeWorldState {
                    _manifest: manifest.clone(),
                    state: RuntimeState::StoppedError(format!("{err:?}")),
                    _terrain_chunks: terrain_chunks.to_vec(),
                    pending_chunks: Vec::new(),
                    loaded_chunks: HashMap::new(),
                    _props: props.to_vec(),
                    _lighting: lighting.to_vec(),
                    spawn_points: spawn_points.to_vec(),
                    last_tick: 0,
                    last_tick_ms: now_ms,
                    last_boot_ms: now_ms,
                    last_stream_ms: now_ms,
                    boot_error: Some(format!("{err:?}")),
                    tick_errors: 0,
                    _enforced_gravity: manifest.gravity,
                    _enforced_tick_rate: manifest.tick_rate_hz,
                    enforced_players: manifest.max_players,
                    zone_count: 1,
                    zone_count_target: 1,
                    last_zone_rebalance_ms: 0,
                    ghost_entities: Vec::new(),
                    chunk_drops: 0,
                };
                self.state
                    .world_states
                    .insert(manifest.world_id.clone(), state);
                output.events.push(LifecycleEvent {
                    world_id: manifest.world_id.clone(),
                    state: RuntimeState::StoppedError(format!("boot failed: {err:?}")),
                    timestamp_ms: now_ms,
                });
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn step_world(
        &mut self,
        world_id: &str,
        player_count: u32,
        cpu_ms: u32,
        physics_ms: u32,
        network_ms: u32,
        script_ms: u32,
        now_ms: u64,
        output: &mut WorldRuntimeOutput,
    ) {
        let state = match self.state.world_states.get_mut(world_id) {
            Some(state) => state,
            None => {
                output
                    .error_report
                    .push(format!("tick on missing world: {world_id}"));
                return;
            }
        };

        if !matches!(state.state, RuntimeState::Running) {
            if state.state == RuntimeState::Draining {
                state.state = RuntimeState::Stopped;
                output.events.push(LifecycleEvent {
                    world_id: world_id.to_string(),
                    state: RuntimeState::Stopped,
                    timestamp_ms: now_ms,
                });
            } else {
                output
                    .error_report
                    .push(format!("tick for non-running world: {world_id}"));
            }
            state.tick_errors = state.tick_errors.saturating_add(1);
            return;
        }

        state.last_tick = state.last_tick.saturating_add(1);
        state.last_tick_ms = now_ms;

        if now_ms.saturating_sub(state.last_boot_ms) >= self.state.cfg.spawn_profile_guard_ms
            && state.spawn_points.is_empty()
        {
            state.state = RuntimeState::StoppedError("missing_spawn_points".into());
            output.events.push(LifecycleEvent {
                world_id: world_id.to_string(),
                state: RuntimeState::StoppedError("missing_spawn_points".into()),
                timestamp_ms: now_ms,
            });
            output
                .error_report
                .push(format!("world {world_id} stopped: missing spawn points"));
            return;
        }

        if player_count > state.enforced_players {
            state.tick_errors = state.tick_errors.saturating_add(1);
            output.error_report.push(format!(
                "tick {world_id}: player_count {player_count} > enforced_players {}",
                state.enforced_players
            ));
        }

        if player_count > 500 {
            state.enforced_players = player_count;
            state.tick_errors = state.tick_errors.saturating_add(1);
            output.error_report.push(format!(
                "tick {world_id}: unusual player count {player_count}"
            ));
        }

        if state.tick_errors as u64 >= self.state.cfg.max_pending_ticks {
            state.state = RuntimeState::StoppedError("tick_budget_exceeded".into());
            output
                .error_report
                .push(format!("world {world_id} exceeded tick error budget"));
            return;
        }

        if cpu_ms > 250 || physics_ms > 250 || network_ms > 250 || script_ms > 250 {
            state.tick_errors = state.tick_errors.saturating_add(1);
        }

        if state.boot_error.is_none() {
            state.boot_error = Some("settings applied".into());
        }
        if now_ms.saturating_sub(state.last_tick_ms) <= self.state.cfg.profile_tolerance_ms {
            output.events.push(LifecycleEvent {
                world_id: world_id.to_string(),
                state: RuntimeState::Running,
                timestamp_ms: now_ms,
            });
        }

        output.samples.push(PerformanceSample {
            world_id: world_id.to_string(),
            render_ms: cpu_ms,
            physics_ms,
            network_ms,
            scripting_ms: script_ms,
        });
    }

    fn stream_chunks(
        &mut self,
        world_id: &str,
        near_entities: &[ChunkDescriptor],
        now_ms: u64,
        output: &mut WorldRuntimeOutput,
    ) {
        let Some(state) = self.state.world_states.get_mut(world_id) else {
            output
                .error_report
                .push(format!("stream on missing world: {world_id}"));
            return;
        };

        if !matches!(state.state, RuntimeState::Running) {
            output
                .error_report
                .push(format!("stream on non-running world: {world_id}"));
            return;
        }

        if near_entities.is_empty() && state.pending_chunks.is_empty() {
            return;
        }

        let policy = ChunkStreamingPolicy {
            max_inflight: self
                .state
                .cfg
                .max_inflight_chunks
                .max(1)
                .min(u16::MAX as usize) as u16,
            min_prefetch_distance: 12.0,
            target_bytes_per_second: self.state.cfg.stream_budget_bytes_per_tick,
        };

        let mut pending: Vec<ChunkDescriptor> = state.pending_chunks.drain(..).collect();
        pending.extend_from_slice(near_entities);
        pending.sort_by_key(|chunk| (chunk.lod, chunk.chunk_id));
        pending.dedup_by_key(|chunk| chunk.chunk_id);

        let budget_total = if pending.is_empty() {
            0
        } else {
            let chunks_to_preload = pending.len().max(1) as u64;
            self.state
                .cfg
                .stream_budget_bytes_per_tick
                .min(128_000 * chunks_to_preload)
        };
        let mut budget = budget_total;
        let mut loaded_this_tick = 0u32;
        for chunk in pending {
            if state.loaded_chunks.contains_key(&chunk.chunk_id) {
                continue;
            }

            let estimated_cost = Self::chunk_estimated_cost(&chunk);
            if state.loaded_chunks.len() >= self.state.cfg.max_visible_chunks {
                state.pending_chunks.push(chunk);
                continue;
            }
            if state.pending_chunks.len() >= self.state.cfg.max_inflight_chunks {
                state.pending_chunks.push(chunk);
                continue;
            }
            if budget < estimated_cost {
                state.pending_chunks.push(chunk);
                continue;
            }

            budget = budget.saturating_sub(estimated_cost);
            state.loaded_chunks.insert(chunk.chunk_id, chunk.clone());
            output.loaded_chunks.push(chunk);
            state.last_stream_ms = now_ms;
            loaded_this_tick = loaded_this_tick.saturating_add(1);
        }

        let max_inflight = usize::from(policy.max_inflight);
        if state.pending_chunks.len() > max_inflight {
            let dropped = state.pending_chunks.len() - max_inflight;
            state.pending_chunks.drain(..dropped);
            state.chunk_drops = state.chunk_drops.saturating_add(dropped as u64);
            output.error_report.push(format!(
                "stream backlog {world_id}: dropped {dropped} chunks"
            ));
        }

        if state.loaded_chunks.len() > self.state.cfg.max_visible_chunks {
            let mut loaded_keys: Vec<u64> = state.loaded_chunks.keys().copied().collect();
            loaded_keys.sort_unstable();
            let excess = state
                .loaded_chunks
                .len()
                .saturating_sub(self.state.cfg.max_visible_chunks);
            for key in loaded_keys.into_iter().take(excess) {
                state.loaded_chunks.remove(&key);
                output
                    .unload_events
                    .push(format!("unloaded {world_id}:{key}"));
            }
        }

        output.samples.push(PerformanceSample {
            world_id: world_id.to_string(),
            render_ms: (policy.target_bytes_per_second - budget) as u32,
            physics_ms: state.loaded_chunks.len() as u32,
            network_ms: loaded_this_tick.saturating_add(state.loaded_chunks.len() as u32),
            scripting_ms: now_ms as u32 % 1_000,
        });
        output.events.push(LifecycleEvent {
            world_id: world_id.to_string(),
            state: RuntimeState::Running,
            timestamp_ms: now_ms,
        });
    }

    fn rebalance_zones(
        &mut self,
        world_id: &str,
        now_ms: u64,
        active_player_count: u32,
        requested_cluster_count: Option<u32>,
        now_spawn_points: usize,
        output: &mut WorldRuntimeOutput,
    ) {
        let state = match self.state.world_states.get_mut(world_id) {
            Some(state) => state,
            None => {
                output
                    .error_report
                    .push(format!("rebalance on missing world: {world_id}"));
                return;
            }
        };

        if !matches!(state.state, RuntimeState::Running) {
            return;
        }
        if now_ms.saturating_sub(state.last_zone_rebalance_ms)
            < self.state.cfg.zone_rebalance_cooldown_ms
        {
            return;
        }

        state.zone_count_target = requested_cluster_count
            .unwrap_or_else(|| 1 + active_player_count / self.state.cfg.zone_split_threshold.max(1))
            .clamp(1, self.state.cfg.max_zone_count.max(1));

        if state.zone_count < state.zone_count_target {
            while state.zone_count < state.zone_count_target
                && state.zone_count < self.state.cfg.max_zone_count.max(1)
            {
                let from = state.zone_count;
                state.zone_count = (state.zone_count + 1).min(self.state.cfg.max_zone_count.max(1));
                state.ghost_entities.push(state.last_tick);
                output.zone_decisions.push(format!(
                    "split {world_id}:{from}->{}:players:{}",
                    state.zone_count, active_player_count
                ));
                output.ghost_handoffs.push(format!(
                    "handoff:{world_id}:{from}->{}:spawn_points:{}",
                    state.zone_count, now_spawn_points
                ));
            }
        } else if state.zone_count > state.zone_count_target
            || now_spawn_points < usize::try_from(state.zone_count).unwrap_or(0)
        {
            while state.zone_count > state.zone_count_target && state.zone_count > 1 {
                let from = state.zone_count;
                state.zone_count = state.zone_count.saturating_sub(1);
                output.zone_decisions.push(format!(
                    "merge {world_id}:{from}->{},players:{}",
                    state.zone_count, active_player_count
                ));
                output
                    .ghost_handoffs
                    .push(format!("handoff:{world_id}:{from}->{}", state.zone_count));
            }
        } else {
            output
                .zone_decisions
                .push(format!("normalize {world_id}:{}", state.zone_count));
        }

        state.last_zone_rebalance_ms = now_ms;
        output.events.push(LifecycleEvent {
            world_id: world_id.to_string(),
            state: RuntimeState::Running,
            timestamp_ms: now_ms,
        });
        output.samples.push(PerformanceSample {
            world_id: world_id.to_string(),
            render_ms: 0,
            physics_ms: active_player_count,
            network_ms: state.zone_count,
            scripting_ms: state.ghost_entities.len() as u32,
        });
    }

    fn shutdown_world(&mut self, world_id: &str, reason: &str, output: &mut WorldRuntimeOutput) {
        match self.state.world_states.remove(world_id) {
            Some(mut state) => {
                state.state = RuntimeState::Draining;
                output.events.push(LifecycleEvent {
                    world_id: world_id.to_string(),
                    state: RuntimeState::Draining,
                    timestamp_ms: 0,
                });
                state.loaded_chunks.clear();
                state.pending_chunks.clear();
                state.state = RuntimeState::Stopped;
                output.events.push(LifecycleEvent {
                    world_id: world_id.to_string(),
                    state: RuntimeState::Stopped,
                    timestamp_ms: 0,
                });
                if !reason.is_empty() {
                    output
                        .error_report
                        .push(format!("world stopped: {world_id} {reason}"));
                }
            }
            None => {
                output
                    .error_report
                    .push(format!("shutdown missing world: {world_id}"));
            }
        }
    }

    #[allow(dead_code)]
    fn stream_budget(&self, chunks: &[ChunkDescriptor]) -> u64 {
        if chunks.is_empty() {
            return 0;
        }
        let chunks_to_preload = chunks.len().max(1) as u64;
        self.state
            .cfg
            .stream_budget_bytes_per_tick
            .min(128_000 * chunks_to_preload)
    }

    fn chunk_estimated_cost(chunk: &ChunkDescriptor) -> u64 {
        chunk
            .size_bytes
            .saturating_mul((u64::from(chunk.lod) + 1).max(1))
            .max(128)
    }
}

pub struct WorldRuntime {
    state: WorldRuntimeState,
}

impl Default for WorldRuntime {
    fn default() -> Self {
        Self::new(WorldRuntimeConfig::default())
    }
}

impl WorldRuntime {
    pub fn validate_manifest(manifest: &WorldRuntimeManifest) -> Result<(), WorldManifestError> {
        validate_runtime_manifest(manifest)
    }

    pub fn validate_settings(manifest: &WorldRuntimeManifest) -> Result<(), RuntimeSettingsError> {
        let manifest_validation = validate_runtime_manifest(manifest);
        match manifest_validation {
            Ok(()) => {
                if manifest.max_players == 0 || manifest.spawn_points == 0 {
                    return Err(RuntimeSettingsError::TooManyPlayers);
                }
                if manifest.max_players < manifest.spawn_points {
                    return Err(RuntimeSettingsError::InvalidSpawnPoints);
                }
                if manifest.tick_rate_hz == 0 {
                    return Err(RuntimeSettingsError::TickRateTooLow);
                }
                Ok(())
            }
            Err(WorldManifestError::GravityUnrealistic) => {
                Err(RuntimeSettingsError::GravityCritical)
            }
            Err(WorldManifestError::MissingTerrain)
            | Err(WorldManifestError::MissingEnvironment) => {
                Err(RuntimeSettingsError::InvalidSpawnPoints)
            }
            Err(WorldManifestError::InvalidTickRate) => Err(RuntimeSettingsError::TickRateTooLow),
            Err(WorldManifestError::MaxPlayersZero) => Err(RuntimeSettingsError::TooManyPlayers),
        }
    }
}
