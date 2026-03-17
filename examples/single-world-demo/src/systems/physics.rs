//! Physics system: bridges aether-physics::PhysicsWorld with ECS Transform
//! and PhysicsBody components.

use aether_ecs::Entity;
use aether_physics::PhysicsWorld;
use aether_physics::WorldPhysicsConfig;

use crate::components::{PhysicsBody, PhysicsBodyType, Transform};

/// Default gravity vector (Earth-like, m/s^2).
pub const DEFAULT_GRAVITY: [f32; 3] = [0.0, -9.81, 0.0];
/// Default physics simulation timestep in seconds (60 Hz).
pub const DEFAULT_PHYSICS_TIMESTEP: f32 = 1.0 / 60.0;

/// Synchronize ECS transforms into the physics world for kinematic bodies.
///
/// For each entity with a `Kinematic` body type, writes the ECS transform
/// position and rotation into the physics world so Rapier can move them
/// to the new position during the next step.
pub fn sync_transforms_to_physics(
    entities: &[(Entity, Transform, PhysicsBody)],
    physics: &mut PhysicsWorld,
) {
    let updates: Vec<(Entity, aether_physics::Transform)> = entities
        .iter()
        .filter(|(_, _, body)| body.body_type == PhysicsBodyType::Kinematic)
        .map(|(entity, transform, _)| {
            let phys_transform = aether_physics::Transform {
                position: transform.position,
                rotation: transform.rotation,
            };
            (*entity, phys_transform)
        })
        .collect();

    physics.sync_from_ecs(&updates);
}

/// Synchronize physics simulation results back into ECS transforms for
/// dynamic bodies.
///
/// Reads positions and rotations from the physics world and writes them
/// into the corresponding ECS `Transform` components.
pub fn sync_physics_to_transforms(
    entities: &mut [(Entity, &mut Transform)],
    physics: &PhysicsWorld,
) {
    for (entity, transform) in entities.iter_mut() {
        if let Some(phys_transform) = physics.get_transform(*entity) {
            transform.position = phys_transform.position;
            transform.rotation = phys_transform.rotation;
        }
    }
}

