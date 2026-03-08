use std::collections::HashMap;

use crate::batching::{batch_instances_by_key, BatchHint, BatchRequest};
use crate::config::{
    FrameBudget, FoveationConfig, FoveationTier, FrameContext, FramePolicy, LODLevel, ShadowCascadeConfig,
    StereoConfig, StreamPriority, StreamRequest,
};
use crate::scheduler::{FrameMode, FrameModeInput, FramePolicyReason, FrameScheduler};
use crate::stream::{ProgressiveMeshStreaming, StreamingProgress};

#[derive(Debug, Clone, Copy)]
pub enum RenderBackend {
    Wgpu,
    Vulkan,
    Metal,
    Mock,
}

impl RenderBackend {
    pub fn supports_multiview(self) -> bool {
        match self {
            RenderBackend::Wgpu => true,
            RenderBackend::Vulkan => true,
            RenderBackend::Metal => true,
            RenderBackend::Mock => false,
        }
    }

    pub fn supports_eye_tracking(self) -> bool {
        match self {
            RenderBackend::Wgpu => true,
            RenderBackend::Vulkan => true,
            RenderBackend::Metal => true,
            RenderBackend::Mock => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameRuntimeConfig {
    pub backend: RenderBackend,
    pub policy: FramePolicy,
    pub stream_budget_bytes: u32,
    pub frame_history_ms: u64,
}

impl Default for FrameRuntimeConfig {
    fn default() -> Self {
        Self {
            backend: RenderBackend::Mock,
            policy: FramePolicy::default(),
            stream_budget_bytes: 12_000,
            frame_history_ms: 2_000,
        }
    }
}

#[derive(Debug)]
pub struct FrameRuntimeState {
    pub frame_index: u64,
    pub last_mode: FrameMode,
    pub last_reason: FramePolicyReason,
    pub selected_tier: FoveationTier,
    pub last_eye_tracking_ms: u64,
    pub stream_levels: HashMap<(u64, u64), u8>,
    pub active_views: u8,
}

impl Default for FrameRuntimeState {
    fn default() -> Self {
        Self {
            frame_index: 0,
            last_mode: FrameMode::Balanced,
            last_reason: FramePolicyReason::BudgetPressure,
            selected_tier: FoveationTier::Tier2,
            last_eye_tracking_ms: 0,
            stream_levels: HashMap::new(),
            active_views: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameRuntimeInput {
    pub now_ms: u64,
    pub context: FrameContext,
    pub eye_tracking_distance_m: Option<f32>,
    pub stream_requests: Vec<StreamRequest>,
    pub batch_requests: Vec<BatchRequest>,
    pub material_bandwidth_bytes: Option<u32>,
}

#[derive(Debug)]
pub struct FrameBatchConfig {
    pub active_hints: Vec<BatchHint>,
    pub batches: usize,
    pub draw_calls: u64,
    pub visible_instances: u64,
}

#[derive(Debug)]
pub struct RenderModeDecision {
    pub mode: FrameMode,
    pub reason: FramePolicyReason,
    pub active_views: u8,
    pub use_multiview: bool,
    pub foveation_tier: FoveationTier,
    pub foveation_config: FoveationConfig,
    pub cluster_max_lights: u16,
    pub cascade_resolutions: [u32; 4],
    pub stream: Vec<StreamingProgress>,
    pub batches: FrameBatchConfig,
    pub estimated_ms: f32,
    pub budget_reason: FramePolicyReason,
}

#[derive(Debug)]
pub struct FrameOutput {
    pub decision: RenderModeDecision,
    pub workload_ms: f32,
}

#[derive(Debug)]
pub struct FrameRuntime {
    cfg: FrameRuntimeConfig,
    state: FrameRuntimeState,
    streamer: ProgressiveMeshStreaming,
}

impl Default for FrameRuntime {
    fn default() -> Self {
        Self::new(FrameRuntimeConfig::default())
    }
}

impl FrameRuntime {
    pub fn new(cfg: FrameRuntimeConfig) -> Self {
        let stream_budget = cfg.stream_budget_bytes;
        Self {
            cfg,
            state: FrameRuntimeState::default(),
            streamer: ProgressiveMeshStreaming::new(stream_budget),
        }
    }

    pub fn step(&mut self, input: FrameRuntimeInput) -> FrameOutput {
        self.state.frame_index = self.state.frame_index.saturating_add(1);
        let policy = self.cfg.policy;
        let runtime_state = self.make_base_frame_policy(&input, &policy);
        let _use_eye_tracking = input.eye_tracking_distance_m.is_some_and(|d| d.is_finite());

        let mode_input = FrameModeInput {
            context: input.context,
            base_policy: runtime_state.policy,
        };
        let (mode, reason, workload) = FrameScheduler::decide_mode(&mode_input);

        let batch = self.plan_batches(&input.batch_requests);
        let streaming = self.plan_streaming(
            input.now_ms,
            &input.stream_requests,
            input.material_bandwidth_bytes.unwrap_or(self.cfg.stream_budget_bytes),
        );
        let (cascade, cluster_lights) = self.plan_render_targets(
            &self.cfg.policy.shadow_cascades,
            &self.cfg.policy.clustered_lighting.max_lights_per_cluster,
            mode,
            workload.estimated_ms,
            input.context.gpu_ms as f64,
        );
        let budget_adjusted_reason = budget_reason(reason, input.context.gpu_ms as f64);

        self.state.last_mode = mode;
        self.state.last_reason = reason;
        self.state.active_views = runtime_state.views_per_frame;

        FrameOutput {
            workload_ms: workload.estimated_ms,
            decision: RenderModeDecision {
                mode,
                reason: budget_adjusted_reason,
                active_views: runtime_state.views_per_frame,
                use_multiview: runtime_state.use_multiview,
                foveation_tier: runtime_state.foveation_tier,
                foveation_config: runtime_state.foveation,
                cluster_max_lights: cluster_lights,
                cascade_resolutions: cascade,
                stream: streaming,
                batches: batch,
                estimated_ms: runtime_state.estimated_ms,
                budget_reason: budget_adjusted_reason,
            },
        }
    }

    pub fn state(&self) -> &FrameRuntimeState {
        &self.state
    }

    fn make_base_frame_policy(&mut self, input: &FrameRuntimeInput, policy: &FramePolicy) -> FramePolicyRuntime {
        let views_per_frame = if !self.cfg.backend.supports_multiview() || !self.cfg.backend.supports_eye_tracking() {
            1
        } else if policy.stereo.views_per_frame == 0 {
            self.default_views_for_mode()
        } else {
            policy.stereo.views_per_frame
        };

        let use_multiview = self.cfg.backend.supports_multiview() && views_per_frame > 1;
        let mut stereo = policy.stereo;
        stereo.views_per_frame = views_per_frame;
        stereo.multiview = use_multiview;

        let foveation = if let Some(distance_m) = input.eye_tracking_distance_m {
            self.state.last_eye_tracking_ms = input.now_ms;
            self.select_foveation(distance_m, policy.foveation)
        } else if input.now_ms.saturating_sub(self.state.last_eye_tracking_ms)
            > policy.foveation.smoothing_ms as u64
        {
            self.state.selected_tier = policy.foveation.tier;
            policy.foveation
        } else {
            let mut foveation = policy.foveation;
            foveation.tier = self.state.selected_tier;
            foveation
        };

        FramePolicyRuntime {
            policy: FramePolicy {
                stereo,
                foveation,
                clustered_lighting: policy.clustered_lighting,
                shadow_cascades: policy.shadow_cascades,
                lod: policy.lod,
                budget: policy.budget,
            },
            estimated_ms: Self::estimate_base_ms(&input.context),
            views_per_frame,
            use_multiview,
            foveation_tier: foveation.tier,
            foveation,
            budgeted_cluster: policy.clustered_lighting.max_lights_per_cluster,
            budget_reason: FramePolicyReason::BudgetPressure,
        }
    }

    fn estimate_base_ms(context: &FrameContext) -> f32 {
        (context.draw_calls as f32 * 0.0016) + (context.visible_entities as f32 * 0.0008) + 2.4
    }

    fn default_views_for_mode(&self) -> u8 {
        if matches!(self.cfg.backend, RenderBackend::Mock) {
            1
        } else {
            2
        }
    }

    fn select_foveation(&mut self, eye_tracking_distance_m: f32, mut foveation: FoveationConfig) -> FoveationConfig {
        let adaptive = if eye_tracking_distance_m <= foveation.max_radius_m {
            FoveationTier::Adaptive
        } else if eye_tracking_distance_m <= foveation.max_radius_m * 1.8 {
            FoveationTier::Tier2
        } else {
            FoveationTier::Tier1
        };

        self.state.selected_tier = if self.state.selected_tier == FoveationTier::Off {
            adaptive
        } else {
            match (self.state.selected_tier, adaptive) {
                (FoveationTier::Off, _) => adaptive,
                (FoveationTier::Tier1, FoveationTier::Tier2 | FoveationTier::Adaptive) => adaptive,
                (FoveationTier::Tier2, FoveationTier::Adaptive) => adaptive,
                (FoveationTier::Adaptive, FoveationTier::Adaptive) => adaptive,
                (_, FoveationTier::Off) => self.state.selected_tier,
                (FoveationTier::Tier1, FoveationTier::Adaptive) => adaptive,
                (FoveationTier::Adaptive, FoveationTier::Tier1 | FoveationTier::Tier2) => adaptive,
                _ => adaptive,
            }
        };
        foveation.tier = self.state.selected_tier;
        foveation
    }

    fn plan_render_targets(
        &self,
        shadow: &ShadowCascadeConfig,
        max_lights: &u16,
        mode: FrameMode,
        estimated_ms: f32,
        gpu_ms: f64,
    ) -> ([u32; 4], u16) {
        let mut cascades = FrameScheduler::cascade_resolution_budget(
            shadow,
            (self.cfg.stream_budget_bytes + u32::try_from(gpu_ms as u32).unwrap_or(0)).max(1) as u64,
        );
        if mode == FrameMode::Safe || estimated_ms > shadow.far_distance_m {
            cascades[3] = cascades[3].max(256);
            cascades[2] = cascades[2].max(384);
        }
        let cluster_lights = if mode == FrameMode::Safe {
            max_lights.saturating_div(2).max(4)
        } else {
            *max_lights
        };
        (cascades, cluster_lights)
    }

    fn plan_batches(&self, requests: &[BatchRequest]) -> FrameBatchConfig {
        let mut hints = batch_instances_by_key(requests);
        let draw_calls = hints.iter().map(|hint| hint.instances.len() as u64).sum::<u64>();
        let visible = hints.len() as u64;
        FrameBatchConfig {
            active_hints: hints,
            batches: visible as usize,
            draw_calls,
            visible_instances: requests.len() as u64,
        }
    }

    fn plan_streaming(
        &mut self,
        now_ms: u64,
        requests: &[StreamRequest],
        budget: u32,
    ) -> Vec<StreamingProgress> {
        let _ = now_ms;
        let mut stream_results = Vec::new();
        for request in requests {
            let current = self
                .state
                .stream_levels
                .entry((request.world_id, request.script_id))
                .or_insert(request.requested_level);
            let effective = match self.streamer.choose_next_level(request, *current, budget) {
                Ok(level) => {
                    *current = level;
                    level
                }
                Err(_) => *current,
            };
            stream_results.push(StreamingProgress {
                world_id: request.world_id,
                script_id: request.script_id,
                bytes_loaded: request.bytes.saturating_mul(u32::from(effective) + 1),
                current_level: effective,
            });
        }
        stream_results
    }
}

#[derive(Debug)]
struct FramePolicyRuntime {
    policy: FramePolicy,
    estimated_ms: f32,
    views_per_frame: u8,
    use_multiview: bool,
    foveation_tier: FoveationTier,
    foveation: FoveationConfig,
    budgeted_cluster: u16,
    budget_reason: FramePolicyReason,
}

fn budget_reason(current: FramePolicyReason, gpu_ms: f64) -> FramePolicyReason {
    if gpu_ms > 90.0 {
        FramePolicyReason::ThermalGuard
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_backend_clears_multiview() {
        let mut runtime = FrameRuntime::new(FrameRuntimeConfig {
            backend: RenderBackend::Mock,
            policy: FramePolicy::default(),
            stream_budget_bytes: 3_000,
            frame_history_ms: 2_000,
        });
        let out = runtime.step(FrameRuntimeInput {
            now_ms: 1_000,
            context: FrameContext {
                fps: 90,
                draw_calls: 2000,
                visible_entities: 4_000,
                gpu_ms: 30.0,
            },
            eye_tracking_distance_m: None,
            stream_requests: vec![],
            batch_requests: vec![],
            material_bandwidth_bytes: Some(2_000),
        });
        assert_eq!(out.decision.active_views, 1);
        assert!(!out.decision.use_multiview);
    }

    #[test]
    fn eye_tracking_switches_foveation_to_adaptive() {
        let mut runtime = FrameRuntime::default();
        let policy = FramePolicy::default();
        let out = runtime.step(FrameRuntimeInput {
            now_ms: 1000,
            context: FrameContext {
                fps: 60,
                draw_calls: 1000,
                visible_entities: 1200,
                gpu_ms: 45.0,
            },
            eye_tracking_distance_m: Some(0.6),
            stream_requests: vec![],
            batch_requests: vec![],
            material_bandwidth_bytes: Some(4_000),
        });
        assert_eq!(out.decision.foveation_tier, FoveationTier::Adaptive);
        assert!(matches!(out.decision.foveation_tier, FoveationTier::Adaptive));
        assert_eq!(out.decision.foveation_config.tier, FoveationTier::Adaptive);
        assert_eq!(policy.shadow_cascades.num_cascades > 0, true);
        let _ = policy; // no-op to avoid lint warnings
    }

    #[test]
    fn batch_and_streaming_are_derived() {
        let mut runtime = FrameRuntime::default();
        let mut keys = vec![];
        for i in 0..3 {
            keys.push(BatchRequest {
                entity_id: i,
                key: crate::MaterialBatchKey {
                    mesh_id: 10,
                    material_id: 5,
                    pass_id: 2,
                    blend_mode: 1,
                },
            });
        }
        let out = runtime.step(FrameRuntimeInput {
            now_ms: 1,
            context: FrameContext {
                fps: 60,
                draw_calls: 1_000,
                visible_entities: 200,
                gpu_ms: 10.0,
            },
            eye_tracking_distance_m: None,
            stream_requests: vec![StreamRequest {
                world_id: 1,
                script_id: 1,
                requested_level: 2,
                bytes: 1_000,
                priority: StreamPriority::High,
            }],
            batch_requests: keys,
            material_bandwidth_bytes: Some(2_000),
        });
        assert_eq!(out.decision.batches.batches, 1);
        assert_eq!(out.decision.batches.visible_instances, 3);
        assert_eq!(out.decision.stream.len(), 1);
    }
}
