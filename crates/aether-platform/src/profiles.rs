//! Per-platform quality profiles with rendering budget presets.
//!
//! Each platform has a default quality profile that defines polygon budgets,
//! draw call limits, texture memory caps, MSAA levels, and shadow resolution.

use serde::{Deserialize, Serialize};

use crate::detection::Platform;

// --- PC VR defaults ---
const PCVR_MAX_POLYGONS_PER_EYE: u32 = 2_000_000;
const PCVR_MAX_DRAW_CALLS: u32 = 2_000;
const PCVR_MAX_TEXTURE_MEMORY_MB: u32 = 4_096;
const PCVR_MSAA_SAMPLES: u32 = 4;
const PCVR_SHADOW_RESOLUTION: u32 = 4_096;
const PCVR_MAX_AVATARS_RENDERED: u32 = 32;

// --- Quest defaults ---
const QUEST_MAX_POLYGONS_PER_EYE: u32 = 500_000;
const QUEST_MAX_DRAW_CALLS: u32 = 500;
const QUEST_MAX_TEXTURE_MEMORY_MB: u32 = 1_024;
const QUEST_MSAA_SAMPLES: u32 = 2;
const QUEST_SHADOW_RESOLUTION: u32 = 1_024;
const QUEST_MAX_AVATARS_RENDERED: u32 = 8;

// --- Desktop defaults ---
const DESKTOP_MAX_POLYGONS_PER_EYE: u32 = 1_000_000;
const DESKTOP_MAX_DRAW_CALLS: u32 = 1_500;
const DESKTOP_MAX_TEXTURE_MEMORY_MB: u32 = 2_048;
const DESKTOP_MSAA_SAMPLES: u32 = 4;
const DESKTOP_SHADOW_RESOLUTION: u32 = 2_048;
const DESKTOP_MAX_AVATARS_RENDERED: u32 = 24;

// --- Web defaults ---
const WEB_MAX_POLYGONS_PER_EYE: u32 = 300_000;
const WEB_MAX_DRAW_CALLS: u32 = 300;
const WEB_MAX_TEXTURE_MEMORY_MB: u32 = 512;
const WEB_MSAA_SAMPLES: u32 = 0;
const WEB_SHADOW_RESOLUTION: u32 = 512;
const WEB_MAX_AVATARS_RENDERED: u32 = 4;

/// Rendering quality profile for a specific platform.
///
/// Defines the rendering budget that systems must stay within
/// to maintain target frame rates on the given platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityProfile {
    /// Maximum polygon count per eye per frame.
    pub max_polygons_per_eye: u32,
    /// Maximum draw calls per frame.
    pub max_draw_calls: u32,
    /// Maximum texture memory in megabytes.
    pub max_texture_memory_mb: u32,
    /// MSAA sample count (0 = disabled).
    pub msaa_samples: u32,
    /// Shadow map resolution in pixels (width = height).
    pub shadow_resolution: u32,
    /// Maximum number of avatars to render simultaneously.
    pub max_avatars_rendered: u32,
}

impl QualityProfile {
    /// Returns the default quality profile for the given platform.
    pub fn for_platform(platform: Platform) -> Self {
        match platform {
            Platform::PcVr => Self::pc_vr(),
            Platform::QuestStandalone => Self::quest(),
            Platform::Desktop => Self::desktop(),
            Platform::WebBrowser => Self::web(),
        }
    }

    /// PC VR quality profile (high-end).
    pub fn pc_vr() -> Self {
        Self {
            max_polygons_per_eye: PCVR_MAX_POLYGONS_PER_EYE,
            max_draw_calls: PCVR_MAX_DRAW_CALLS,
            max_texture_memory_mb: PCVR_MAX_TEXTURE_MEMORY_MB,
            msaa_samples: PCVR_MSAA_SAMPLES,
            shadow_resolution: PCVR_SHADOW_RESOLUTION,
            max_avatars_rendered: PCVR_MAX_AVATARS_RENDERED,
        }
    }

    /// Quest standalone quality profile (constrained mobile GPU).
    pub fn quest() -> Self {
        Self {
            max_polygons_per_eye: QUEST_MAX_POLYGONS_PER_EYE,
            max_draw_calls: QUEST_MAX_DRAW_CALLS,
            max_texture_memory_mb: QUEST_MAX_TEXTURE_MEMORY_MB,
            msaa_samples: QUEST_MSAA_SAMPLES,
            shadow_resolution: QUEST_SHADOW_RESOLUTION,
            max_avatars_rendered: QUEST_MAX_AVATARS_RENDERED,
        }
    }

    /// Desktop quality profile (monitor-based, no VR).
    pub fn desktop() -> Self {
        Self {
            max_polygons_per_eye: DESKTOP_MAX_POLYGONS_PER_EYE,
            max_draw_calls: DESKTOP_MAX_DRAW_CALLS,
            max_texture_memory_mb: DESKTOP_MAX_TEXTURE_MEMORY_MB,
            msaa_samples: DESKTOP_MSAA_SAMPLES,
            shadow_resolution: DESKTOP_SHADOW_RESOLUTION,
            max_avatars_rendered: DESKTOP_MAX_AVATARS_RENDERED,
        }
    }

