use crate::config::{ClusterLightingConfig, FrameContext, FramePolicy, FoveationConfig, FoveationTier, LODLevel, LODPolicy, LodCurve, ShadowCascadeConfig, FrameBudget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBucket {
    Comfortable,
    Elevated,
    Constrained,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameMode {
    Ultra,
    High,
    Balanced,
    Safe,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameWorkload {
    pub estimated_ms: f32,
    pub multiview_gain: f32,
    pub bucket: LoadBucket,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameModeInput {
    pub context: FrameContext,
    pub base_policy: FramePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePolicyReason {
    BudgetAvailable,
    BudgetPressure,
    Overloaded,
    ThermalGuard,
}

#[derive(Debug)]
pub struct FrameScheduler;

impl FrameScheduler {
    pub fn estimate_workload(context: &FrameContext, policy: &FramePolicy) -> FrameWorkload {
        let base_cost = (context.draw_calls as f32 * 0.00015)
            + (context.visible_entities as f32 * 0.00002)
            + (context.fps as f32 * 0.0)
            + 1.5;
        let multiview_gain = if policy.stereo.multiview { 0.85 } else { 1.0 };
        let foveation_gain = match policy.foveation.tier {
            FoveationTier::Off => 1.0,
            FoveationTier::Tier1 => 1.1,
            FoveationTier::Tier2 => 1.2,
            FoveationTier::Adaptive => 1.25,
        };
        let est = (base_cost / multiview_gain) / foveation_gain;

        let bucket = if est < policy.budget.target_ms * 0.55 {
            LoadBucket::Comfortable
        } else if est < policy.budget.target_ms * 0.78 {
            LoadBucket::Elevated
        } else if est < policy.budget.target_ms * 0.95 {
            LoadBucket::Constrained
        } else {
            LoadBucket::Critical
        };

        FrameWorkload {
            estimated_ms: est,
            multiview_gain,
            bucket,
        }
    }

    pub fn decide_mode(input: &FrameModeInput) -> (FrameMode, FramePolicyReason, FrameWorkload) {
        let workload = Self::estimate_workload(&input.context, &input.base_policy);
        let headroom = workload.estimated_ms / input.base_policy.budget.target_ms;
        let thermal_pressure = input.context.gpu_ms > 100.0;

        let (mode, reason) = if thermal_pressure {
            (FrameMode::Safe, FramePolicyReason::ThermalGuard)
        } else {
            match workload.bucket {
                LoadBucket::Comfortable => (FrameMode::Ultra, FramePolicyReason::BudgetAvailable),
                LoadBucket::Elevated => (FrameMode::High, FramePolicyReason::BudgetAvailable),
                LoadBucket::Constrained => {
                    if headroom < 0.9 {
                        (FrameMode::Balanced, FramePolicyReason::BudgetPressure)
                    } else {
                        (FrameMode::High, FramePolicyReason::BudgetPressure)
                    }
                }
                LoadBucket::Critical => {
                    if headroom < 0.95 {
                        (FrameMode::Safe, FramePolicyReason::Overloaded)
                    } else {
                        (FrameMode::Balanced, FramePolicyReason::BudgetPressure)
                    }
                }
            }
        };

        (mode, reason, workload)
    }

    pub fn cascade_resolution_budget(cfg: &ShadowCascadeConfig, target_bytes: u64) -> [u32; 4] {
        let mut out = cfg.cascade_resolutions;
        let total: u64 = out.iter().map(|r| (*r as u64).saturating_mul(*r as u64)).sum();
        if total <= target_bytes {
            return out;
        }
        let down_scale = (target_bytes as f64 / total as f64).sqrt();
        for entry in out.iter_mut() {
            let scaled = ((*entry as f64) * down_scale).floor() as u64;
            *entry = scaled.max(128) as u32;
        }
        out
    }
}

pub fn decide_frame_mode(input: FrameModeInput) -> FrameMode {
    FrameScheduler::decide_mode(&input).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FrameContext, FramePolicy, ShadowCascadeConfig};

    #[test]
    fn select_higher_profile_when_workload_is_low() {
        let input = FrameModeInput {
            context: FrameContext {
                fps: 120,
                draw_calls: 1_500,
                visible_entities: 3500,
                gpu_ms: 40.0,
            },
            base_policy: FramePolicy::default(),
        };
        let (mode, reason, workload) = FrameScheduler::decide_mode(&input);
        assert_eq!(reason, FramePolicyReason::BudgetAvailable);
        assert!(matches!(mode, FrameMode::High | FrameMode::Ultra));
        assert!(workload.estimated_ms > 0.0);
    }

    #[test]
    fn cascade_budget_clamps_resolutions_monotonically() {
        let policy = ShadowCascadeConfig::default();
        let res = FrameScheduler::cascade_resolution_budget(&policy, 4_000_000);
        assert!(res[0] <= policy.cascade_resolutions[0]);
        assert!(res[1] <= policy.cascade_resolutions[1]);
        assert!(res[2] <= policy.cascade_resolutions[2]);
        assert!(res[3] <= policy.cascade_resolutions[3]);
    }
}
