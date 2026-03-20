//! Platform feature toggles.
//!
//! Provides a typed set of features and per-platform defaults so engine
//! subsystems can query feature availability without platform-specific branching.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::detection::Platform;

/// A discrete engine feature that may or may not be available on a platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Feature {
    /// 6DoF hand tracking (no controllers).
    HandTracking,
    /// Eye/gaze tracking.
    EyeTracking,
    /// Controller haptic feedback.
    Haptics,
    /// Client-side WASM scripting execution.
    WasmClientScripting,
    /// Spatial audio with HRTF.
    SpatialAudio,
    /// Passthrough mixed reality.
    PassthroughMr,
    /// High-resolution textures (4K+).
    HighResTextures,
    /// Hardware ray tracing.
    RayTracing,
}

/// A set of features available on a particular platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFlags {
    enabled: HashSet<Feature>,
}

impl FeatureFlags {
    /// Create an empty feature set.
    pub fn none() -> Self {
        Self {
            enabled: HashSet::new(),
        }
    }

    /// Create a feature set from a slice of features.
    pub fn from_features(features: &[Feature]) -> Self {
        Self {
            enabled: features.iter().copied().collect(),
        }
    }

    /// Returns the default feature set for a given platform.
    pub fn for_platform(platform: Platform) -> Self {
        match platform {
            Platform::PcVr => Self::from_features(&[
                Feature::HandTracking,
                Feature::EyeTracking,
                Feature::Haptics,
                Feature::WasmClientScripting,
                Feature::SpatialAudio,
                Feature::HighResTextures,
                Feature::RayTracing,
            ]),
            Platform::QuestStandalone => Self::from_features(&[
                Feature::HandTracking,
                Feature::EyeTracking,
                Feature::Haptics,
                Feature::WasmClientScripting,
                Feature::SpatialAudio,
                Feature::PassthroughMr,
            ]),
            Platform::Desktop => Self::from_features(&[
                Feature::WasmClientScripting,
                Feature::SpatialAudio,
                Feature::HighResTextures,
                Feature::RayTracing,
            ]),
            Platform::WebBrowser => {
                Self::from_features(&[Feature::WasmClientScripting, Feature::SpatialAudio])
            }
        }
    }

    /// Check whether a specific feature is supported.
    pub fn supports(&self, feature: Feature) -> bool {
        self.enabled.contains(&feature)
    }

    /// Add a feature to the set.
    pub fn enable(&mut self, feature: Feature) {
        self.enabled.insert(feature);
    }

    /// Remove a feature from the set.
    pub fn disable(&mut self, feature: Feature) {
        self.enabled.remove(&feature);
    }

    /// Returns the number of enabled features.
    pub fn count(&self) -> usize {
        self.enabled.len()
    }

