use std::collections::{HashMap, HashSet};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::{
    CriticalStatePriority, CriticalWriteResult, PodPlacementHint, Snapshot,
    SnapshotRecorder, SyncStateMutation, WorldManifest, WorldPersistenceProfile, WalAppendError,
    WalAppendResult, WalDurability, WalReplayRecord, WalWriteCoordinator,
};
use crate::transactions::{CriticalStateKey as TxStateKey, SyncStateChannel};

#[derive(Debug, Clone)]
pub struct PersistenceRuntimeConfig {
    pub wal_segment_entry_limit: usize,
    pub snapshot_keep_ms: u64,
    pub script_state_max_events: usize,
}

impl Default for PersistenceRuntimeConfig {
    fn default() -> Self {
        Self {
            wal_segment_entry_limit: 1024,
            snapshot_keep_ms: 10 * 60 * 1000,
            script_state_max_events: 10_000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeTickInput {
    pub tick: u64,
    pub now_ms: u64,
}

#[derive(Debug, Clone)]
pub struct WorldRuntimeInput {
    pub profile: WorldPersistenceProfile,
    pub actor_count: u32,
    pub critical_mutations: Vec<SyncStateMutation>,
    pub wal_ack_upto: Option<u64>,
    pub request_recovery: bool,
}

#[derive(Debug, Clone, Default)]
pub struct WorldRecovery {
    pub restored: bool,
    pub restored_snapshot: Option<Snapshot>,
    pub restored_scripts: Vec<(String, Vec<u8>)>,
    pub replay_records: Vec<WalReplayRecord>,
}

#[derive(Debug, Default)]
pub struct PersistenceRuntimeOutput {
    pub produced_snapshots: Vec<Snapshot>,
    pub wal_appends: Vec<WalAppendResult>,
    pub wal_replays: Vec<WalReplayRecord>,
    pub critical_writes: Vec<CriticalWriteResult>,
    pub placement_hints: Vec<PodPlacementHint>,
    pub recovered_worlds: Vec<(String, u64)>,
    pub idempotent_skips: usize,
    pub wal_committed: usize,
    pub recovery: Vec<WorldRecovery>,
}

#[derive(Debug)]
struct RuntimeWorldState {
    profile: WorldPersistenceProfile,
    snapshot_recorder: SnapshotRecorder,
    wal: WalWriteCoordinator,
    sync: SyncStateChannel,
    placement_hint: PodPlacementHint,
    script_state: HashMap<String, Vec<u8>>,
    seen_mutation_keys: HashSet<u64>,
    sequence_to_script_payload: HashMap<u64, (String, Vec<u8>)>,
}

impl RuntimeWorldState {
    fn new(profile: &WorldPersistenceProfile, cfg: &PersistenceRuntimeConfig) -> Self {
        let manifest = WorldManifest {
            world_id: profile.world_id.clone(),
            world_name: profile.world_id.clone(),
            durability_class: profile.classify_pod(),
            p2p_enabled: true,
            expected_players: profile.expected_players,
            economy_enabled: profile.has_economy_state,
        };
        Self {
            profile: profile.clone(),
            snapshot_recorder: SnapshotRecorder::new(),
            wal: WalWriteCoordinator::new(cfg.wal_segment_entry_limit),
            sync: SyncStateChannel::default(),
            placement_hint: manifest.make_placement_hint(),
            script_state: HashMap::new(),
            seen_mutation_keys: HashSet::new(),
            sequence_to_script_payload: HashMap::new(),
        }
    }

    fn update_profile(&mut self, profile: &WorldPersistenceProfile) {
        self.profile = profile.clone();
        let manifest = WorldManifest {
            world_id: profile.world_id.clone(),
            world_name: profile.world_id.clone(),
            durability_class: profile.classify_pod(),
            p2p_enabled: true,
            expected_players: profile.expected_players,
            economy_enabled: profile.has_economy_state,
        };
        self.placement_hint = manifest.make_placement_hint();
    }

    fn restore_from_snapshot_and_wal(
        &mut self,
        _now_ms: u64,
        max_replay_records: usize,
    ) -> WorldRecovery {
        let mut recovered_snapshot = self
            .snapshot_recorder
            .frames
            .iter()
            .filter(|snapshot| snapshot.kind == crate::snapshot::SnapshotKind::Durable)
            .max_by_key(|snapshot| snapshot.captured_ms)
            .cloned();

        if recovered_snapshot.is_none() {
            recovered_snapshot = self
                .snapshot_recorder
                .frames
                .iter()
                .max_by_key(|snapshot| snapshot.captured_ms)
                .cloned();
        }

        let replays = self.wal.replay();
        let mut restored_scripts = Vec::new();
        for replay in replays.iter().take(max_replay_records) {
            if let Some((script, payload)) =
                self.sequence_to_script_payload.get(&replay.sequence)
            {
                self.script_state
                    .insert(script.clone(), payload.clone());
                restored_scripts.push((script.clone(), payload.clone()));
            }
        }

        if self.profile.has_durable_script_state {
            for (script, payload) in restored_scripts.iter() {
                self.script_state.insert(script.clone(), payload.clone());
            }
        }

        WorldRecovery {
            restored: recovered_snapshot.is_some(),
            restored_snapshot: recovered_snapshot,
            restored_scripts,
            replay_records: replays,
        }
    }

}

#[derive(Debug, Default)]
pub struct PersistenceRuntimeState {
    worlds: HashMap<String, RuntimeWorldState>,
}

#[derive(Debug)]
pub struct PersistenceRuntime {
    cfg: PersistenceRuntimeConfig,
}

impl PersistenceRuntime {
    pub fn new(cfg: PersistenceRuntimeConfig) -> Self {
        Self { cfg }
    }

    pub fn default() -> Self {
        Self::new(PersistenceRuntimeConfig::default())
    }

    pub fn resolve_placement(&self, manifest: &WorldManifest) -> PodPlacementHint {
        manifest.make_placement_hint()
    }

    pub fn step(
        &self,
        input: RuntimeTickInput,
        worlds: &[WorldRuntimeInput],
        state: &mut PersistenceRuntimeState,
    ) -> PersistenceRuntimeOutput {
        let mut output = PersistenceRuntimeOutput::default();

        for world in worlds {
            let profile = &world.profile;
            let mut world_state = state
                .worlds
                .remove(&profile.world_id)
                .unwrap_or_else(|| RuntimeWorldState::new(profile, &self.cfg));
            world_state.update_profile(profile);

            output.placement_hints.push(world_state.placement_hint.clone());
            world_state.snapshot_recorder.prune_older_than_ms(self.cfg.snapshot_keep_ms, input.now_ms);

            if let Some(ack_upto) = world.wal_ack_upto {
                let committed = world_state.wal.ack(ack_upto);
                output.wal_committed += committed;
            }

            if world.request_recovery {
                let recovery = world_state
                    .restore_from_snapshot_and_wal(input.now_ms, self.cfg.script_state_max_events);
                let recovered_snapshot_id = recovery
                    .restored_snapshot
                    .as_ref()
                    .map_or(0, |snapshot| snapshot.id);
                output.wal_replays.extend_from_slice(&recovery.replay_records);
                output.recovery.push(recovery);
                output
                    .recovered_worlds
                    .push((profile.world_id.clone(), recovered_snapshot_id));
            }

            if let Some(snapshot) = world_state
                .snapshot_recorder
                .push(profile, input.tick, input.now_ms, world.actor_count)
            {
                output.produced_snapshots.push(snapshot);
            }

            for mutation in &world.critical_mutations {
                let signature = mutation_signature(profile.world_id.as_str(), mutation);
                if !world_state.seen_mutation_keys.insert(signature) {
                    output.idempotent_skips = output.idempotent_skips.saturating_add(1);
                    continue;
                }

                let key = transaction_key_to_wal(&mutation.key);
                let mut crc = crc32_simple(&mutation.payload);
                if crc == 0 {
                    crc = 1;
                }
                let durability = if matches!(mutation.priority, CriticalStatePriority::High) {
                    WalDurability::FsyncBeforeAck
                } else {
                    WalDurability::Ephemeral
                };
                match world_state
                    .wal
                    .append(
                        profile.world_id.clone(),
                        key.clone(),
                        crc,
                        durability,
                        input.now_ms,
                    ) {
                    Ok(appended) => {
                        output.wal_appends.push(appended.clone());
                        if let TxStateKey::ScriptState(script) = &mutation.key {
                            let mut payload = mutation.payload.clone();
                            payload.shrink_to_fit();
                            world_state
                                .sequence_to_script_payload
                                .insert(appended.sequence, (script.clone(), payload.clone()));
                            world_state
                                .script_state
                                .insert(script.clone(), payload.clone());
                        }
                        let record = world_state.sync.enqueue(profile.world_id.clone(), mutation.clone());
                        output
                            .critical_writes
                            .push(world_state.sync.commit(record.mutation_id));
                    }
                    Err(error) => {
                        if matches!(error, WalAppendError::SegmentFull) {
                            output.critical_writes.push(CriticalWriteResult {
                                acknowledged: false,
                                elapsed: std::time::Duration::from_millis(0),
                                sequence: 0,
                            });
                            let _ = error;
                        }
                    }
                }
            }
            state.worlds.insert(profile.world_id.clone(), world_state);
        }

        output
    }
}

fn mutation_signature(world_id: &str, mutation: &SyncStateMutation) -> u64 {
    let mut h = DefaultHasher::new();
    world_id.hash(&mut h);
    mutation.actor_id.hash(&mut h);
    mutation.timestamp_ms.hash(&mut h);
    mutation.priority.hash(&mut h);
    match &mutation.key {
        TxStateKey::Economy => "economy".hash(&mut h),
        TxStateKey::Inventory => "inventory".hash(&mut h),
        TxStateKey::Identity => "identity".hash(&mut h),
        TxStateKey::ScriptState(name) => {
            "script_state".hash(&mut h);
            name.hash(&mut h);
        }
    }
    mutation.payload.hash(&mut h);
    h.finish()
}

fn transaction_key_to_wal(key: &TxStateKey) -> String {
    match key {
        TxStateKey::Economy => "critical/economy".to_string(),
        TxStateKey::Inventory => "critical/inventory".to_string(),
        TxStateKey::Identity => "critical/identity".to_string(),
        TxStateKey::ScriptState(name) => format!("critical/script/{name}"),
    }
}

fn crc32_simple(data: &[u8]) -> u32 {
    let mut hash = 0x811C9DC5u32;
    for byte in data {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transactions::CriticalStateKey;

    #[test]
    fn snapshot_and_recovery_roundtrip() {
        let profile = WorldPersistenceProfile::from_defaults("world-1");
        let runtime = PersistenceRuntime::new(PersistenceRuntimeConfig {
            snapshot_keep_ms: 1000,
            ..Default::default()
        });
        let mut state = PersistenceRuntimeState::default();

        let input = WorldRuntimeInput {
            profile: profile.clone(),
            actor_count: 3,
            critical_mutations: vec![],
            wal_ack_upto: None,
            request_recovery: false,
        };
        let out = runtime.step(
            RuntimeTickInput {
                tick: 1,
                now_ms: 1000,
            },
            &[input],
            &mut state,
        );
        assert!(!out.produced_snapshots.is_empty());

        let recover = runtime.step(
            RuntimeTickInput {
                tick: 2,
                now_ms: 2000,
            },
            &[WorldRuntimeInput {
                request_recovery: true,
                ..WorldRuntimeInput {
                    profile,
                    actor_count: 0,
                    critical_mutations: vec![],
                    wal_ack_upto: None,
                    request_recovery: false,
                }
            }],
            &mut state,
        );
        assert!(recover.recovered_worlds.iter().any(|(world_id, _)| world_id == "world-1"));
    }

    #[test]
    fn wal_append_and_replay_with_idempotence() {
        let profile = WorldPersistenceProfile::from_defaults("world-2");
        let runtime = PersistenceRuntime::new(PersistenceRuntimeConfig::default());
        let mut state = PersistenceRuntimeState::default();

        let mutation = SyncStateMutation {
            key: CriticalStateKey::ScriptState("init".into()),
            payload: b"state@1".to_vec(),
            timestamp_ms: 100,
            actor_id: 8,
            priority: CriticalStatePriority::High,
        };

        let _ = runtime.step(
            RuntimeTickInput {
                tick: 1,
                now_ms: 100,
            },
            &[WorldRuntimeInput {
                profile: profile.clone(),
                actor_count: 1,
                critical_mutations: vec![mutation.clone(), mutation.clone()],
                wal_ack_upto: None,
                request_recovery: false,
            }],
            &mut state,
        );
        assert!(state
            .worlds
            .get("world-2")
            .is_some_and(|entry| entry.sync.pending_count() == 0));
    }

    #[test]
    fn placement_matches_stateful_requirements() {
        let profile = WorldPersistenceProfile {
            world_id: "world-3".into(),
            has_economy_state: true,
            expected_players: 20,
            ..WorldPersistenceProfile::from_defaults("world-3")
        };
        let runtime = PersistenceRuntime::default();
        let manifest = WorldManifest {
            world_id: profile.world_id.clone(),
            world_name: "test".into(),
            durability_class: profile.classify_pod(),
            p2p_enabled: true,
            expected_players: profile.expected_players,
            economy_enabled: profile.has_economy_state,
        };
        assert_eq!(runtime.resolve_placement(&manifest).pod_class, crate::PodRuntimeClass::StatefulSet);
    }
}
