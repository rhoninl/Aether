/// Per-world physics configuration.
#[derive(Debug, Clone)]
pub struct WorldPhysicsConfig {
    pub gravity: [f32; 3],
    pub time_step: f32,
    pub max_velocity: f32,
    pub enable_ccd: bool,
    pub solver_iterations: u8,
}

const DEFAULT_GRAVITY: [f32; 3] = [0.0, -9.81, 0.0];
const DEFAULT_TIME_STEP: f32 = 1.0 / 60.0;
const DEFAULT_MAX_VELOCITY: f32 = 100.0;
const DEFAULT_SOLVER_ITERATIONS: u8 = 4;

impl Default for WorldPhysicsConfig {
    fn default() -> Self {
        Self {
            gravity: DEFAULT_GRAVITY,
            time_step: DEFAULT_TIME_STEP,
            max_velocity: DEFAULT_MAX_VELOCITY,
            enable_ccd: false,
            solver_iterations: DEFAULT_SOLVER_ITERATIONS,
        }
    }
}

impl WorldPhysicsConfig {
    pub fn zero_gravity() -> Self {
        Self {
            gravity: [0.0, 0.0, 0.0],
            ..Default::default()
        }
    }

    pub fn low_gravity() -> Self {
        Self {
            gravity: [0.0, -1.62, 0.0], // Moon-like
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = WorldPhysicsConfig::default();
        assert_eq!(config.gravity, [0.0, -9.81, 0.0]);
        assert!((config.time_step - 1.0 / 60.0).abs() < f32::EPSILON);
        assert_eq!(config.max_velocity, 100.0);
        assert!(!config.enable_ccd);
        assert_eq!(config.solver_iterations, 4);
    }

    #[test]
    fn zero_gravity_config() {
        let config = WorldPhysicsConfig::zero_gravity();
        assert_eq!(config.gravity, [0.0, 0.0, 0.0]);
        assert_eq!(config.solver_iterations, DEFAULT_SOLVER_ITERATIONS);
    }

    #[test]
    fn low_gravity_config() {
        let config = WorldPhysicsConfig::low_gravity();
        assert_eq!(config.gravity[1], -1.62);
    }

    #[test]
    fn custom_config() {
        let config = WorldPhysicsConfig {
            gravity: [0.0, -20.0, 0.0],
            time_step: 1.0 / 120.0,
            max_velocity: 50.0,
            enable_ccd: true,
            solver_iterations: 8,
        };
        assert_eq!(config.gravity[1], -20.0);
        assert!(config.enable_ccd);
        assert_eq!(config.solver_iterations, 8);
    }
}
