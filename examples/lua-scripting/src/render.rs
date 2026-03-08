pub const WIDTH: usize = 960;
pub const HEIGHT: usize = 640;

const SKY_TOP: u32 = 0x1a1a2e;
const SKY_BOT: u32 = 0x16213e;
const GROUND_COLOR: u32 = 0x2d4a22;
const GRID_COLOR: u32 = 0x3a5e2c;
const HUD_COLOR: u32 = 0xffffff;
const HUD_DIM: u32 = 0x999999;

// Entity type colors
const GUARD_COLOR: u32 = 0x20e070;
const NPC_COLOR: u32 = 0x60c0ff;
const SPHERE_COLOR: u32 = 0xe07020;
const CUBE_COLOR: u32 = 0x2090e0;
const PLATFORM_COLOR: u32 = 0x808080;
const WAYPOINT_COLOR: u32 = 0xf0e040;
const DEFAULT_COLOR: u32 = 0xc0c0c0;

pub struct Camera {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub fov_deg: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            eye: [20.0, 18.0, -20.0],
            target: [10.0, 0.0, 10.0],
            fov_deg: 60.0,
        }
    }
}

pub struct FrameBuffer {
    pub buf: Vec<u32>,
    depth: Vec<f32>,
}

impl FrameBuffer {
    pub fn new() -> Self {
        Self {
            buf: vec![0; WIDTH * HEIGHT],
            depth: vec![f32::MAX; WIDTH * HEIGHT],
        }
    }

    pub fn clear(&mut self) {
        for y in 0..HEIGHT {
            let t = y as f32 / HEIGHT as f32;
            let r = lerp_u8((SKY_TOP >> 16) as u8, (SKY_BOT >> 16) as u8, t);
            let g = lerp_u8((SKY_TOP >> 8) as u8, (SKY_BOT >> 8) as u8, t);
            let b = lerp_u8(SKY_TOP as u8, SKY_BOT as u8, t);
            let color = (r as u32) << 16 | (g as u32) << 8 | b as u32;
            for x in 0..WIDTH {
                self.buf[y * WIDTH + x] = color;
            }
        }
        self.depth.fill(f32::MAX);
    }

    fn set_pixel(&mut self, x: i32, y: i32, color: u32) {
        if x >= 0 && x < WIDTH as i32 && y >= 0 && y < HEIGHT as i32 {
            self.buf[y as usize * WIDTH + x as usize] = color;
        }
    }

    fn set_pixel_depth(&mut self, x: i32, y: i32, z: f32, color: u32) {
        if x >= 0 && x < WIDTH as i32 && y >= 0 && y < HEIGHT as i32 {
            let idx = y as usize * WIDTH + x as usize;
            if z < self.depth[idx] {
                self.depth[idx] = z;
                self.buf[idx] = color;
            }
        }
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).clamp(0.0, 255.0) as u8
}

fn darken(color: u32, factor: f32) -> u32 {
    let r = (((color >> 16) & 0xff) as f32 * factor) as u32;
    let g = (((color >> 8) & 0xff) as f32 * factor) as u32;
    let b = ((color & 0xff) as f32 * factor) as u32;
    (r.min(255) << 16) | (g.min(255) << 8) | b.min(255)
}

