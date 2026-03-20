use std::collections::{HashMap, VecDeque};

use crate::ghost::GhostCache;
use crate::{
    AxisChoice, GhostEntity, GhostPolicy, HandoffDecision, HandoffFailureMode, HandoffResult,
    KdBoundary, KdPoint, KdTree, KdTreeSplitResult, LoadMetrics, MergeThreshold, SplitPolicy,
    SplitResult, ZoneSpec,
};

#[derive(Debug)]
pub struct ZoningRuntimeConfig {
    pub split_policy: SplitPolicy,
    pub merge_threshold: MergeThreshold,
    pub handoff_ttl_ms: u64,
    pub handoff_grace_ms: u64,
    pub ghost_policy: GhostPolicy,
    pub max_zones: u32,
}

impl Default for ZoningRuntimeConfig {
    fn default() -> Self {
        Self {
            split_policy: SplitPolicy {
                preferred_axes: vec![AxisChoice::X, AxisChoice::Z],
                max_depth: 3,
            },
            merge_threshold: MergeThreshold {
                merge_player_threshold: 12,
                merge_hold_ms: 4_000,
            },
            handoff_ttl_ms: 8_000,
            handoff_grace_ms: 600,
            ghost_policy: GhostPolicy {
                ttl_ms: 1_200,
                max_ghosts_per_connection: 64,
                visibility: crate::ghost::GhostVisibilityScope::DistanceCapped {
                    max_distance_m: 35.0,
                },
            },
            max_zones: 32,
        }
    }
}

#[derive(Debug)]
pub struct ZoningRuntimeInput {
    pub now_ms: u64,
    pub zones: Vec<ZoneSpec>,
    pub load_samples: Vec<LoadMetrics>,
}

#[derive(Debug)]
pub struct ZoningRuntimeOutput {
    pub now_ms: u64,
    pub split_decisions: Vec<SplitResult>,
    pub merge_decisions: Vec<SplitResult>,
    pub handoffs: Vec<HandoffResult>,
    pub ghost_queue: Vec<GhostEntity>,
}

#[derive(Debug, Default)]
pub struct ZoningRuntimeState {
    pub zone_tree: Vec<KdTree>,
    pub zone_load: HashMap<String, LoadMetrics>,
    pub ghost_cache: GhostCache,
    pub pending_handoffs: VecDeque<HandoffDecision>,
}

#[derive(Debug)]
pub struct ZoningRuntime {
    cfg: ZoningRuntimeConfig,
    state: ZoningRuntimeState,
}

impl Default for ZoningRuntime {
    fn default() -> Self {
        Self::new(ZoningRuntimeConfig::default())
    }
}

impl ZoningRuntime {
    pub fn new(cfg: ZoningRuntimeConfig) -> Self {
        Self {
            cfg,
            state: ZoningRuntimeState::default(),
        }
    }

    pub fn state(&self) -> &ZoningRuntimeState {
        &self.state
    }

    pub fn step(&mut self, input: ZoningRuntimeInput) -> ZoningRuntimeOutput {
        let mut output = ZoningRuntimeOutput {
            now_ms: input.now_ms,
            split_decisions: Vec::new(),
            merge_decisions: Vec::new(),
            handoffs: Vec::new(),
            ghost_queue: Vec::new(),
        };
        self.state.zone_load.clear();

        for zone in &input.zones {
            let boundary = KdBoundary {
                min: KdPoint {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                max: KdPoint {
                    x: 128.0,
                    y: 128.0,
                    z: 128.0,
                },
            };
            let tree = KdTree::new(zone.zone_index.clone(), boundary, &self.cfg.split_policy);
            self.state.zone_tree.push(tree);
        }

        for sample in input.load_samples {
            let too_large = sample.players > self.cfg.split_policy.preferred_axes.len() as u32
                && sample.players > self.cfg.merge_threshold.merge_player_threshold;
            if too_large {
                output.split_decisions.push(self.evaluate_split(&sample));
            } else if sample.players < self.cfg.merge_threshold.merge_player_threshold
                && self.state.zone_tree.len() > 1
            {
                output.merge_decisions.push(self.evaluate_merge(&sample));
            }
            self.state.zone_load.insert(sample.zone_id.clone(), sample);
        }

        for decision in self.state.pending_handoffs.drain(..).collect::<Vec<_>>() {
            output
                .handoffs
                .push(self.execute_handoff(input.now_ms, decision));
        }

        self.state.ghost_cache.cull_expired(input.now_ms);
        output.ghost_queue = self
            .state
            .ghost_cache
            .as_identities()
            .into_iter()
            .map(|identity| GhostEntity {
                source_entity: identity.entity_id,
                local_entity: identity.entity_id,
                source_zone: identity.authority_zone.clone(),
                remote_zone: identity.authority_zone,
                ttl_ms: self.cfg.ghost_policy.ttl_ms,
                collision_enabled: false,
                render_only: true,
            })
            .collect();
        output
    }

    pub fn enqueue_handoff(&mut self, decision: HandoffDecision) {
        self.state.pending_handoffs.push_back(decision);
    }

    fn evaluate_split(&self, sample: &LoadMetrics) -> SplitResult {
        let axis = if let Some(axis) = self.cfg.split_policy.preferred_axes.first() {
            axis.clone()
        } else {
            AxisChoice::X
        };
        let mut tree = KdTree::new(
            sample.zone_id.clone(),
            KdBoundary {
                min: KdPoint {
                    x: -64.0,
                    y: -64.0,
                    z: -64.0,
                },
                max: KdPoint {
                    x: 64.0,
                    y: 64.0,
                    z: 64.0,
                },
            },
            &self.cfg.split_policy,
        );
        let samples = vec![];
        match tree.split_if_needed(&sample.zone_id, &samples, &self.cfg.split_policy) {
            Some(KdTreeSplitResult::SplitDone {
                axis: _,
                left_count,
                right_count,
                ..
            }) => SplitResult::SplitOk {
                left: ZoneSpec {
                    world_id: "world".into(),
                    shard_key: format!("{}-A", sample.zone_id),
                    zone_index: format!("{}-L{}", sample.zone_id, left_count),
                },
                right: ZoneSpec {
                    world_id: "world".into(),
                    shard_key: format!("{}-B", sample.zone_id),
                    zone_index: format!("{}-R{}", sample.zone_id, right_count),
                },
                axis,
            },
            _ => SplitResult::Unchanged,
        }
    }

    fn evaluate_merge(&self, _sample: &LoadMetrics) -> SplitResult {
        SplitResult::Unchanged
    }

    fn execute_handoff(&self, now_ms: u64, decision: HandoffDecision) -> HandoffResult {
        if decision.sequence == 0 {
            return HandoffResult::Rejected {
                player_id: decision.player_id,
                reason: HandoffFailureMode::AuthorityMismatch,
            };
        }
        if decision.timeout_ms < 1 {
            return HandoffResult::Rejected {
                player_id: decision.player_id,
                reason: HandoffFailureMode::PlayerDisconnect,
            };
        }
        if now_ms.saturating_sub(decision.sequence) > self.cfg.handoff_ttl_ms {
            return HandoffResult::Rejected {
                player_id: decision.player_id,
                reason: HandoffFailureMode::Timeout,
            };
        }
        HandoffResult::Accepted {
            player_id: decision.player_id,
            from: decision.source_zone,
            to: decision.target_zone,
            applied_sequence: decision.sequence,
        }
    }
}
