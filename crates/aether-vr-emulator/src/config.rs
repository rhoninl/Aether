//! Configuration types and headset presets for the VR emulator.

use serde::{Deserialize, Serialize};

/// Default window width in pixels.
const DEFAULT_WINDOW_WIDTH: usize = 1280;

/// Default window height in pixels.
const DEFAULT_WINDOW_HEIGHT: usize = 720;

/// Minimum supported refresh rate in Hz.
const MIN_REFRESH_RATE_HZ: u32 = 30;

/// Maximum supported refresh rate in Hz.
const MAX_REFRESH_RATE_HZ: u32 = 240;

/// Minimum supported IPD in millimeters.
const MIN_IPD_MM: f32 = 50.0;

/// Maximum supported IPD in millimeters.
const MAX_IPD_MM: f32 = 80.0;

/// Minimum FOV in degrees.
const MIN_FOV_DEG: f32 = 30.0;

/// Maximum FOV in degrees.
const MAX_FOV_DEG: f32 = 180.0;

/// Well-known VR headset presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeadsetPreset {
    Quest2,
    Quest3,
    ValveIndex,
    Pico4,
}

/// Per-eye display specification.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EyeResolution {
    pub width: u32,
    pub height: u32,
}

/// View mode for the emulator window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ViewMode {
    /// Side-by-side stereo (left and right eye).
    Stereo,
    /// Single eye mono view.
    Mono,
}

/// Display configuration for the emulator.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// Per-eye logical resolution (scaled for display).
    pub eye_resolution: EyeResolution,
    /// Horizontal field of view in degrees.
    pub h_fov_deg: f32,
    /// Vertical field of view in degrees.
    pub v_fov_deg: f32,
    /// Interpupillary distance in millimeters.
    pub ipd_mm: f32,
    /// Target refresh rate in Hz.
    pub refresh_rate_hz: u32,
    /// View mode (stereo or mono).
    pub view_mode: ViewMode,
}

/// Keyboard/mouse sensitivity settings.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InputSensitivity {
    /// Mouse look sensitivity multiplier.
    pub mouse_look: f32,
    /// Movement speed in meters per second.
    pub move_speed: f32,
    /// Controller rotation speed in degrees per second.
    pub controller_rotation_speed: f32,
}

impl Default for InputSensitivity {
    fn default() -> Self {
        Self {
            mouse_look: 0.003,
            move_speed: 2.0,
            controller_rotation_speed: 90.0,
        }
    }
}

/// Full emulator configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmulatorConfig {
    /// Display configuration derived from headset preset or custom.
    pub display: DisplayConfig,
    /// Input sensitivity settings.
    pub input_sensitivity: InputSensitivity,
    /// Window width in pixels.
    pub window_width: usize,
    /// Window height in pixels.
    pub window_height: usize,
    /// Whether to show the debug overlay.
    pub show_debug_overlay: bool,
    /// Application name shown in the window title.
    pub application_name: String,
}

/// Errors that can occur when validating configuration.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    /// IPD is outside the valid range.
    IpdOutOfRange { value: f32, min: f32, max: f32 },
    /// Refresh rate is outside the valid range.
    RefreshRateOutOfRange { value: u32, min: u32, max: u32 },
    /// FOV is outside the valid range.
    FovOutOfRange { value: f32, min: f32, max: f32 },
    /// Eye resolution has a zero dimension.
    ZeroResolution,
    /// Window size has a zero dimension.
    ZeroWindowSize,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::IpdOutOfRange { value, min, max } => {
                write!(f, "IPD {value} mm out of range [{min}, {max}]")
            }
            ConfigError::RefreshRateOutOfRange { value, min, max } => {
                write!(f, "Refresh rate {value} Hz out of range [{min}, {max}]")
            }
            ConfigError::FovOutOfRange { value, min, max } => {
                write!(f, "FOV {value} deg out of range [{min}, {max}]")
            }
            ConfigError::ZeroResolution => write!(f, "Eye resolution cannot be zero"),
            ConfigError::ZeroWindowSize => write!(f, "Window size cannot be zero"),
        }
    }
}