// 3D math
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-8 {
        return [0.0, 0.0, 0.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

struct ViewProj {
    right: [f32; 3],
    up: [f32; 3],
    forward: [f32; 3],
    eye: [f32; 3],
    fov_scale: f32,
    aspect: f32,
    near: f32,
}

impl ViewProj {
    fn new(cam: &Camera) -> Self {
        let forward = normalize(sub(cam.target, cam.eye));
        let world_up = [0.0f32, 1.0, 0.0];
        let right = normalize(cross(forward, world_up));
        let up = cross(right, forward);
        let fov_scale = (cam.fov_deg.to_radians() * 0.5).tan();
        Self {
            right,
            up,
            forward,
            eye: cam.eye,
            fov_scale,
            aspect: WIDTH as f32 / HEIGHT as f32,
            near: 0.1,
        }
    }

    fn project(&self, p: [f32; 3]) -> Option<(f32, f32, f32)> {
        let rel = sub(p, self.eye);
        let z = dot(rel, self.forward);
        if z < self.near {
            return None;
        }
        let x = dot(rel, self.right);
        let y = dot(rel, self.up);
        let px = x / (z * self.fov_scale * self.aspect);
        let py = -y / (z * self.fov_scale);
        let sx = (px * 0.5 + 0.5) * WIDTH as f32;
        let sy = (py * 0.5 + 0.5) * HEIGHT as f32;
        Some((sx, sy, z))
    }

    fn project_radius(&self, center: [f32; 3], radius: f32) -> Option<f32> {
        let rel = sub(center, self.eye);
        let z = dot(rel, self.forward);
        if z < self.near {
            return None;
        }
        Some(radius / (z * self.fov_scale) * (HEIGHT as f32 * 0.5))
    }
}

// Drawing primitives

fn draw_line(fb: &mut FrameBuffer, x0: i32, y0: i32, x1: i32, y1: i32, z: f32, color: u32) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        fb.set_pixel_depth(x, y, z, color);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn draw_circle(fb: &mut FrameBuffer, cx: i32, cy: i32, r: i32, z: f32, color: u32) {
    let mut x = 0;
    let mut y = r;
    let mut d = 3 - 2 * r;
    while x <= y {
        for &(dx, dy) in &[
            (x, y), (-x, y), (x, -y), (-x, -y),
            (y, x), (-y, x), (y, -x), (-y, -x),
        ] {
            fb.set_pixel_depth(cx + dx, cy + dy, z, color);
        }
        if d < 0 {
            d += 4 * x + 6;
        } else {
            d += 4 * (x - y) + 10;
            y -= 1;
        }
        x += 1;
    }
}

fn draw_filled_circle(fb: &mut FrameBuffer, cx: i32, cy: i32, r: i32, z: f32, color: u32) {
    for dy in -r..=r {
        let half_w = ((r * r - dy * dy) as f32).sqrt() as i32;
        for dx in -half_w..=half_w {
            fb.set_pixel_depth(cx + dx, cy + dy, z, color);
        }
    }
}

fn fill_quad(fb: &mut FrameBuffer, pts: &[(f32, f32, f32)], z: f32, color: u32) {
    let min_y = pts.iter().map(|p| p.1 as i32).min().unwrap().max(0);
    let max_y = pts.iter().map(|p| p.1 as i32).max().unwrap().min(HEIGHT as i32 - 1);
    for y in min_y..=max_y {
        let mut min_x = WIDTH as i32;
        let mut max_x = 0i32;
        for i in 0..4 {
            let j = (i + 1) % 4;
            let (x0, y0) = (pts[i].0, pts[i].1);
            let (x1, y1) = (pts[j].0, pts[j].1);
            if (y0 as i32 <= y && (y1 as i32) >= y) || (y1 as i32 <= y && (y0 as i32) >= y) {
                let dy = y1 - y0;
                if dy.abs() > 0.001 {
                    let t = (y as f32 - y0) / dy;
                    let x = (x0 + t * (x1 - x0)) as i32;
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                }
            }
        }
        min_x = min_x.max(0);
        max_x = max_x.min(WIDTH as i32 - 1);
        for x in min_x..=max_x {
            fb.set_pixel_depth(x, y, z, color);
        }
    }
}

// Public rendering functions

pub fn render_ground(fb: &mut FrameBuffer, cam: &Camera) {
    let vp = ViewProj::new(cam);
    let grid_half = 15;
    let spacing = 2.0f32;

    for gz in -grid_half..grid_half {
        for gx in -grid_half..grid_half {
            let corners = [
                [gx as f32 * spacing, 0.0, gz as f32 * spacing],
                [(gx + 1) as f32 * spacing, 0.0, gz as f32 * spacing],
                [(gx + 1) as f32 * spacing, 0.0, (gz + 1) as f32 * spacing],
                [gx as f32 * spacing, 0.0, (gz + 1) as f32 * spacing],
            ];
            let projected: Vec<_> = corners.iter().filter_map(|c| vp.project(*c)).collect();
            if projected.len() == 4 {
                let avg_z = projected.iter().map(|p| p.2).sum::<f32>() / 4.0;
                let is_checker = (gx + gz) % 2 == 0;
                let color = if is_checker {
                    GROUND_COLOR
                } else {
                    darken(GROUND_COLOR, 0.85)
                };
                fill_quad(fb, &projected, avg_z, color);
            }
        }
    }

    for i in -grid_half..=grid_half {
        let p0 = [i as f32 * spacing, 0.01, -grid_half as f32 * spacing];
        let p1 = [i as f32 * spacing, 0.01, grid_half as f32 * spacing];
        if let (Some(a), Some(b)) = (vp.project(p0), vp.project(p1)) {
            draw_line(fb, a.0 as i32, a.1 as i32, b.0 as i32, b.1 as i32, a.2.min(b.2), GRID_COLOR);
        }
        let p0 = [-grid_half as f32 * spacing, 0.01, i as f32 * spacing];
        let p1 = [grid_half as f32 * spacing, 0.01, i as f32 * spacing];
        if let (Some(a), Some(b)) = (vp.project(p0), vp.project(p1)) {
            draw_line(fb, a.0 as i32, a.1 as i32, b.0 as i32, b.1 as i32, a.2.min(b.2), GRID_COLOR);
        }
    }
}

pub fn render_sphere(fb: &mut FrameBuffer, cam: &Camera, pos: [f32; 3], radius: f32, color: u32) {
    let vp = ViewProj::new(cam);
    if let Some((sx, sy, z)) = vp.project(pos) {
        if let Some(sr) = vp.project_radius(pos, radius) {
            let r = sr.max(2.0) as i32;
            let cx = sx as i32;
            let cy = sy as i32;
            for dy in -r..=r {
                let half_w = ((r * r - dy * dy) as f32).sqrt() as i32;
                for dx in -half_w..=half_w {
                    let dist = ((dx * dx + dy * dy) as f32).sqrt() / r as f32;
                    let shade = (1.0 - dist * 0.5).max(0.3);
                    let c = darken(color, shade);
                    fb.set_pixel_depth(cx + dx, cy + dy, z, c);
                }
            }
            draw_circle(fb, cx, cy, r, z - 0.01, darken(color, 1.3));
        }
    }
}

pub fn render_cube(fb: &mut FrameBuffer, cam: &Camera, pos: [f32; 3], half: f32, color: u32) {
    let vp = ViewProj::new(cam);
    let h = half;
    let corners = [
        [pos[0] - h, pos[1] - h, pos[2] - h],
        [pos[0] + h, pos[1] - h, pos[2] - h],
        [pos[0] + h, pos[1] + h, pos[2] - h],
        [pos[0] - h, pos[1] + h, pos[2] - h],
        [pos[0] - h, pos[1] - h, pos[2] + h],
        [pos[0] + h, pos[1] - h, pos[2] + h],
        [pos[0] + h, pos[1] + h, pos[2] + h],
        [pos[0] - h, pos[1] + h, pos[2] + h],
    ];
    let faces: [[usize; 4]; 6] = [
        [0, 1, 2, 3],
        [5, 4, 7, 6],
        [4, 0, 3, 7],
        [1, 5, 6, 2],
        [3, 2, 6, 7],
        [4, 5, 1, 0],
    ];
    let face_shades = [0.9, 0.7, 0.75, 0.85, 1.0, 0.6];
    for (fi, face) in faces.iter().enumerate() {
        let projected: Vec<_> = face.iter().filter_map(|&i| vp.project(corners[i])).collect();
        if projected.len() == 4 {
            let avg_z = projected.iter().map(|p| p.2).sum::<f32>() / 4.0;
            fill_quad(fb, &projected, avg_z, darken(color, face_shades[fi]));
        }
    }
    let edges = [
        (0, 1), (1, 2), (2, 3), (3, 0),
        (4, 5), (5, 6), (6, 7), (7, 4),
        (0, 4), (1, 5), (2, 6), (3, 7),
    ];
    for (a, b) in &edges {
        if let (Some(pa), Some(pb)) = (vp.project(corners[*a]), vp.project(corners[*b])) {
            let z = pa.2.min(pb.2);
            draw_line(fb, pa.0 as i32, pa.1 as i32, pb.0 as i32, pb.1 as i32, z - 0.01, darken(color, 1.4));
        }
    }
}

pub fn render_player(fb: &mut FrameBuffer, cam: &Camera, pos: [f32; 3], color: u32) {
    let vp = ViewProj::new(cam);
    let body_bot = pos;
    let body_top = [pos[0], pos[1] + 1.6, pos[2]];
    let head = [pos[0], pos[1] + 2.0, pos[2]];

    if let (Some(bot), Some(top)) = (vp.project(body_bot), vp.project(body_top)) {
        let z = bot.2.min(top.2);
        for dx in -2..=2 {
            draw_line(fb, bot.0 as i32 + dx, bot.1 as i32, top.0 as i32 + dx, top.1 as i32, z, color);
        }
        let arm_y = [pos[0], pos[1] + 1.2, pos[2]];
        let arm_l = [pos[0] - 0.6, pos[1] + 0.8, pos[2]];
        let arm_r = [pos[0] + 0.6, pos[1] + 0.8, pos[2]];
        if let (Some(ay), Some(al), Some(ar)) = (vp.project(arm_y), vp.project(arm_l), vp.project(arm_r)) {
            draw_line(fb, al.0 as i32, al.1 as i32, ay.0 as i32, ay.1 as i32, z - 0.01, color);
            draw_line(fb, ay.0 as i32, ay.1 as i32, ar.0 as i32, ar.1 as i32, z - 0.01, color);
        }
    }
    if let Some((hx, hy, hz)) = vp.project(head) {
        if let Some(hr) = vp.project_radius(head, 0.25) {
            draw_filled_circle(fb, hx as i32, hy as i32, hr.max(3.0) as i32, hz, color);
        }
    }
}

pub fn render_waypoint(fb: &mut FrameBuffer, cam: &Camera, pos: [f32; 3]) {
    let vp = ViewProj::new(cam);
    // Diamond marker at waypoint
    if let Some((sx, sy, z)) = vp.project(pos) {
        let size = 6i32;
        let cx = sx as i32;
        let cy = sy as i32;
        for i in 0..size {
            let w = size - i;
            for dx in -w..=w {
                fb.set_pixel_depth(cx + dx, cy - i, z, WAYPOINT_COLOR);
                fb.set_pixel_depth(cx + dx, cy + i, z, WAYPOINT_COLOR);
            }
        }
    }
    // Vertical line above
    let top = [pos[0], pos[1] + 1.5, pos[2]];
    if let (Some(b), Some(t)) = (vp.project(pos), vp.project(top)) {
        draw_line(fb, b.0 as i32, b.1 as i32, t.0 as i32, t.1 as i32, b.2, darken(WAYPOINT_COLOR, 0.7));
    }
}

pub fn render_shadow(fb: &mut FrameBuffer, cam: &Camera, pos: [f32; 3], radius: f32) {
    let shadow_pos = [pos[0], 0.02, pos[2]];
    let vp = ViewProj::new(cam);
    if let Some((cx, cy, _z)) = vp.project(shadow_pos) {
        if let Some(sr) = vp.project_radius(shadow_pos, radius) {
            let r = sr.max(1.0) as i32;
            let ry = (r as f32 * 0.35) as i32;
            for dy in -ry..=ry {
                let half_w =
                    ((1.0 - (dy as f32 / ry.max(1) as f32).powi(2)).max(0.0).sqrt() * r as f32)
                        as i32;
                for dx in -half_w..=half_w {
                    let px = cx as i32 + dx;
                    let py = cy as i32 + dy;
                    if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                        let idx = py as usize * WIDTH + px as usize;
                        let c = fb.buf[idx];
                        let cr = (((c >> 16) & 0xff) as f32 * 0.65) as u32;
                        let cg = (((c >> 8) & 0xff) as f32 * 0.65) as u32;
                        let cb = ((c & 0xff) as f32 * 0.65) as u32;
                        fb.buf[idx] = (cr << 16) | (cg << 8) | cb;
                    }
                }
            }
        }
    }
}

/// Returns the color for a given entity template name.
pub fn color_for_template(template: &str) -> u32 {
    match template {
        "patrol_guard" => GUARD_COLOR,
        "hello_npc" => NPC_COLOR,
        "physics_sphere" => SPHERE_COLOR,
        "physics_cube" => CUBE_COLOR,
        "static_platform" => PLATFORM_COLOR,
        _ => DEFAULT_COLOR,
    }
}

// HUD

const FONT: &[u8] = include_bytes!("font5x7.bin");

pub fn draw_text(fb: &mut FrameBuffer, text: &str, mut x: i32, y: i32, color: u32) {
    for ch in text.chars() {
        let idx = if ch.is_ascii() { ch as usize } else { b'?' as usize };
        if idx >= 128 {
            x += 6;
            continue;
        }
        for row in 0..7 {
            let bits = FONT[idx * 7 + row];
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    fb.set_pixel(x + col, y + row as i32, color);
                }
            }
        }
        x += 6;
    }
}

