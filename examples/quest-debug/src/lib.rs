//! Quest 3 debug overlay demo.
//!
//! Minimal VR application that renders a debug overlay panel
//! showing real-time tracking and performance data inside VR.
//!
//! On Quest 3: uses real OpenXR + EGL + GLES to render frames.
//! On desktop: testable headlessly via stubbed `run_headless()`.

pub mod overlay_quad;

#[cfg(target_os = "android")]
mod egl;
#[cfg(target_os = "android")]
mod xr_runtime;

use aether_vr_overlay::layout::DebugOverlayData;
use aether_vr_overlay::panel::OverlayPanel;
use aether_vr_overlay::OverlayConfig;

/// Run the headless test loop (desktop only, for unit tests).
pub fn run_headless(config: OverlayConfig) -> Result<(), String> {
    let mut overlay = OverlayPanel::from_config(&config);
    let data = DebugOverlayData::default();
    for _ in 0..10 {
        overlay.render(&data);
    }
    Ok(())
}

/// NativeActivity entry point for Android/Quest.
/// The `ndk_glue::main` macro generates `ANativeActivity_onCreate` which
/// Android's NativeActivity loader calls when starting the app.
#[cfg(target_os = "android")]
#[ndk_glue::main()]
pub fn android_main() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("AetherVR"),
    );

    log::info!("Aether Quest Debug starting...");

    // Step 1: Initialize EGL
    let egl_ctx = match egl::init_egl() {
        Ok(ctx) => ctx,
        Err(e) => {
            log::error!("EGL init failed: {e}");
            return;
        }
    };

    // Step 2: Run real OpenXR loop
    match xr_runtime::run_xr_loop(&egl_ctx) {
        Ok(()) => log::info!("Aether Quest Debug exited normally"),
        Err(e) => log::error!("Aether Quest Debug error: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_headless_completes() {
        let result = run_headless(OverlayConfig::default());
        assert!(result.is_ok());
    }

    #[test]
    fn run_headless_with_custom_config() {
        let config = OverlayConfig {
            width: 256,
            height: 128,
            text_scale: 1,
            initially_visible: true,
        };
        let result = run_headless(config);
        assert!(result.is_ok());
    }

    #[test]
    fn run_headless_invisible() {
        let config = OverlayConfig {
            initially_visible: false,
            ..OverlayConfig::default()
        };
        let result = run_headless(config);
        assert!(result.is_ok());
    }
}
