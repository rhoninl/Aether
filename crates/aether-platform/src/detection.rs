//! Runtime platform detection.
//!
//! Determines which platform the engine is running on by checking
//! the `AETHER_PLATFORM` environment variable, with OS-based fallback.

use serde::{Deserialize, Serialize};
use std::fmt;

const ENV_VAR_PLATFORM: &str = "AETHER_PLATFORM";

/// Target platform for the Aether engine client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    /// PC-based VR headset (e.g. Valve Index, HTC Vive).
    PcVr,
    /// Meta Quest standalone headset.
    QuestStandalone,
    /// Traditional desktop (monitor, keyboard, mouse).
    Desktop,
    /// Web browser via WebXR/WebGL.
    WebBrowser,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::PcVr => write!(f, "pc_vr"),
            Platform::QuestStandalone => write!(f, "quest"),
            Platform::Desktop => write!(f, "desktop"),
            Platform::WebBrowser => write!(f, "web"),
        }
    }
}

/// Error returned when a platform string cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformParseError {
    pub input: String,
}

impl fmt::Display for PlatformParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown platform: '{}'", self.input)
    }
}

impl std::error::Error for PlatformParseError {}

impl std::str::FromStr for Platform {
    type Err = PlatformParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pc_vr" | "pcvr" | "pc-vr" => Ok(Platform::PcVr),
            "quest" | "quest_standalone" | "quest-standalone" => Ok(Platform::QuestStandalone),
            "desktop" => Ok(Platform::Desktop),
            "web" | "web_browser" | "web-browser" | "webbrowser" => Ok(Platform::WebBrowser),
            _ => Err(PlatformParseError {
                input: s.to_string(),
            }),
        }
    }
}

/// Detect the current platform at runtime.
///
/// Checks the `AETHER_PLATFORM` environment variable first. If not set,
/// falls back to OS-based heuristics (defaults to `Desktop`).
pub fn detect_platform() -> Platform {
    if let Ok(val) = std::env::var(ENV_VAR_PLATFORM) {
        if let Ok(p) = val.parse::<Platform>() {
            return p;
        }
    }
    detect_from_os()
}

/// Detect platform from the current OS target.
fn detect_from_os() -> Platform {
    if cfg!(target_os = "android") {
        Platform::QuestStandalone
    } else if cfg!(target_arch = "wasm32") {
        Platform::WebBrowser
    } else {
        Platform::Desktop
    }
}

/// All known platform variants, useful for iteration.
pub fn all_platforms() -> &'static [Platform] {
    &[
        Platform::PcVr,
        Platform::QuestStandalone,
        Platform::Desktop,
        Platform::WebBrowser,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parse_pc_vr_variants() {
        assert_eq!(Platform::from_str("pc_vr").unwrap(), Platform::PcVr);
        assert_eq!(Platform::from_str("pcvr").unwrap(), Platform::PcVr);
        assert_eq!(Platform::from_str("pc-vr").unwrap(), Platform::PcVr);
        assert_eq!(Platform::from_str("PC_VR").unwrap(), Platform::PcVr);
    }

    #[test]
    fn parse_quest_variants() {
        assert_eq!(
            Platform::from_str("quest").unwrap(),
            Platform::QuestStandalone
        );
        assert_eq!(
            Platform::from_str("quest_standalone").unwrap(),
            Platform::QuestStandalone
        );
        assert_eq!(
            Platform::from_str("quest-standalone").unwrap(),
            Platform::QuestStandalone
        );
    }

    #[test]
    fn parse_desktop() {
        assert_eq!(Platform::from_str("desktop").unwrap(), Platform::Desktop);
        assert_eq!(Platform::from_str("DESKTOP").unwrap(), Platform::Desktop);
    }

    #[test]
    fn parse_web_variants() {
        assert_eq!(Platform::from_str("web").unwrap(), Platform::WebBrowser);
        assert_eq!(
            Platform::from_str("web_browser").unwrap(),
            Platform::WebBrowser
        );
        assert_eq!(
            Platform::from_str("web-browser").unwrap(),
            Platform::WebBrowser
        );
        assert_eq!(
            Platform::from_str("webbrowser").unwrap(),
            Platform::WebBrowser
        );
    }

    #[test]
    fn parse_unknown_returns_error() {
        let err = Platform::from_str("nintendo_switch").unwrap_err();
        assert_eq!(err.input, "nintendo_switch");
        assert!(err.to_string().contains("nintendo_switch"));
    }

    #[test]
    fn display_roundtrip() {
        for p in all_platforms() {
            let s = p.to_string();
            let parsed = Platform::from_str(&s).unwrap();
            assert_eq!(*p, parsed);
        }
    }

    #[test]
    fn detect_platform_env_var() {
        // When env var is not set (or invalid), should fall back to OS detection.
        // On non-Android, non-WASM hosts this will be Desktop.
        let detected = detect_from_os();
        assert_eq!(detected, Platform::Desktop);
    }

    #[test]
    fn all_platforms_contains_all_variants() {
        let platforms = all_platforms();
        assert_eq!(platforms.len(), 4);
        assert!(platforms.contains(&Platform::PcVr));
        assert!(platforms.contains(&Platform::QuestStandalone));
        assert!(platforms.contains(&Platform::Desktop));
        assert!(platforms.contains(&Platform::WebBrowser));
    }

    #[test]
    fn platform_clone_and_eq() {
        let p = Platform::PcVr;
        let p2 = p;
        assert_eq!(p, p2);
    }

    #[test]
    fn platform_hash_works() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Platform::PcVr);
        set.insert(Platform::PcVr);
        assert_eq!(set.len(), 1);
        set.insert(Platform::Desktop);
        assert_eq!(set.len(), 2);
    }
}
