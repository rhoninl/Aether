//! CPU-side transient effects: particles, floating text, hit rings, screen shake.
//! The renderer never touches these directly — the game ticks them and reads
//! `camera_offset` for shake and iterates the lists for drawing.

const SHAKE_FREQ_X: f32 = 37.0;
const SHAKE_FREQ_Y: f32 = 41.0;
const SHAKE_SEED_MIX: f32 = 0.001;

pub struct Particle {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub life: f32,
    pub color: u32,
}

pub struct FloaterText {
    pub pos: [f32; 3],
    pub text: String,
    pub vel_y: f32,
    pub life: f32,
    pub color: u32,
}

pub struct HitRing {
    pub pos: [f32; 3],
    pub radius: f32,
    pub life: f32,
    pub color: u32,
}

pub struct ScreenShake {
    pub remaining: f32,
    pub intensity: f32,
}

impl ScreenShake {
    fn new() -> Self {
        Self {
            remaining: 0.0,
            intensity: 0.0,
        }
    }
}

pub struct FxState {
    pub particles: Vec<Particle>,
    pub floaters: Vec<FloaterText>,
    pub rings: Vec<HitRing>,
    pub shake: ScreenShake,
}

impl FxState {
    pub fn new() -> Self {
        Self {
            particles: Vec::new(),
            floaters: Vec::new(),
            rings: Vec::new(),
            shake: ScreenShake::new(),
        }
    }

    pub fn tick(&mut self, dt: f32) {
        for p in self.particles.iter_mut() {
            p.pos[0] += p.vel[0] * dt;
            p.pos[1] += p.vel[1] * dt;
            p.pos[2] += p.vel[2] * dt;
            p.life -= dt;
        }
        self.particles.retain(|p| p.life > 0.0);

        for f in self.floaters.iter_mut() {
            f.pos[1] += f.vel_y * dt;
            f.life -= dt;
        }
        self.floaters.retain(|f| f.life > 0.0);

        for r in self.rings.iter_mut() {
            r.life -= dt;
        }
        self.rings.retain(|r| r.life > 0.0);

        if self.shake.remaining > 0.0 {
            self.shake.remaining -= dt;
            if self.shake.remaining <= 0.0 {
                self.shake.remaining = 0.0;
                self.shake.intensity = 0.0;
            }
        }
    }

    pub fn spawn_particle(&mut self, p: Particle) {
        self.particles.push(p);
    }

    pub fn spawn_floater(&mut self, f: FloaterText) {
        self.floaters.push(f);
    }

    pub fn spawn_ring(&mut self, r: HitRing) {
        self.rings.push(r);
    }

    pub fn trigger_shake(&mut self, duration: f32, intensity: f32) {
        if duration > self.shake.remaining {
            self.shake.remaining = duration;
        }
        if intensity > self.shake.intensity {
            self.shake.intensity = intensity;
        }
    }

    /// Deterministic shake offset driven by `remaining * intensity` and a seed.
    /// Uses sine of a phase mixed with the seed so it is jitter-free and
    /// repeatable for the same (remaining, intensity, seed).
    pub fn camera_offset(&self, rng_seed: u64) -> [f32; 2] {
        if self.shake.remaining <= 0.0 || self.shake.intensity <= 0.0 {
            return [0.0, 0.0];
        }
        let seed = (rng_seed as f32) * SHAKE_SEED_MIX;
        let phase = self.shake.remaining + seed;
        let dx = (phase * SHAKE_FREQ_X).sin() * self.shake.intensity;
        let dy = (phase * SHAKE_FREQ_Y).cos() * self.shake.intensity;
        [dx, dy]
    }
}

impl Default for FxState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_particle(life: f32) -> Particle {
        Particle {
            pos: [0.0, 0.0, 0.0],
            vel: [1.0, 2.0, 3.0],
            life,
            color: 0xffffffff,
        }
    }

    #[test]
    fn new_state_is_empty() {
        let fx = FxState::new();
        assert!(fx.particles.is_empty());
        assert!(fx.floaters.is_empty());
        assert!(fx.rings.is_empty());
        assert_eq!(fx.shake.remaining, 0.0);
    }

    #[test]
    fn tick_advances_particle_position_and_life() {
        let mut fx = FxState::new();
        fx.spawn_particle(mk_particle(1.0));
        fx.tick(0.25);
        assert_eq!(fx.particles.len(), 1);
        let p = &fx.particles[0];
        assert!((p.pos[0] - 0.25).abs() < 1e-5);
        assert!((p.pos[1] - 0.5).abs() < 1e-5);
        assert!((p.pos[2] - 0.75).abs() < 1e-5);
        assert!((p.life - 0.75).abs() < 1e-5);
    }

    #[test]
    fn tick_removes_dead_particle() {
        let mut fx = FxState::new();
        fx.spawn_particle(mk_particle(0.1));
        fx.tick(0.2);
        assert!(fx.particles.is_empty());
    }

    #[test]
    fn tick_removes_dead_floater_and_ring() {
        let mut fx = FxState::new();
        fx.spawn_floater(FloaterText {
            pos: [0.0; 3],
            text: "x".into(),
            vel_y: 1.0,
            life: 0.05,
            color: 0xffffffff,
        });
        fx.spawn_ring(HitRing {
            pos: [0.0; 3],
            radius: 1.0,
            life: 0.05,
            color: 0xffffffff,
        });
        fx.tick(0.1);
        assert!(fx.floaters.is_empty());
        assert!(fx.rings.is_empty());
    }

    #[test]
    fn floater_rises_over_time() {
        let mut fx = FxState::new();
        fx.spawn_floater(FloaterText {
            pos: [0.0, 0.0, 0.0],
            text: "hi".into(),
            vel_y: 2.0,
            life: 1.0,
            color: 0xffffffff,
        });
        fx.tick(0.5);
        assert!((fx.floaters[0].pos[1] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn shake_decays() {
        let mut fx = FxState::new();
        fx.trigger_shake(0.5, 2.0);
        assert_eq!(fx.shake.remaining, 0.5);
        fx.tick(0.2);
        assert!((fx.shake.remaining - 0.3).abs() < 1e-5);
    }

    #[test]
    fn shake_clamps_at_zero() {
        let mut fx = FxState::new();
        fx.trigger_shake(0.1, 1.0);
        fx.tick(1.0);
        assert_eq!(fx.shake.remaining, 0.0);
    }

    #[test]
    fn camera_offset_zero_when_not_shaking() {
        let fx = FxState::new();
        assert_eq!(fx.camera_offset(42), [0.0, 0.0]);
    }

    #[test]
    fn camera_offset_nonzero_during_shake() {
        let mut fx = FxState::new();
        fx.trigger_shake(0.5, 1.0);
        let off = fx.camera_offset(42);
        assert!(off[0] != 0.0 || off[1] != 0.0);
    }

    #[test]
    fn camera_offset_deterministic() {
        let mut fx = FxState::new();
        fx.trigger_shake(0.5, 1.0);
        let a = fx.camera_offset(7);
        let b = fx.camera_offset(7);
        assert_eq!(a, b);
    }

    #[test]
    fn trigger_shake_keeps_stronger_intensity() {
        let mut fx = FxState::new();
        fx.trigger_shake(0.5, 1.0);
        fx.trigger_shake(0.1, 5.0);
        assert_eq!(fx.shake.intensity, 5.0);
        assert_eq!(fx.shake.remaining, 0.5);
    }
}
