//! Performance budget validation.
//!
//! Defines per-platform rendering budgets and provides a mechanism
//! to check current usage against the budget, producing a detailed report.

use serde::{Deserialize, Serialize};

use crate::detection::Platform;
use crate::profiles::QualityProfile;

// Frame time budgets in milliseconds per platform.
const PCVR_FRAME_TIME_BUDGET_MS: f32 = 11.1; // 90 Hz
const QUEST_FRAME_TIME_BUDGET_MS: f32 = 13.8; // 72 Hz
const DESKTOP_FRAME_TIME_BUDGET_MS: f32 = 16.6; // 60 Hz
const WEB_FRAME_TIME_BUDGET_MS: f32 = 16.6; // 60 Hz

/// Performance budget for a specific platform.
///
/// Combines the quality profile polygon/draw call limits with
/// a per-frame time budget derived from the target refresh rate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceBudget {
    /// Maximum polygons per eye per frame.
    pub max_polygons_per_eye: u32,
    /// Maximum draw calls per frame.
    pub max_draw_calls: u32,
    /// Maximum texture memory in MB.
    pub max_texture_memory_mb: u32,
    /// Frame time budget in milliseconds.
    pub frame_time_budget_ms: f32,
}

impl PerformanceBudget {
    /// Create a performance budget from a quality profile and target frame time.
    pub fn from_profile(profile: &QualityProfile, frame_time_budget_ms: f32) -> Self {
        Self {
            max_polygons_per_eye: profile.max_polygons_per_eye,
            max_draw_calls: profile.max_draw_calls,
            max_texture_memory_mb: profile.max_texture_memory_mb,
            frame_time_budget_ms,
        }
    }

    /// Returns the default performance budget for a given platform.
    pub fn for_platform(platform: Platform) -> Self {
        let profile = QualityProfile::for_platform(platform);
        let frame_time = match platform {
            Platform::PcVr => PCVR_FRAME_TIME_BUDGET_MS,
            Platform::QuestStandalone => QUEST_FRAME_TIME_BUDGET_MS,
            Platform::Desktop => DESKTOP_FRAME_TIME_BUDGET_MS,
            Platform::WebBrowser => WEB_FRAME_TIME_BUDGET_MS,
        };
        Self::from_profile(&profile, frame_time)
    }

    /// Check current usage against this budget and produce a report.
    pub fn check(&self, usage: &BudgetUsage) -> BudgetReport {
        let polygon_ok = usage.polygons_per_eye <= self.max_polygons_per_eye;
        let draw_calls_ok = usage.draw_calls <= self.max_draw_calls;
        let texture_memory_ok = usage.texture_memory_mb <= self.max_texture_memory_mb;
        let frame_time_ok = usage.frame_time_ms <= self.frame_time_budget_ms;

        BudgetReport {
            polygon_ok,
            draw_calls_ok,
            texture_memory_ok,
            frame_time_ok,
            polygon_usage_pct: percentage(usage.polygons_per_eye, self.max_polygons_per_eye),
            draw_call_usage_pct: percentage(usage.draw_calls, self.max_draw_calls),
            texture_memory_usage_pct: percentage(usage.texture_memory_mb, self.max_texture_memory_mb),
            frame_time_usage_pct: if self.frame_time_budget_ms > 0.0 {
                (usage.frame_time_ms / self.frame_time_budget_ms) * 100.0
            } else {
                0.0
            },
        }
    }
}

/// Current resource usage to check against a budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetUsage {
    /// Current polygon count per eye.
    pub polygons_per_eye: u32,
    /// Current draw call count.
    pub draw_calls: u32,
    /// Current texture memory usage in MB.
    pub texture_memory_mb: u32,
    /// Current frame time in milliseconds.
    pub frame_time_ms: f32,
}

impl Default for BudgetUsage {
    fn default() -> Self {
        Self {
            polygons_per_eye: 0,
            draw_calls: 0,
            texture_memory_mb: 0,
            frame_time_ms: 0.0,
        }
    }
}