    /// Returns an iterator over all enabled features.
    pub fn iter(&self) -> impl Iterator<Item = &Feature> {
        self.enabled.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcvr_supports_hand_tracking() {
        let flags = FeatureFlags::for_platform(Platform::PcVr);
        assert!(flags.supports(Feature::HandTracking));
    }

    #[test]
    fn pcvr_supports_ray_tracing() {
        let flags = FeatureFlags::for_platform(Platform::PcVr);
        assert!(flags.supports(Feature::RayTracing));
    }

    #[test]
    fn pcvr_does_not_support_passthrough() {
        let flags = FeatureFlags::for_platform(Platform::PcVr);
        assert!(!flags.supports(Feature::PassthroughMr));
    }

    #[test]
    fn quest_supports_passthrough() {
        let flags = FeatureFlags::for_platform(Platform::QuestStandalone);
        assert!(flags.supports(Feature::PassthroughMr));
    }

    #[test]
    fn quest_does_not_support_ray_tracing() {
        let flags = FeatureFlags::for_platform(Platform::QuestStandalone);
        assert!(!flags.supports(Feature::RayTracing));
    }

    #[test]
    fn quest_does_not_support_high_res_textures() {
        let flags = FeatureFlags::for_platform(Platform::QuestStandalone);
        assert!(!flags.supports(Feature::HighResTextures));
    }

    #[test]
    fn desktop_no_hand_tracking() {
        let flags = FeatureFlags::for_platform(Platform::Desktop);
        assert!(!flags.supports(Feature::HandTracking));
    }

    #[test]
    fn desktop_no_haptics() {
        let flags = FeatureFlags::for_platform(Platform::Desktop);
        assert!(!flags.supports(Feature::Haptics));
    }

    #[test]
    fn desktop_supports_wasm() {
        let flags = FeatureFlags::for_platform(Platform::Desktop);
        assert!(flags.supports(Feature::WasmClientScripting));
    }

    #[test]
    fn web_minimal_features() {
        let flags = FeatureFlags::for_platform(Platform::WebBrowser);
        assert!(flags.supports(Feature::WasmClientScripting));
        assert!(flags.supports(Feature::SpatialAudio));
        assert!(!flags.supports(Feature::HandTracking));
        assert!(!flags.supports(Feature::EyeTracking));
        assert!(!flags.supports(Feature::Haptics));
        assert!(!flags.supports(Feature::RayTracing));
        assert!(!flags.supports(Feature::HighResTextures));
        assert!(!flags.supports(Feature::PassthroughMr));
    }

    #[test]
    fn web_has_fewest_features() {
        let web = FeatureFlags::for_platform(Platform::WebBrowser);
        for platform in crate::detection::all_platforms() {
            let other = FeatureFlags::for_platform(*platform);
            assert!(
                web.count() <= other.count(),
                "web should have <= features than {:?}",
                platform
            );
        }
    }

    #[test]
    fn all_platforms_support_spatial_audio() {
        for platform in crate::detection::all_platforms() {
            let flags = FeatureFlags::for_platform(*platform);
            assert!(
                flags.supports(Feature::SpatialAudio),
                "{:?} should support spatial audio",
                platform
            );
        }
    }

    #[test]
    fn all_platforms_support_wasm_client() {
        for platform in crate::detection::all_platforms() {
            let flags = FeatureFlags::for_platform(*platform);
            assert!(
                flags.supports(Feature::WasmClientScripting),
                "{:?} should support wasm client scripting",
                platform
            );
        }
    }

    #[test]
    fn empty_feature_set() {
        let flags = FeatureFlags::none();
        assert_eq!(flags.count(), 0);
        assert!(!flags.supports(Feature::HandTracking));
    }

    #[test]
    fn from_features_constructor() {
        let flags = FeatureFlags::from_features(&[Feature::Haptics, Feature::RayTracing]);
        assert_eq!(flags.count(), 2);
        assert!(flags.supports(Feature::Haptics));
        assert!(flags.supports(Feature::RayTracing));
        assert!(!flags.supports(Feature::HandTracking));
    }

    #[test]
    fn enable_and_disable() {
        let mut flags = FeatureFlags::none();
        assert!(!flags.supports(Feature::Haptics));

        flags.enable(Feature::Haptics);
        assert!(flags.supports(Feature::Haptics));
        assert_eq!(flags.count(), 1);

        flags.disable(Feature::Haptics);
        assert!(!flags.supports(Feature::Haptics));
        assert_eq!(flags.count(), 0);
    }

    #[test]
    fn enable_duplicate_is_idempotent() {
        let mut flags = FeatureFlags::none();
        flags.enable(Feature::Haptics);
        flags.enable(Feature::Haptics);
        assert_eq!(flags.count(), 1);
    }

    #[test]
    fn disable_missing_is_noop() {
        let mut flags = FeatureFlags::none();
        flags.disable(Feature::Haptics);
        assert_eq!(flags.count(), 0);
    }

    #[test]
    fn feature_flags_clone_eq() {
        let f1 = FeatureFlags::for_platform(Platform::PcVr);
        let f2 = f1.clone();
        assert_eq!(f1, f2);
    }
}