    /// Web browser quality profile (WebGL/WebGPU constraints).
    pub fn web() -> Self {
        Self {
            max_polygons_per_eye: WEB_MAX_POLYGONS_PER_EYE,
            max_draw_calls: WEB_MAX_DRAW_CALLS,
            max_texture_memory_mb: WEB_MAX_TEXTURE_MEMORY_MB,
            msaa_samples: WEB_MSAA_SAMPLES,
            shadow_resolution: WEB_SHADOW_RESOLUTION,
            max_avatars_rendered: WEB_MAX_AVATARS_RENDERED,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pc_vr_profile_values() {
        let p = QualityProfile::for_platform(Platform::PcVr);
        assert_eq!(p.max_polygons_per_eye, 2_000_000);
        assert_eq!(p.max_draw_calls, 2_000);
        assert_eq!(p.max_texture_memory_mb, 4_096);
        assert_eq!(p.msaa_samples, 4);
        assert_eq!(p.shadow_resolution, 4_096);
        assert_eq!(p.max_avatars_rendered, 32);
    }

    #[test]
    fn quest_profile_values() {
        let p = QualityProfile::for_platform(Platform::QuestStandalone);
        assert_eq!(p.max_polygons_per_eye, 500_000);
        assert_eq!(p.max_draw_calls, 500);
        assert_eq!(p.max_texture_memory_mb, 1_024);
        assert_eq!(p.msaa_samples, 2);
        assert_eq!(p.shadow_resolution, 1_024);
        assert_eq!(p.max_avatars_rendered, 8);
    }

    #[test]
    fn desktop_profile_values() {
        let p = QualityProfile::for_platform(Platform::Desktop);
        assert_eq!(p.max_polygons_per_eye, 1_000_000);
        assert_eq!(p.max_draw_calls, 1_500);
        assert_eq!(p.max_texture_memory_mb, 2_048);
        assert_eq!(p.msaa_samples, 4);
        assert_eq!(p.shadow_resolution, 2_048);
        assert_eq!(p.max_avatars_rendered, 24);
    }

    #[test]
    fn web_profile_values() {
        let p = QualityProfile::for_platform(Platform::WebBrowser);
        assert_eq!(p.max_polygons_per_eye, 300_000);
        assert_eq!(p.max_draw_calls, 300);
        assert_eq!(p.max_texture_memory_mb, 512);
        assert_eq!(p.msaa_samples, 0);
        assert_eq!(p.shadow_resolution, 512);
        assert_eq!(p.max_avatars_rendered, 4);
    }

    #[test]
    fn quest_is_most_constrained_vr() {
        let quest = QualityProfile::for_platform(Platform::QuestStandalone);
        let pcvr = QualityProfile::for_platform(Platform::PcVr);
        assert!(quest.max_polygons_per_eye < pcvr.max_polygons_per_eye);
        assert!(quest.max_draw_calls < pcvr.max_draw_calls);
        assert!(quest.max_texture_memory_mb < pcvr.max_texture_memory_mb);
        assert!(quest.shadow_resolution < pcvr.shadow_resolution);
    }

    #[test]
    fn web_is_most_constrained_overall() {
        let web = QualityProfile::for_platform(Platform::WebBrowser);
        for platform in crate::detection::all_platforms() {
            if *platform == Platform::WebBrowser {
                continue;
            }
            let other = QualityProfile::for_platform(*platform);
            assert!(
                web.max_polygons_per_eye <= other.max_polygons_per_eye,
                "web polygons should be <= {:?}",
                platform
            );
            assert!(
                web.max_draw_calls <= other.max_draw_calls,
                "web draw calls should be <= {:?}",
                platform
            );
        }
    }

    #[test]
    fn profile_clone_eq() {
        let p1 = QualityProfile::pc_vr();
        let p2 = p1.clone();
        assert_eq!(p1, p2);
    }

    #[test]
    fn named_constructors_match_for_platform() {
        assert_eq!(
            QualityProfile::pc_vr(),
            QualityProfile::for_platform(Platform::PcVr)
        );
        assert_eq!(
            QualityProfile::quest(),
            QualityProfile::for_platform(Platform::QuestStandalone)
        );
        assert_eq!(
            QualityProfile::desktop(),
            QualityProfile::for_platform(Platform::Desktop)
        );
        assert_eq!(
            QualityProfile::web(),
            QualityProfile::for_platform(Platform::WebBrowser)
        );
    }

    #[test]
    fn desktop_between_quest_and_pcvr() {
        let quest = QualityProfile::quest();
        let desktop = QualityProfile::desktop();
        let pcvr = QualityProfile::pc_vr();
        assert!(quest.max_polygons_per_eye < desktop.max_polygons_per_eye);
        assert!(desktop.max_polygons_per_eye < pcvr.max_polygons_per_eye);
    }
}
