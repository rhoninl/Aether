mod scene;

use std::time::Instant;

use aether_vr_emulator::{config::ViewMode, HeadsetPreset, VrEmulator};

fn main() {
    let mut emulator =
        VrEmulator::new_windowed(HeadsetPreset::Quest2).expect("failed to create VR emulator");

    let mut frame_time_ms: f32 = 0.0;

    while emulator.is_running() {
        let frame_start = Instant::now();
        let dt = if frame_time_ms > 0.0 {
            frame_time_ms / 1000.0
        } else {
            1.0 / 90.0
        };

        // Update emulator (polls input, updates head/controllers)
        let snapshot = emulator.update(dt);

        // Create framebuffer and clear
        let mut fb = emulator.create_framebuffer();
        fb.clear_sky();

        // Get stereo display for rendering
        let display = emulator.display();

        // Render left eye
        let left_view = emulator.left_eye_view();
        scene::render_eye(&mut fb, display, &left_view, &snapshot);

        // Render right eye (only in stereo mode)
        if display.view_mode() == ViewMode::Stereo {
            let right_view = emulator.right_eye_view();
            scene::render_eye(&mut fb, display, &right_view, &snapshot);
        }

        // Draw debug overlay and present
        emulator
            .present_with_overlay(&mut fb)
            .expect("failed to present");

        frame_time_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_vr_emulator::{EmulatorConfig, VrEmulator};

    fn headless_emulator() -> VrEmulator {
        VrEmulator::new_headless(EmulatorConfig::default()).unwrap()
    }

    #[test]
    fn render_eye_no_panic() {
        let mut emulator = headless_emulator();
        let snapshot = emulator.update(0.016);
        let mut fb = emulator.create_framebuffer();
        fb.clear_sky();
        let display = emulator.display();
        let left_view = emulator.left_eye_view();
        scene::render_eye(&mut fb, display, &left_view, &snapshot);
    }

    #[test]
    fn render_both_eyes_no_panic() {
        let mut emulator = headless_emulator();
        let snapshot = emulator.update(0.016);
        let mut fb = emulator.create_framebuffer();
        fb.clear_sky();
        let display = emulator.display();
        let left_view = emulator.left_eye_view();
        scene::render_eye(&mut fb, display, &left_view, &snapshot);
        let right_view = emulator.right_eye_view();
        scene::render_eye(&mut fb, display, &right_view, &snapshot);
    }

    #[test]
    fn full_frame_renders_without_panic() {
        let mut emulator = headless_emulator();
        let snapshot = emulator.update(0.016);
        let mut fb = emulator.create_framebuffer();
        fb.clear_sky();
        let display = emulator.display();
        let left_view = emulator.left_eye_view();
        scene::render_eye(&mut fb, display, &left_view, &snapshot);
        if display.view_mode() == ViewMode::Stereo {
            let right_view = emulator.right_eye_view();
            scene::render_eye(&mut fb, display, &right_view, &snapshot);
        }
        emulator.present_with_overlay(&mut fb).unwrap();
    }
}