/// Return the display configuration for a known headset preset.
pub fn preset_display(preset: HeadsetPreset) -> DisplayConfig {
    match preset {
        HeadsetPreset::Quest2 => DisplayConfig {
            eye_resolution: EyeResolution {
                width: 1832,
                height: 1920,
            },
            h_fov_deg: 97.0,
            v_fov_deg: 93.0,
            ipd_mm: 63.0,
            refresh_rate_hz: 90,
            view_mode: ViewMode::Stereo,
        },
        HeadsetPreset::Quest3 => DisplayConfig {
            eye_resolution: EyeResolution {
                width: 2064,
                height: 2208,
            },
            h_fov_deg: 110.0,
            v_fov_deg: 96.0,
            ipd_mm: 63.0,
            refresh_rate_hz: 120,
            view_mode: ViewMode::Stereo,
        },
        HeadsetPreset::ValveIndex => DisplayConfig {
            eye_resolution: EyeResolution {
                width: 1440,
                height: 1600,
            },
            h_fov_deg: 130.0,
            v_fov_deg: 120.0,
            ipd_mm: 63.5,
            refresh_rate_hz: 144,
            view_mode: ViewMode::Stereo,
        },
        HeadsetPreset::Pico4 => DisplayConfig {
            eye_resolution: EyeResolution {
                width: 2160,
                height: 2160,
            },
            h_fov_deg: 105.0,
            v_fov_deg: 105.0,
            ipd_mm: 62.0,
            refresh_rate_hz: 90,
            view_mode: ViewMode::Stereo,
        },
    }
}

impl EmulatorConfig {
    /// Create a configuration from a known headset preset.
    pub fn from_preset(preset: HeadsetPreset) -> Self {
        Self {
            display: preset_display(preset),
            input_sensitivity: InputSensitivity::default(),
            window_width: DEFAULT_WINDOW_WIDTH,
            window_height: DEFAULT_WINDOW_HEIGHT,
            show_debug_overlay: true,
            application_name: "Aether VR Emulator".to_string(),
        }
    }

    /// Create a configuration with custom display settings.
    pub fn custom(display: DisplayConfig) -> Self {
        Self {
            display,
            input_sensitivity: InputSensitivity::default(),
            window_width: DEFAULT_WINDOW_WIDTH,
            window_height: DEFAULT_WINDOW_HEIGHT,
            show_debug_overlay: true,
            application_name: "Aether VR Emulator".to_string(),
        }
    }

    /// Validate the configuration, returning an error if any values are invalid.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.display.ipd_mm < MIN_IPD_MM || self.display.ipd_mm > MAX_IPD_MM {
            return Err(ConfigError::IpdOutOfRange {
                value: self.display.ipd_mm,
                min: MIN_IPD_MM,
                max: MAX_IPD_MM,
            });
        }
        if self.display.refresh_rate_hz < MIN_REFRESH_RATE_HZ
            || self.display.refresh_rate_hz > MAX_REFRESH_RATE_HZ
        {
            return Err(ConfigError::RefreshRateOutOfRange {
                value: self.display.refresh_rate_hz,
                min: MIN_REFRESH_RATE_HZ,
                max: MAX_REFRESH_RATE_HZ,
            });
        }
        if self.display.h_fov_deg < MIN_FOV_DEG || self.display.h_fov_deg > MAX_FOV_DEG {
            return Err(ConfigError::FovOutOfRange {
                value: self.display.h_fov_deg,
                min: MIN_FOV_DEG,
                max: MAX_FOV_DEG,
            });
        }
        if self.display.v_fov_deg < MIN_FOV_DEG || self.display.v_fov_deg > MAX_FOV_DEG {
            return Err(ConfigError::FovOutOfRange {
                value: self.display.v_fov_deg,
                min: MIN_FOV_DEG,
                max: MAX_FOV_DEG,
            });
        }
        if self.display.eye_resolution.width == 0 || self.display.eye_resolution.height == 0 {
            return Err(ConfigError::ZeroResolution);
        }
        if self.window_width == 0 || self.window_height == 0 {
            return Err(ConfigError::ZeroWindowSize);
        }
        Ok(())
    }

    /// Get the frame interval in nanoseconds for the target refresh rate.
    pub fn frame_interval_ns(&self) -> u64 {
        1_000_000_000 / self.display.refresh_rate_hz as u64
    }

    /// Get the IPD in meters (converted from millimeters).
    pub fn ipd_meters(&self) -> f32 {
        self.display.ipd_mm / 1000.0
    }
}

