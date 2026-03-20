//! Simple 3D scene rendering for the VR emulator demo.
//!
//! Renders a ground grid, some floating objects, and visualizations of
//! the emulated VR controllers within each eye's viewport.

use aether_input::openxr_tracking::TrackingSnapshot;
use aether_vr_emulator::display::{EyeView, StereoDisplay};
use aether_vr_emulator::window::EmulatorFrameBuffer;

/// Grid line color.
const GRID_LINE_COLOR: u32 = 0x3a5e2c;

/// Object colors.
const CUBE_COLOR: u32 = 0x2090e0;
const SPHERE_COLOR: u32 = 0xe07020;
const CONTROLLER_COLOR: u32 = 0xff3060;
const CONTROLLER_BEAM_COLOR: u32 = 0x60ff90;

/// Scene objects: some floating cubes and spheres at fixed positions.
const CUBES: [[f32; 3]; 4] = [
    [-2.0, 1.0, -5.0],
    [2.0, 1.5, -4.0],
    [0.0, 2.0, -7.0],
    [-3.0, 0.5, -3.0],
];

const SPHERES: [[f32; 3]; 3] = [[1.0, 1.0, -6.0], [-1.5, 2.5, -5.0], [3.0, 1.0, -8.0]];

/// Render the scene from one eye's perspective.
pub fn render_eye(
    fb: &mut EmulatorFrameBuffer,
    display: &StereoDisplay,
    eye_view: &EyeView,
    snapshot: &TrackingSnapshot,
) {
    render_ground_grid(fb, display, eye_view);
    render_cubes(fb, display, eye_view);
    render_spheres(fb, display, eye_view);
    render_controller_marker(
        fb,
        display,
        eye_view,
        snapshot.left_controller.grip_pose.position,
    );
    render_controller_marker(
        fb,
        display,
        eye_view,
        snapshot.right_controller.grip_pose.position,
    );
    render_controller_beam(
        fb,
        display,
        eye_view,
        snapshot.right_controller.grip_pose.position,
        snapshot.right_controller.aim_pose.rotation,
    );
}

/// Render a checkerboard ground grid.
fn render_ground_grid(fb: &mut EmulatorFrameBuffer, display: &StereoDisplay, eye_view: &EyeView) {
    let grid_half = 10;
    let spacing = 1.0f32;

    // Draw grid lines
    for i in -grid_half..=grid_half {
        let p0 = [i as f32 * spacing, 0.0, -grid_half as f32 * spacing];
        let p1 = [i as f32 * spacing, 0.0, grid_half as f32 * spacing];
        if let (Some(a), Some(b)) = (
            display.project_point(eye_view, p0),
            display.project_point(eye_view, p1),
        ) {
            draw_line_clipped(fb, eye_view, a, b, GRID_LINE_COLOR);
        }

        let p0 = [-grid_half as f32 * spacing, 0.0, i as f32 * spacing];
        let p1 = [grid_half as f32 * spacing, 0.0, i as f32 * spacing];
        if let (Some(a), Some(b)) = (
            display.project_point(eye_view, p0),
            display.project_point(eye_view, p1),
        ) {
            draw_line_clipped(fb, eye_view, a, b, GRID_LINE_COLOR);
        }
    }
}

/// Render cubes as wireframe boxes.
fn render_cubes(fb: &mut EmulatorFrameBuffer, display: &StereoDisplay, eye_view: &EyeView) {
    for pos in &CUBES {
        render_wireframe_cube(fb, display, eye_view, *pos, 0.4, CUBE_COLOR);
    }
}

/// Render spheres as filled circles.
fn render_spheres(fb: &mut EmulatorFrameBuffer, display: &StereoDisplay, eye_view: &EyeView) {
    for pos in &SPHERES {
        if let Some((sx, sy, _z)) = display.project_point(eye_view, *pos) {
            let radius_px = project_radius(eye_view, *pos, 0.3);
            if let Some(r) = radius_px {
                let r = r.max(2.0) as i32;
                draw_filled_circle_viewport(fb, eye_view, sx as i32, sy as i32, r, SPHERE_COLOR);
            }
        }
    }
}

