//! Platform hardware capabilities.
//!
//! Reports GPU tier, memory limits, and hardware feature support
//! for each target platform.

use serde::{Deserialize, Serialize};

use crate::detection::Platform;

// Default memory limits per platform (MB).
const PCVR_MAX_MEMORY_MB: u32 = 16_384;
const QUEST_MAX_MEMORY_MB: u32 = 6_144;
const DESKTOP_MAX_MEMORY_MB: u32 = 8_192;
const WEB_MAX_MEMORY_MB: u32 = 2_048;

/// GPU performance tier classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GpuTier {
    /// Low-end (mobile, integrated).
    Low,
    /// Mid-range (mainstream discrete).
    Medium,
    /// High-end (enthusiast discrete).
    High,
    /// Ultra (workstation / top-tier).
    Ultra,
}

impl GpuTier {
    /// Returns a relative numeric score for comparison (higher = better).
    pub fn score(&self) -> u32 {
        match self {
            GpuTier::Low => 1,
            GpuTier::Medium => 2,
            GpuTier::High => 3,
            GpuTier::Ultra => 4,
        }
    }
}

/// Hardware capabilities available on a specific platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformCapabilities {
    /// Whether the platform supports VR rendering.
    pub supports_vr: bool,
    /// Whether the platform supports hand tracking input.
    pub supports_hand_tracking: bool,
    /// Whether the platform supports eye/gaze tracking.
    pub supports_eye_tracking: bool,
    /// Whether the platform can run client-side WASM scripts.
    pub supports_wasm_client: bool,
    /// Maximum available system memory in MB.
    pub max_memory_mb: u32,
    /// GPU performance tier.
    pub gpu_tier: GpuTier,
}

impl PlatformCapabilities {
    /// Returns the default capabilities for a given platform.
    pub fn for_platform(platform: Platform) -> Self {
        match platform {
            Platform::PcVr => Self {
                supports_vr: true,
                supports_hand_tracking: true,
                supports_eye_tracking: true,
                supports_wasm_client: true,
                max_memory_mb: PCVR_MAX_MEMORY_MB,
                gpu_tier: GpuTier::High,
            },
            Platform::QuestStandalone => Self {
                supports_vr: true,
                supports_hand_tracking: true,
                supports_eye_tracking: true,
                supports_wasm_client: true,
                max_memory_mb: QUEST_MAX_MEMORY_MB,
                gpu_tier: GpuTier::Low,
            },
            Platform::Desktop => Self {
                supports_vr: false,
                supports_hand_tracking: false,
                supports_eye_tracking: false,
                supports_wasm_client: true,
                max_memory_mb: DESKTOP_MAX_MEMORY_MB,
                gpu_tier: GpuTier::Medium,
            },
            Platform::WebBrowser => Self {
                supports_vr: false,
                supports_hand_tracking: false,
                supports_eye_tracking: false,
                supports_wasm_client: true,
                max_memory_mb: WEB_MAX_MEMORY_MB,
                gpu_tier: GpuTier::Low,
            },
        }
    }