pub fn render_hud(
    fb: &mut FrameBuffer,
    tick: u64,
    entity_count: usize,
    script_count: usize,
    error_count: u64,
    fps_ms: f32,
) {
    draw_text(fb, "Aether Lua Scripting Demo", 10, 10, HUD_COLOR);
    draw_text(
        fb,
        &format!(
            "Tick: {}  Entities: {}  Scripts: {}  Errors: {}",
            tick, entity_count, script_count, error_count
        ),
        10,
        22,
        HUD_COLOR,
    );
    draw_text(fb, &format!("Frame: {:.1}ms", fps_ms), 10, 34, HUD_COLOR);

    // Controls
    draw_text(fb, "Arrow keys: orbit camera  Q/E: zoom  ESC: quit", 10, HEIGHT as i32 - 14, HUD_DIM);

    // Legend
    let ly = HEIGHT as i32 - 36;
    draw_filled_circle(fb, 20, ly, 4, 0.0, GUARD_COLOR);
    draw_text(fb, "Guard", 30, ly - 3, HUD_COLOR);
    draw_filled_circle(fb, 90, ly, 4, 0.0, NPC_COLOR);
    draw_text(fb, "NPC", 100, ly - 3, HUD_COLOR);
    draw_filled_circle(fb, 140, ly, 4, 0.0, SPHERE_COLOR);
    draw_text(fb, "Sphere", 150, ly - 3, HUD_COLOR);
    draw_filled_circle(fb, 210, ly, 4, 0.0, CUBE_COLOR);
    draw_text(fb, "Cube", 220, ly - 3, HUD_COLOR);
    draw_filled_circle(fb, 260, ly, 4, 0.0, PLATFORM_COLOR);
    draw_text(fb, "Platform", 270, ly - 3, HUD_COLOR);
    draw_filled_circle(fb, 340, ly, 4, 0.0, WAYPOINT_COLOR);
    draw_text(fb, "Waypoint", 350, ly - 3, HUD_COLOR);
}