impl Default for EmulatorConfig {
    fn default() -> Self {
        Self::from_preset(HeadsetPreset::Quest2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- HeadsetPreset tests ----

    #[test]
    fn quest2_preset_values() {
        let display = preset_display(HeadsetPreset::Quest2);
        assert_eq!(display.eye_resolution.width, 1832);
        assert_eq!(display.eye_resolution.height, 1920);
        assert_eq!(display.h_fov_deg, 97.0);
        assert_eq!(display.v_fov_deg, 93.0);
        assert_eq!(display.ipd_mm, 63.0);
        assert_eq!(display.refresh_rate_hz, 90);
        assert_eq!(display.view_mode, ViewMode::Stereo);
    }

    #[test]
    fn quest3_preset_values() {
        let display = preset_display(HeadsetPreset::Quest3);
        assert_eq!(display.eye_resolution.width, 2064);
        assert_eq!(display.eye_resolution.height, 2208);
        assert_eq!(display.h_fov_deg, 110.0);
        assert_eq!(display.refresh_rate_hz, 120);
    }

    #[test]
    fn valve_index_preset_values() {
        let display = preset_display(HeadsetPreset::ValveIndex);
        assert_eq!(display.eye_resolution.width, 1440);
        assert_eq!(display.eye_resolution.height, 1600);
        assert_eq!(display.h_fov_deg, 130.0);
        assert_eq!(display.v_fov_deg, 120.0);
        assert_eq!(display.ipd_mm, 63.5);
        assert_eq!(display.refresh_rate_hz, 144);
    }

    #[test]
    fn pico4_preset_values() {
        let display = preset_display(HeadsetPreset::Pico4);
        assert_eq!(display.eye_resolution.width, 2160);
        assert_eq!(display.eye_resolution.height, 2160);
        assert_eq!(display.h_fov_deg, 105.0);
        assert_eq!(display.refresh_rate_hz, 90);
    }

    // ---- EmulatorConfig tests ----

    #[test]
    fn from_preset_creates_valid_config() {
        let config = EmulatorConfig::from_preset(HeadsetPreset::Quest2);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn all_presets_create_valid_configs() {
        for preset in [
            HeadsetPreset::Quest2,
            HeadsetPreset::Quest3,
            HeadsetPreset::ValveIndex,
            HeadsetPreset::Pico4,
        ] {
            let config = EmulatorConfig::from_preset(preset);
            assert!(config.validate().is_ok(), "preset {preset:?} invalid");
        }
    }

    #[test]
    fn default_config_is_quest2() {
        let config = EmulatorConfig::default();
        assert_eq!(config.display, preset_display(HeadsetPreset::Quest2));
    }

    #[test]
    fn default_shows_debug_overlay() {
        let config = EmulatorConfig::default();
        assert!(config.show_debug_overlay);
    }

    #[test]
    fn custom_config_stores_display() {
        let display = DisplayConfig {
            eye_resolution: EyeResolution {
                width: 800,
                height: 600,
            },
            h_fov_deg: 90.0,
            v_fov_deg: 80.0,
            ipd_mm: 65.0,
            refresh_rate_hz: 60,
            view_mode: ViewMode::Mono,
        };
        let config = EmulatorConfig::custom(display);
        assert_eq!(config.display.view_mode, ViewMode::Mono);
        assert_eq!(config.display.eye_resolution.width, 800);
    }

    // ---- Validation tests ----

    #[test]
    fn validate_rejects_too_small_ipd() {
        let mut config = EmulatorConfig::default();
        config.display.ipd_mm = 40.0;
        let err = config.validate().unwrap_err();
        assert_eq!(
            err,
            ConfigError::IpdOutOfRange {
                value: 40.0,
                min: MIN_IPD_MM,
                max: MAX_IPD_MM,
            }
        );
    }

    #[test]
    fn validate_rejects_too_large_ipd() {
        let mut config = EmulatorConfig::default();
        config.display.ipd_mm = 90.0;
        assert!(matches!(
            config.validate(),
            Err(ConfigError::IpdOutOfRange { .. })
        ));
    }

    #[test]
    fn validate_rejects_too_low_refresh_rate() {
        let mut config = EmulatorConfig::default();
        config.display.refresh_rate_hz = 10;
        assert!(matches!(
            config.validate(),
            Err(ConfigError::RefreshRateOutOfRange { .. })
        ));
    }

    #[test]
    fn validate_rejects_too_high_refresh_rate() {
        let mut config = EmulatorConfig::default();
        config.display.refresh_rate_hz = 500;
        assert!(matches!(
            config.validate(),
            Err(ConfigError::RefreshRateOutOfRange { .. })
        ));
    }

    #[test]
    fn validate_rejects_too_small_fov() {
        let mut config = EmulatorConfig::default();
        config.display.h_fov_deg = 10.0;
        assert!(matches!(
            config.validate(),
            Err(ConfigError::FovOutOfRange { .. })
        ));
    }

    #[test]
    fn validate_rejects_too_large_fov() {
        let mut config = EmulatorConfig::default();
        config.display.v_fov_deg = 200.0;
        assert!(matches!(
            config.validate(),
            Err(ConfigError::FovOutOfRange { .. })
        ));
    }

    #[test]
    fn validate_rejects_zero_resolution() {
        let mut config = EmulatorConfig::default();
        config.display.eye_resolution.width = 0;
        assert_eq!(config.validate().unwrap_err(), ConfigError::ZeroResolution);
    }

    #[test]
    fn validate_rejects_zero_window_size() {
        let mut config = EmulatorConfig::default();
        config.window_width = 0;
        assert_eq!(config.validate().unwrap_err(), ConfigError::ZeroWindowSize);
    }

    #[test]
    fn validate_accepts_boundary_values() {
        let mut config = EmulatorConfig::default();
        config.display.ipd_mm = MIN_IPD_MM;
        config.display.refresh_rate_hz = MIN_REFRESH_RATE_HZ;
        config.display.h_fov_deg = MIN_FOV_DEG;
        config.display.v_fov_deg = MIN_FOV_DEG;
        assert!(config.validate().is_ok());

        config.display.ipd_mm = MAX_IPD_MM;
        config.display.refresh_rate_hz = MAX_REFRESH_RATE_HZ;
        config.display.h_fov_deg = MAX_FOV_DEG;
        config.display.v_fov_deg = MAX_FOV_DEG;
        assert!(config.validate().is_ok());
    }

    // ---- Computed values tests ----

    #[test]
    fn frame_interval_ns_90hz() {
        let config = EmulatorConfig::from_preset(HeadsetPreset::Quest2);
        assert_eq!(config.frame_interval_ns(), 1_000_000_000 / 90);
    }

    #[test]
    fn frame_interval_ns_120hz() {
        let config = EmulatorConfig::from_preset(HeadsetPreset::Quest3);
        assert_eq!(config.frame_interval_ns(), 1_000_000_000 / 120);
    }

    #[test]
    fn frame_interval_ns_144hz() {
        let config = EmulatorConfig::from_preset(HeadsetPreset::ValveIndex);
        assert_eq!(config.frame_interval_ns(), 1_000_000_000 / 144);
    }

    #[test]
    fn ipd_meters_conversion() {
        let config = EmulatorConfig::default();
        let expected = 63.0 / 1000.0;
        assert!((config.ipd_meters() - expected).abs() < 1e-6);
    }

    #[test]
    fn ipd_meters_custom() {
        let mut config = EmulatorConfig::default();
        config.display.ipd_mm = 70.0;
        assert!((config.ipd_meters() - 0.070).abs() < 1e-6);
    }

    // ---- InputSensitivity tests ----

    #[test]
    fn default_sensitivity_values() {
        let sens = InputSensitivity::default();
        assert_eq!(sens.mouse_look, 0.003);
        assert_eq!(sens.move_speed, 2.0);
        assert_eq!(sens.controller_rotation_speed, 90.0);
    }

    // ---- Serialization tests ----

    #[test]
    fn view_mode_equality() {
        assert_eq!(ViewMode::Stereo, ViewMode::Stereo);
        assert_eq!(ViewMode::Mono, ViewMode::Mono);
        assert_ne!(ViewMode::Stereo, ViewMode::Mono);
    }

    #[test]
    fn headset_preset_equality() {
        assert_eq!(HeadsetPreset::Quest2, HeadsetPreset::Quest2);
        assert_ne!(HeadsetPreset::Quest2, HeadsetPreset::Quest3);
    }

    #[test]
    fn config_error_display() {
        let err = ConfigError::IpdOutOfRange {
            value: 40.0,
            min: 50.0,
            max: 80.0,
        };
        let msg = format!("{err}");
        assert!(msg.contains("40"));
        assert!(msg.contains("50"));
        assert!(msg.contains("80"));
    }
}
