//! Quest 3 debug overlay demo.
//!
//! This is a minimal VR application that renders a debug overlay panel
//! showing real-time tracking and performance data in VR space.
//!
//! On Quest 3, it runs as a NativeActivity with OpenXR + GLES.
//! On desktop, it can be tested headlessly.

pub mod overlay_quad;

use aether_openxr::frame_loop;
use aether_openxr::input_actions::XrInputActions;
use aether_openxr::instance::{InstanceConfig, XrInstance};
use aether_openxr::session::{XrSession, XrSessionState};
use aether_openxr::swapchain::{SwapchainConfig, XrSwapchain};
use aether_vr_overlay::layout::DebugOverlayData;
use aether_vr_overlay::panel::OverlayPanel;
use aether_vr_overlay::OverlayConfig;

const TARGET_FRAME_TIME_MS: f32 = 1000.0 / 120.0; // 120 Hz for Quest 3

/// Run the VR debug overlay loop.
///
/// This is the main entry point called from `android_main` on Quest
/// or from tests on desktop.
pub fn run_debug_overlay(config: OverlayConfig) -> Result<(), String> {
    // Initialize OpenXR
    let xr_instance =
        XrInstance::new(InstanceConfig::default()).map_err(|e| format!("XR init: {e}"))?;

    let mut session = XrSession::new(&xr_instance).map_err(|e| format!("XR session: {e}"))?;

    let mut left_swapchain =
        XrSwapchain::new(SwapchainConfig::default()).map_err(|e| format!("swapchain: {e}"))?;
    let mut right_swapchain =
        XrSwapchain::new(SwapchainConfig::default()).map_err(|e| format!("swapchain: {e}"))?;

    let mut input_actions = XrInputActions::new();
    let mut overlay = OverlayPanel::from_config(&config);

    log::info!("Quest debug overlay initialized");

    // Simulate a few frames (on real Quest, this would be the render loop)
    session.transition_to(XrSessionState::Ready);
    session.transition_to(XrSessionState::Focused);

    let mut frame_count: u64 = 0;

    while session.is_active() && frame_count < 10 {
        let frame_state = frame_loop::wait_frame(&session).map_err(|e| format!("wait: {e}"))?;

        frame_loop::begin_frame(&session).map_err(|e| format!("begin: {e}"))?;

        if frame_state.should_render {
            // Get tracking data
            let snapshot = input_actions.sync_and_snapshot(frame_state.predicted_display_time_ns);

            // Build overlay data
            let overlay_data = DebugOverlayData::from_snapshot(
                &snapshot,
                1000.0 / TARGET_FRAME_TIME_MS,
                TARGET_FRAME_TIME_MS,
                &session.state().to_string(),
                frame_count,
            );

            // Render overlay to RGBA buffer
            overlay.render(&overlay_data);

            // Per-eye rendering (on real Quest: bind swapchain FBO, render scene + overlay quad)
            for swapchain in [&mut left_swapchain, &mut right_swapchain] {
                let _image_idx = swapchain.acquire().map_err(|e| format!("acquire: {e}"))?;
                // In production: glBindFramebuffer, render scene, render overlay quad
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

/// Entry point for Android NativeActivity.
///
/// When the `android_activity` crate is available, this becomes the real entry point.
/// For now, it's a placeholder that can be called from tests.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn android_main() {
    // android_logger::init_once(...);
    let config = OverlayConfig::from_env();
    if let Err(e) = run_debug_overlay(config) {
        log::error!("Fatal: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_debug_overlay_completes() {
        let config = OverlayConfig::default();
        let result = run_debug_overlay(config);
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
        let result = run_debug_overlay(config);
        assert!(result.is_ok());
    }

    #[test]
    fn run_debug_overlay_invisible() {
        let config = OverlayConfig {
            initially_visible: false,
            ..OverlayConfig::default()
        };
        let result = run_debug_overlay(config);
        assert!(result.is_ok());
    }
}
