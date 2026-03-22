//! Quest 3 debug overlay demo.
//!
//! Minimal VR application that renders a debug overlay panel
//! showing real-time tracking and performance data inside VR.
//!
//! On Quest 3: runs as NativeActivity, renders overlay via GLES.
//! On desktop: testable headlessly via `run_debug_overlay()`.

pub mod overlay_quad;

#[cfg(target_os = "android")]
use android_logger;

use aether_openxr::frame_loop;
use aether_openxr::input_actions::XrInputActions;
use aether_openxr::instance::{InstanceConfig, XrInstance};
use aether_openxr::session::{XrSession, XrSessionState};
use aether_openxr::swapchain::{SwapchainConfig, XrSwapchain};
use aether_vr_overlay::layout::DebugOverlayData;
use aether_vr_overlay::panel::OverlayPanel;
use aether_vr_overlay::OverlayConfig;

const TARGET_FRAME_TIME_MS: f32 = 1000.0 / 120.0;
const MAX_HEADLESS_FRAMES: u64 = 10;

/// Run the VR debug overlay loop.
///
/// On Quest this runs until the session ends.
/// In tests (headless) it runs for `MAX_HEADLESS_FRAMES` frames.
pub fn run_debug_overlay(config: OverlayConfig, headless: bool) -> Result<(), String> {
    let xr_instance =
        XrInstance::new(InstanceConfig::default()).map_err(|e| format!("XR init: {e}"))?;

    let mut session = XrSession::new(&xr_instance).map_err(|e| format!("XR session: {e}"))?;

    let mut left_swapchain =
        XrSwapchain::new(SwapchainConfig::default()).map_err(|e| format!("swapchain: {e}"))?;
    let mut right_swapchain =
        XrSwapchain::new(SwapchainConfig::default()).map_err(|e| format!("swapchain: {e}"))?;

    let mut input_actions = XrInputActions::new();
    let mut overlay = OverlayPanel::from_config(&config);

    log::info!("Quest debug overlay initialized (headless={})", headless);

    session.transition_to(XrSessionState::Ready);
    session.transition_to(XrSessionState::Focused);

    let mut frame_count: u64 = 0;

    loop {
        if !session.is_active() {
            break;
        }
        if headless && frame_count >= MAX_HEADLESS_FRAMES {
            break;
        }

        let frame_state = frame_loop::wait_frame(&session).map_err(|e| format!("wait: {e}"))?;

        frame_loop::begin_frame(&session).map_err(|e| format!("begin: {e}"))?;

        if frame_state.should_render {
            let snapshot = input_actions.sync_and_snapshot(frame_state.predicted_display_time_ns);

            let overlay_data = DebugOverlayData::from_snapshot(
                &snapshot,
                1000.0 / TARGET_FRAME_TIME_MS,
                TARGET_FRAME_TIME_MS,
                &session.state().to_string(),
                frame_count,
            );

            overlay.render(&overlay_data);

            for swapchain in [&mut left_swapchain, &mut right_swapchain] {
                let _image_idx = swapchain.acquire().map_err(|e| format!("acquire: {e}"))?;
                // TODO: GLES rendering — bind FBO, draw scene + overlay quad, present
                swapchain.release().map_err(|e| format!("release: {e}"))?;
            }
        }

        frame_loop::end_frame(&session, frame_state.predicted_display_time_ns)
            .map_err(|e| format!("end: {e}"))?;

        frame_count += 1;
    }

    log::info!("Quest debug overlay finished ({frame_count} frames)");
    Ok(())
}

/// NativeActivity entry point for Android/Quest.
///
/// `ndk_glue::main` provides the `ANativeActivity_onCreate` export that
/// Android's NativeActivity looks for when loading the .so.
#[cfg(target_os = "android")]
#[no_mangle]
pub fn android_main() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("AetherVR"),
    );

    log::info!("Aether Quest Debug starting...");

    let config = OverlayConfig::from_env();
    match run_debug_overlay(config, false) {
        Ok(()) => log::info!("Aether Quest Debug exited normally"),
        Err(e) => log::error!("Aether Quest Debug fatal error: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_debug_overlay_completes() {
        let result = run_debug_overlay(OverlayConfig::default(), true);
        assert!(result.is_ok());
    }

    #[test]
    fn run_debug_overlay_with_custom_config() {
        let config = OverlayConfig {
            width: 256,
            height: 128,
            text_scale: 1,
            initially_visible: true,
        };
        let result = run_debug_overlay(config, true);
        assert!(result.is_ok());
    }

    #[test]
    fn run_debug_overlay_invisible() {
        let config = OverlayConfig {
            initially_visible: false,
            ..OverlayConfig::default()
        };
        let result = run_debug_overlay(config, true);
        assert!(result.is_ok());
    }
}