/// Create a default physics configuration with standard Earth gravity
/// and 60 Hz timestep.
pub fn create_default_physics_config() -> WorldPhysicsConfig {
    WorldPhysicsConfig {
        gravity: DEFAULT_GRAVITY,
        time_step: DEFAULT_PHYSICS_TIMESTEP,
        ..WorldPhysicsConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_gravity_is_earth_like() {
        assert_eq!(DEFAULT_GRAVITY[0], 0.0);
        assert!((DEFAULT_GRAVITY[1] - (-9.81)).abs() < 1e-6);
        assert_eq!(DEFAULT_GRAVITY[2], 0.0);
    }

    #[test]
    fn default_timestep_is_60hz() {
        assert!((DEFAULT_PHYSICS_TIMESTEP - 1.0 / 60.0).abs() < 1e-6);
    }

    #[test]
    fn create_default_config_gravity() {
        let config = create_default_physics_config();
        assert_eq!(config.gravity, DEFAULT_GRAVITY);
    }

    #[test]
    fn create_default_config_timestep() {
        let config = create_default_physics_config();
        assert!((config.time_step - DEFAULT_PHYSICS_TIMESTEP).abs() < 1e-6);
    }

    #[test]
    fn create_default_config_solver_iterations() {
        let config = create_default_physics_config();
        assert_eq!(config.solver_iterations, 4);
    }

    #[test]
    fn create_default_config_ccd_disabled() {
        let config = create_default_physics_config();
        assert!(!config.enable_ccd);
    }

    #[test]
    fn create_default_config_max_velocity() {
        let config = create_default_physics_config();
        assert_eq!(config.max_velocity, 100.0);
    }

    #[test]
    fn sync_transforms_to_physics_filters_kinematic() {
        let config = create_default_physics_config();
        let mut physics = PhysicsWorld::new(&config);

        // We only test the filtering logic here.
        // Dynamic and Static bodies should be ignored by the sync function.
        let entities = vec![
            (
                make_test_entity(0),
                Transform::at(1.0, 2.0, 3.0),
                PhysicsBody {
                    body_type: PhysicsBodyType::Dynamic,
                },
            ),
            (
                make_test_entity(1),
                Transform::at(4.0, 5.0, 6.0),
                PhysicsBody {
                    body_type: PhysicsBodyType::Static,
                },
            ),
        ];

        // Should not panic -- these entities have no bodies in the world,
        // but the function should filter them out (Dynamic/Static are skipped).
        sync_transforms_to_physics(&entities, &mut physics);
    }

    #[test]
    fn sync_transforms_to_physics_with_kinematic_body() {
        let config = create_default_physics_config();
        let mut physics = PhysicsWorld::new(&config);

        let entity = make_test_entity(0);
        physics.add_rigid_body(
            entity,
            aether_physics::BodyType::Kinematic,
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        );

        let entities = vec![(
            entity,
            Transform::at(10.0, 20.0, 30.0),
            PhysicsBody {
                body_type: PhysicsBodyType::Kinematic,
            },
        )];

        sync_transforms_to_physics(&entities, &mut physics);

        // After sync + step, the kinematic body should move to the new position.
        // We step to apply the kinematic target.
        physics.step();

        let phys_transform = physics.get_transform(entity).unwrap();
        assert!((phys_transform.position[0] - 10.0).abs() < 1e-4);
        assert!((phys_transform.position[1] - 20.0).abs() < 1e-4);
        assert!((phys_transform.position[2] - 30.0).abs() < 1e-4);
    }

    #[test]
    fn sync_physics_to_transforms_reads_position() {
        let config = create_default_physics_config();
        let mut physics = PhysicsWorld::new(&config);

        let entity = make_test_entity(0);
        physics.add_rigid_body(
            entity,
            aether_physics::BodyType::Dynamic,
            [5.0, 10.0, 15.0],
            [0.0, 0.0, 0.0, 1.0],
        );

        let mut transform = Transform::default();
        let mut entities: Vec<(Entity, &mut Transform)> = vec![(entity, &mut transform)];

        sync_physics_to_transforms(&mut entities, &physics);

        assert!((transform.position[0] - 5.0).abs() < 1e-4);
        assert!((transform.position[1] - 10.0).abs() < 1e-4);
        assert!((transform.position[2] - 15.0).abs() < 1e-4);
    }

    #[test]
    fn sync_physics_to_transforms_unknown_entity_unchanged() {
        let config = create_default_physics_config();
        let physics = PhysicsWorld::new(&config);

        let mut transform = Transform::at(1.0, 2.0, 3.0);
        let entity = make_test_entity(99);
        let mut entities: Vec<(Entity, &mut Transform)> = vec![(entity, &mut transform)];

        sync_physics_to_transforms(&mut entities, &physics);

        // Transform should remain unchanged since entity is not in physics
        assert_eq!(transform.position, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn sync_transforms_to_physics_empty_list() {
        let config = create_default_physics_config();
        let mut physics = PhysicsWorld::new(&config);
        sync_transforms_to_physics(&[], &mut physics);
        // Should not panic
    }

    #[test]
    fn sync_physics_to_transforms_empty_list() {
        let config = create_default_physics_config();
        let physics = PhysicsWorld::new(&config);
        let mut entities: Vec<(Entity, &mut Transform)> = vec![];
        sync_physics_to_transforms(&mut entities, &physics);
        // Should not panic
    }

    #[test]
    fn sync_physics_to_transforms_reads_rotation() {
        let config = create_default_physics_config();
        let mut physics = PhysicsWorld::new(&config);

        let entity = make_test_entity(0);
        physics.add_rigid_body(
            entity,
            aether_physics::BodyType::Dynamic,
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        );

        let mut transform = Transform::default();
        let mut entities: Vec<(Entity, &mut Transform)> = vec![(entity, &mut transform)];

        sync_physics_to_transforms(&mut entities, &physics);

        // Identity quaternion
        let len = transform.rotation.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((len - 1.0).abs() < 1e-4);
    }

    /// Helper to create a test entity. Entity fields are pub(crate), so we
    /// use a small workaround via transmute of the same layout.
    fn make_test_entity(index: u32) -> Entity {
        // SAFETY: Entity is repr(C)-compatible: two u32 fields (index, generation).
        unsafe { std::mem::transmute::<(u32, u32), Entity>((index, 0)) }
    }
}
