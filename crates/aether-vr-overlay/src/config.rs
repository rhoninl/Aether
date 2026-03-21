//! Environment-based configuration for the debug overlay.

use crate::panel::{DEFAULT_PANEL_HEIGHT, DEFAULT_PANEL_WIDTH, DEFAULT_TEXT_SCALE};

const ENV_OVERLAY_WIDTH: &str = "AETHER_OVERLAY_WIDTH";
const ENV_OVERLAY_HEIGHT: &str = "AETHER_OVERLAY_HEIGHT";
const ENV_OVERLAY_SCALE: &str = "AETHER_OVERLAY_TEXT_SCALE";
const ENV_OVERLAY_VISIBLE: &str = "AETHER_OVERLAY_VISIBLE";

/// Configuration for the overlay panel.
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    pub width: usize,
    pub height: usize,
    pub text_scale: usize,
    pub initially_visible: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            width: DEFAULT_PANEL_WIDTH,
            height: DEFAULT_PANEL_HEIGHT,
            text_scale: DEFAULT_TEXT_SCALE,
            initially_visible: true,
        }
    }
}

impl OverlayConfig {
    /// Load configuration from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        Self {
            width: parse_env_usize(ENV_OVERLAY_WIDTH, DEFAULT_PANEL_WIDTH),
            height: parse_env_usize(ENV_OVERLAY_HEIGHT, DEFAULT_PANEL_HEIGHT),
            text_scale: parse_env_usize(ENV_OVERLAY_SCALE, DEFAULT_TEXT_SCALE),
            initially_visible: parse_env_bool(ENV_OVERLAY_VISIBLE, true),
        }
    }
}

fn parse_env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn parse_env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = OverlayConfig::default();
        assert_eq!(config.width, DEFAULT_PANEL_WIDTH);
        assert_eq!(config.height, DEFAULT_PANEL_HEIGHT);
        assert_eq!(config.text_scale, DEFAULT_TEXT_SCALE);
        assert!(config.initially_visible);
    }

    #[test]
    fn from_env_uses_defaults() {
        // When env vars are not set, should use defaults
        let config = OverlayConfig::from_env();
        assert!(config.width > 0);
        assert!(config.height > 0);
        assert!(config.text_scale > 0);
    }

    #[test]
    fn parse_env_usize_default() {
        let val = parse_env_usize("NONEXISTENT_VAR_12345", 42);
        assert_eq!(val, 42);
    }

    #[test]
    fn parse_env_bool_default() {
        let val = parse_env_bool("NONEXISTENT_VAR_12345", true);
        assert!(val);
        let val = parse_env_bool("NONEXISTENT_VAR_12345", false);
        assert!(!val);
    }
}