/// Render a VR controller as a small marker.
fn render_controller_marker(
    fb: &mut EmulatorFrameBuffer,
    display: &StereoDisplay,
    eye_view: &EyeView,
    position: [f32; 3],
) {
    if let Some((sx, sy, _z)) = display.project_point(eye_view, position) {
        draw_filled_circle_viewport(fb, eye_view, sx as i32, sy as i32, 5, CONTROLLER_COLOR);
        // Draw crosshair
        for d in -8..=8 {
            set_pixel_viewport(fb, eye_view, sx as i32 + d, sy as i32, CONTROLLER_COLOR);
            set_pixel_viewport(fb, eye_view, sx as i32, sy as i32 + d, CONTROLLER_COLOR);
        }
    }
}

/// Render a pointing beam from the right controller.
fn render_controller_beam(
    fb: &mut EmulatorFrameBuffer,
    display: &StereoDisplay,
    eye_view: &EyeView,
    position: [f32; 3],
    rotation: [f32; 4],
) {
    // Compute forward direction from quaternion
    let forward = quat_forward(rotation);
    let beam_length = 5.0;
    let end = [
        position[0] + forward[0] * beam_length,
        position[1] + forward[1] * beam_length,
        position[2] + forward[2] * beam_length,
    ];

    if let (Some(a), Some(b)) = (
        display.project_point(eye_view, position),
        display.project_point(eye_view, end),
    ) {
        draw_line_clipped(fb, eye_view, a, b, CONTROLLER_BEAM_COLOR);
    }
}

/// Render a wireframe cube.
fn render_wireframe_cube(
    fb: &mut EmulatorFrameBuffer,
    display: &StereoDisplay,
    eye_view: &EyeView,
    center: [f32; 3],
    half: f32,
    color: u32,
) {
    let corners = [
        [center[0] - half, center[1] - half, center[2] - half],
        [center[0] + half, center[1] - half, center[2] - half],
        [center[0] + half, center[1] + half, center[2] - half],
        [center[0] - half, center[1] + half, center[2] - half],
        [center[0] - half, center[1] - half, center[2] + half],
        [center[0] + half, center[1] - half, center[2] + half],
        [center[0] + half, center[1] + half, center[2] + half],
        [center[0] - half, center[1] + half, center[2] + half],
    ];

    let edges: [(usize, usize); 12] = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];

    for (a, b) in &edges {
        if let (Some(pa), Some(pb)) = (
            display.project_point(eye_view, corners[*a]),
            display.project_point(eye_view, corners[*b]),
        ) {
            draw_line_clipped(fb, eye_view, pa, pb, color);
        }
    }
}

/// Draw a line between two projected points, clipped to the eye viewport.
fn draw_line_clipped(
    fb: &mut EmulatorFrameBuffer,
    eye_view: &EyeView,
    a: (f32, f32, f32),
    b: (f32, f32, f32),
    color: u32,
) {
    let (x0, y0) = (a.0 as i32, a.1 as i32);
    let (x1, y1) = (b.0 as i32, b.1 as i32);

    // Bresenham line drawing with viewport clipping
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;

    loop {
        set_pixel_viewport(fb, eye_view, x, y, color);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            if x == x1 {
                break;
            }
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            if y == y1 {
                break;
            }
            err += dx;
            y += sy;
        }
    }
}

/// Draw a filled circle clipped to the eye viewport.
fn draw_filled_circle_viewport(
    fb: &mut EmulatorFrameBuffer,
    eye_view: &EyeView,
    cx: i32,
    cy: i32,
    r: i32,
    color: u32,
) {
    for dy in -r..=r {
        let half_w = ((r * r - dy * dy) as f32).sqrt() as i32;
        for dx in -half_w..=half_w {
            set_pixel_viewport(fb, eye_view, cx + dx, cy + dy, color);
        }
    }
}

/// Set a pixel, clipped to the eye viewport bounds.
fn set_pixel_viewport(
    fb: &mut EmulatorFrameBuffer,
    eye_view: &EyeView,
    x: i32,
    y: i32,
    color: u32,
) {
    let vp = &eye_view.viewport;
    let vx = vp.x as i32;
    let vy = vp.y as i32;
    let vw = vp.width as i32;
    let vh = vp.height as i32;

    if x >= vx && x < vx + vw && y >= vy && y < vy + vh {
        fb.set_pixel(x, y, color);
    }
}