/// Result of checking usage against a performance budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetReport {
    /// Whether polygon count is within budget.
    pub polygon_ok: bool,
    /// Whether draw call count is within budget.
    pub draw_calls_ok: bool,
    /// Whether texture memory is within budget.
    pub texture_memory_ok: bool,
    /// Whether frame time is within budget.
    pub frame_time_ok: bool,
    /// Polygon usage as a percentage of budget.
    pub polygon_usage_pct: f32,
    /// Draw call usage as a percentage of budget.
    pub draw_call_usage_pct: f32,
    /// Texture memory usage as a percentage of budget.
    pub texture_memory_usage_pct: f32,
    /// Frame time usage as a percentage of budget.
    pub frame_time_usage_pct: f32,
}

impl BudgetReport {
    /// Returns true if all metrics are within budget.
    pub fn is_within_budget(&self) -> bool {
        self.polygon_ok && self.draw_calls_ok && self.texture_memory_ok && self.frame_time_ok
    }

    /// Returns a list of the metrics that are over budget.
    pub fn violations(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if !self.polygon_ok {
            v.push("polygons_per_eye");
        }
        if !self.draw_calls_ok {
            v.push("draw_calls");
        }
        if !self.texture_memory_ok {
            v.push("texture_memory_mb");
        }
        if !self.frame_time_ok {
            v.push("frame_time_ms");
        }
        v
    }
}