    /// Check whether this platform has at least the given GPU tier.
    pub fn meets_gpu_tier(&self, minimum: GpuTier) -> bool {
        self.gpu_tier.score() >= minimum.score()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcvr_capabilities() {
        let caps = PlatformCapabilities::for_platform(Platform::PcVr);
        assert!(caps.supports_vr);
        assert!(caps.supports_hand_tracking);
        assert!(caps.supports_eye_tracking);
        assert!(caps.supports_wasm_client);
        assert_eq!(caps.max_memory_mb, 16_384);
        assert_eq!(caps.gpu_tier, GpuTier::High);
    }

    #[test]
    fn quest_capabilities() {
        let caps = PlatformCapabilities::for_platform(Platform::QuestStandalone);
        assert!(caps.supports_vr);
        assert!(caps.supports_hand_tracking);
        assert!(caps.supports_eye_tracking);
        assert!(caps.supports_wasm_client);
        assert_eq!(caps.max_memory_mb, 6_144);
        assert_eq!(caps.gpu_tier, GpuTier::Low);
    }

    #[test]
    fn desktop_capabilities() {
        let caps = PlatformCapabilities::for_platform(Platform::Desktop);
        assert!(!caps.supports_vr);
        assert!(!caps.supports_hand_tracking);
        assert!(!caps.supports_eye_tracking);
        assert!(caps.supports_wasm_client);
        assert_eq!(caps.max_memory_mb, 8_192);
        assert_eq!(caps.gpu_tier, GpuTier::Medium);
    }

    #[test]
    fn web_capabilities() {
        let caps = PlatformCapabilities::for_platform(Platform::WebBrowser);
        assert!(!caps.supports_vr);
        assert!(!caps.supports_hand_tracking);
        assert!(!caps.supports_eye_tracking);
        assert!(caps.supports_wasm_client);
        assert_eq!(caps.max_memory_mb, 2_048);
        assert_eq!(caps.gpu_tier, GpuTier::Low);
    }

    #[test]
    fn vr_platforms_support_vr() {
        assert!(PlatformCapabilities::for_platform(Platform::PcVr).supports_vr);
        assert!(PlatformCapabilities::for_platform(Platform::QuestStandalone).supports_vr);
    }

    #[test]
    fn non_vr_platforms_do_not_support_vr() {
        assert!(!PlatformCapabilities::for_platform(Platform::Desktop).supports_vr);
        assert!(!PlatformCapabilities::for_platform(Platform::WebBrowser).supports_vr);
    }

    #[test]
    fn all_platforms_support_wasm() {
        for platform in crate::detection::all_platforms() {
            let caps = PlatformCapabilities::for_platform(*platform);
            assert!(caps.supports_wasm_client, "{:?} should support wasm", platform);
        }
    }

    #[test]
    fn quest_has_least_memory() {
        let quest = PlatformCapabilities::for_platform(Platform::QuestStandalone);
        let web = PlatformCapabilities::for_platform(Platform::WebBrowser);
        // Web has less than Quest in our model
        assert!(web.max_memory_mb < quest.max_memory_mb);
    }

    #[test]
    fn pcvr_has_most_memory() {
        let pcvr = PlatformCapabilities::for_platform(Platform::PcVr);
        for platform in crate::detection::all_platforms() {
            let caps = PlatformCapabilities::for_platform(*platform);
            assert!(
                pcvr.max_memory_mb >= caps.max_memory_mb,
                "pcvr should have >= memory than {:?}",
                platform
            );
        }
    }

    #[test]
    fn gpu_tier_score_ordering() {
        assert!(GpuTier::Low.score() < GpuTier::Medium.score());
        assert!(GpuTier::Medium.score() < GpuTier::High.score());
        assert!(GpuTier::High.score() < GpuTier::Ultra.score());
    }

    #[test]
    fn meets_gpu_tier_check() {
        let caps = PlatformCapabilities::for_platform(Platform::PcVr);
        assert!(caps.meets_gpu_tier(GpuTier::Low));
        assert!(caps.meets_gpu_tier(GpuTier::Medium));
        assert!(caps.meets_gpu_tier(GpuTier::High));
        assert!(!caps.meets_gpu_tier(GpuTier::Ultra));
    }

    #[test]
    fn quest_does_not_meet_medium_tier() {
        let caps = PlatformCapabilities::for_platform(Platform::QuestStandalone);
        assert!(caps.meets_gpu_tier(GpuTier::Low));
        assert!(!caps.meets_gpu_tier(GpuTier::Medium));
    }

    #[test]
    fn capabilities_clone_eq() {
        let c1 = PlatformCapabilities::for_platform(Platform::Desktop);
        let c2 = c1.clone();
        assert_eq!(c1, c2);
    }
}