/// Project a world-space radius to screen pixels for a given eye view.
fn project_radius(eye_view: &EyeView, center: [f32; 3], radius: f32) -> Option<f32> {
    let rel = [
        center[0] - eye_view.position[0],
        center[1] - eye_view.position[1],
        center[2] - eye_view.position[2],
    ];
    let z = dot(rel, eye_view.forward);
    if z < 0.1 {
        return None;
    }
    let fov_scale = (eye_view.v_fov_rad * 0.5).tan();
    Some(radius / (z * fov_scale) * (eye_view.viewport.height as f32 * 0.5))
}

/// Compute forward direction from a quaternion [x, y, z, w].
fn quat_forward(q: [f32; 4]) -> [f32; 3] {
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    // Forward direction is -Z rotated by the quaternion
    [
        -2.0 * (x * z + w * y),
        -2.0 * (y * z - w * x),
        -(1.0 - 2.0 * (x * x + y * y)),
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-3;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn quat_forward_identity_is_neg_z() {
        let fwd = quat_forward([0.0, 0.0, 0.0, 1.0]);
        assert!(approx_eq(fwd[0], 0.0), "x={}", fwd[0]);
        assert!(approx_eq(fwd[1], 0.0), "y={}", fwd[1]);
        assert!(approx_eq(fwd[2], -1.0), "z={}", fwd[2]);
    }

    #[test]
    fn quat_forward_90_yaw_right() {
        // 90 degrees around Y: q = (0, sin(45), 0, cos(45))
        let s = std::f32::consts::FRAC_PI_4.sin();
        let c = std::f32::consts::FRAC_PI_4.cos();
        let fwd = quat_forward([0.0, s, 0.0, c]);
        // After 90 degree yaw, forward should be along -X
        assert!(approx_eq(fwd[0], -1.0), "x={}", fwd[0]);
        assert!(approx_eq(fwd[2], 0.0), "z={}", fwd[2]);
    }

    #[test]
    fn draw_line_clipped_no_panic() {
        let mut fb = EmulatorFrameBuffer::new(200, 200);
        fb.clear_color(0x000000);
        let eye_view = EyeView {
            position: [0.0, 0.0, 0.0],
            forward: [0.0, 0.0, -1.0],
            up: [0.0, 1.0, 0.0],
            right: [1.0, 0.0, 0.0],
            h_fov_rad: 90.0f32.to_radians(),
            v_fov_rad: 90.0f32.to_radians(),
            viewport: aether_vr_emulator::Viewport {
                x: 0,
                y: 0,
                width: 200,
                height: 200,
            },
        };
        draw_line_clipped(
            &mut fb,
            &eye_view,
            (10.0, 10.0, 1.0),
            (190.0, 190.0, 1.0),
            0xffffff,
        );
    }

    #[test]
    fn set_pixel_viewport_clips_outside() {
        let mut fb = EmulatorFrameBuffer::new(200, 200);
        fb.clear_color(0x000000);
        let eye_view = EyeView {
            position: [0.0, 0.0, 0.0],
            forward: [0.0, 0.0, -1.0],
            up: [0.0, 1.0, 0.0],
            right: [1.0, 0.0, 0.0],
            h_fov_rad: 90.0f32.to_radians(),
            v_fov_rad: 90.0f32.to_radians(),
            viewport: aether_vr_emulator::Viewport {
                x: 0,
                y: 0,
                width: 100,
                height: 200,
            },
        };
        // Pixel at x=150 should be clipped (viewport width is 100)
        set_pixel_viewport(&mut fb, &eye_view, 150, 50, 0xffffff);
        assert_eq!(fb.pixels[50 * 200 + 150], 0x000000);

        // Pixel at x=50 should be drawn
        set_pixel_viewport(&mut fb, &eye_view, 50, 50, 0xffffff);
        assert_eq!(fb.pixels[50 * 200 + 50], 0xffffff);
    }
}