fn percentage(value: u32, max: u32) -> f32 {
    if max == 0 {
        return 0.0;
    }
    (value as f32 / max as f32) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_budget_values() {
        let budget = PerformanceBudget::for_platform(Platform::QuestStandalone);
        assert_eq!(budget.max_polygons_per_eye, 500_000);
        assert_eq!(budget.max_draw_calls, 500);
        assert_eq!(budget.max_texture_memory_mb, 1_024);
        assert!((budget.frame_time_budget_ms - 13.8).abs() < 0.1);
    }

    #[test]
    fn pcvr_budget_values() {
        let budget = PerformanceBudget::for_platform(Platform::PcVr);
        assert_eq!(budget.max_polygons_per_eye, 2_000_000);
        assert!((budget.frame_time_budget_ms - 11.1).abs() < 0.1);
    }

    #[test]
    fn usage_within_budget() {
        let budget = PerformanceBudget::for_platform(Platform::QuestStandalone);
        let usage = BudgetUsage {
            polygons_per_eye: 400_000,
            draw_calls: 400,
            texture_memory_mb: 900,
            frame_time_ms: 12.0,
        };
        let report = budget.check(&usage);
        assert!(report.is_within_budget());
        assert!(report.violations().is_empty());
    }

    #[test]
    fn usage_over_budget_polygons() {
        let budget = PerformanceBudget::for_platform(Platform::QuestStandalone);
        let usage = BudgetUsage {
            polygons_per_eye: 600_000,
            draw_calls: 400,
            texture_memory_mb: 900,
            frame_time_ms: 12.0,
        };
        let report = budget.check(&usage);
        assert!(!report.is_within_budget());
        assert!(!report.polygon_ok);
        assert!(report.draw_calls_ok);
        assert_eq!(report.violations(), vec!["polygons_per_eye"]);
    }

    #[test]
    fn usage_over_budget_draw_calls() {
        let budget = PerformanceBudget::for_platform(Platform::QuestStandalone);
        let usage = BudgetUsage {
            polygons_per_eye: 400_000,
            draw_calls: 600,
            texture_memory_mb: 900,
            frame_time_ms: 12.0,
        };
        let report = budget.check(&usage);
        assert!(!report.is_within_budget());
        assert!(report.polygon_ok);
        assert!(!report.draw_calls_ok);
        assert_eq!(report.violations(), vec!["draw_calls"]);
    }

    #[test]
    fn usage_over_budget_texture_memory() {
        let budget = PerformanceBudget::for_platform(Platform::QuestStandalone);
        let usage = BudgetUsage {
            polygons_per_eye: 400_000,
            draw_calls: 400,
            texture_memory_mb: 2_000,
            frame_time_ms: 12.0,
        };
        let report = budget.check(&usage);
        assert!(!report.is_within_budget());
        assert!(!report.texture_memory_ok);
        assert_eq!(report.violations(), vec!["texture_memory_mb"]);
    }

    #[test]
    fn usage_over_budget_frame_time() {
        let budget = PerformanceBudget::for_platform(Platform::QuestStandalone);
        let usage = BudgetUsage {
            polygons_per_eye: 400_000,
            draw_calls: 400,
            texture_memory_mb: 900,
            frame_time_ms: 20.0,
        };
        let report = budget.check(&usage);
        assert!(!report.is_within_budget());
        assert!(!report.frame_time_ok);
        assert_eq!(report.violations(), vec!["frame_time_ms"]);
    }

    #[test]
    fn usage_exactly_at_limit() {
        let budget = PerformanceBudget::for_platform(Platform::QuestStandalone);
        let usage = BudgetUsage {
            polygons_per_eye: 500_000,
            draw_calls: 500,
            texture_memory_mb: 1_024,
            frame_time_ms: budget.frame_time_budget_ms,
        };
        let report = budget.check(&usage);
        assert!(report.is_within_budget());
        assert!(report.violations().is_empty());
    }

    #[test]
    fn usage_one_over_limit() {
        let budget = PerformanceBudget::for_platform(Platform::QuestStandalone);
        let usage = BudgetUsage {
            polygons_per_eye: 500_001,
            draw_calls: 500,
            texture_memory_mb: 1_024,
            frame_time_ms: budget.frame_time_budget_ms,
        };
        let report = budget.check(&usage);
        assert!(!report.is_within_budget());
        assert!(!report.polygon_ok);
    }

    #[test]
    fn multiple_violations() {
        let budget = PerformanceBudget::for_platform(Platform::QuestStandalone);
        let usage = BudgetUsage {
            polygons_per_eye: 600_000,
            draw_calls: 600,
            texture_memory_mb: 2_000,
            frame_time_ms: 20.0,
        };
        let report = budget.check(&usage);
        assert!(!report.is_within_budget());
        assert_eq!(report.violations().len(), 4);
    }

    #[test]
    fn zero_usage_within_budget() {
        let budget = PerformanceBudget::for_platform(Platform::QuestStandalone);
        let usage = BudgetUsage::default();
        let report = budget.check(&usage);
        assert!(report.is_within_budget());
        assert!((report.polygon_usage_pct - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn percentage_calculation() {
        let budget = PerformanceBudget::for_platform(Platform::QuestStandalone);
        let usage = BudgetUsage {
            polygons_per_eye: 250_000,
            draw_calls: 250,
            texture_memory_mb: 512,
            frame_time_ms: budget.frame_time_budget_ms / 2.0,
        };
        let report = budget.check(&usage);
        assert!((report.polygon_usage_pct - 50.0).abs() < 0.01);
        assert!((report.draw_call_usage_pct - 50.0).abs() < 0.01);
        assert!((report.texture_memory_usage_pct - 50.0).abs() < 0.01);
        assert!((report.frame_time_usage_pct - 50.0).abs() < 0.01);
    }

    #[test]
    fn from_profile_custom_frame_time() {
        let profile = QualityProfile::quest();
        let budget = PerformanceBudget::from_profile(&profile, 8.3);
        assert_eq!(budget.max_polygons_per_eye, 500_000);
        assert!((budget.frame_time_budget_ms - 8.3).abs() < 0.01);
    }

    #[test]
    fn budget_clone_eq() {
        let b1 = PerformanceBudget::for_platform(Platform::Desktop);
        let b2 = b1.clone();
        assert_eq!(b1, b2);
    }
}
