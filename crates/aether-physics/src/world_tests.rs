use super::*;

fn entity(index: u32) -> Entity {
    unsafe { std::mem::transmute::<(u32, u32), Entity>((index, 0)) }
}

fn default_config() -> WorldPhysicsConfig {
    WorldPhysicsConfig::default()
}

#[test]
fn create_world_with_default_config() {
    let world = PhysicsWorld::new(&default_config());
    assert_eq!(world.gravity(), [0.0, -9.81, 0.0]);
    assert_eq!(world.body_count(), 0);
    assert_eq!(world.collider_count(), 0);
}

#[test]
fn create_world_with_zero_gravity() {
    let config = WorldPhysicsConfig::zero_gravity();
    let world = PhysicsWorld::new(&config);
    assert_eq!(world.gravity(), [0.0, 0.0, 0.0]);
}

#[test]
fn create_world_with_custom_config() {
    let config = WorldPhysicsConfig {
        gravity: [0.0, -20.0, 0.0],
        time_step: 1.0 / 120.0,
        max_velocity: 50.0,
        enable_ccd: false,
        solver_iterations: 8,
    };
    let world = PhysicsWorld::new(&config);
    assert_eq!(world.gravity(), [0.0, -20.0, 0.0]);
}

#[test]
fn add_dynamic_body() {
    let mut world = PhysicsWorld::new(&default_config());
    let e = entity(0);
    let handle =
        world.add_rigid_body(e, BodyType::Dynamic, [0.0, 10.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    assert!(handle.is_some());
    assert_eq!(world.body_count(), 1);
    assert!(world.has_body(e));
}

#[test]
fn add_kinematic_body() {
    let mut world = PhysicsWorld::new(&default_config());
    let e = entity(0);
    let handle =
        world.add_rigid_body(e, BodyType::Kinematic, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    assert!(handle.is_some());
    assert_eq!(world.body_count(), 1);
}

#[test]
fn add_static_body() {
    let mut world = PhysicsWorld::new(&default_config());
    let e = entity(0);
    let handle =
        world.add_rigid_body(e, BodyType::Static, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    assert!(handle.is_some());
    assert_eq!(world.body_count(), 1);
}

#[test]
fn duplicate_body_returns_none() {
    let mut world = PhysicsWorld::new(&default_config());
    let e = entity(0);
    assert!(world
        .add_rigid_body(e, BodyType::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0])
        .is_some());
    assert!(world
        .add_rigid_body(e, BodyType::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0])
        .is_none());
    assert_eq!(world.body_count(), 1);
}

#[test]
fn add_sphere_collider() {
    let mut world = PhysicsWorld::new(&default_config());
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Sphere { radius: 0.5 };
    let handle = world.add_collider(e, &shape, false, 0.5, 0.3, 1.0, &CollisionLayers::default());
    assert!(handle.is_some());
    assert_eq!(world.collider_count(), 1);
}

#[test]
fn add_box_collider() {
    let mut world = PhysicsWorld::new(&default_config());
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Box {
        half_extents: [1.0, 1.0, 1.0],
    };
    let handle = world.add_collider(e, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());
    assert!(handle.is_some());
}

#[test]
fn add_capsule_collider() {
    let mut world = PhysicsWorld::new(&default_config());
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Capsule {
        half_height: 0.5,
        radius: 0.3,
    };
    let handle = world.add_collider(e, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());
    assert!(handle.is_some());
}

#[test]
fn add_cylinder_collider() {
    let mut world = PhysicsWorld::new(&default_config());
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Cylinder {
        half_height: 1.0,
        radius: 0.5,
    };
    let handle = world.add_collider(e, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());
    assert!(handle.is_some());
}

#[test]
fn add_sensor_collider() {
    let mut world = PhysicsWorld::new(&default_config());
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Sphere { radius: 1.0 };
    let handle = world.add_collider(
        e,
        &shape,
        true, // sensor
        0.5,
        0.0,
        1.0,
        &CollisionLayers::default(),
    );
    assert!(handle.is_some());
}

#[test]
fn collider_without_body_returns_none() {
    let mut world = PhysicsWorld::new(&default_config());
    let e = entity(99);
    let shape = ColliderShape::Sphere { radius: 0.5 };
    let handle = world.add_collider(e, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());
    assert!(handle.is_none());
}

#[test]
fn remove_entity_cleans_up() {
    let mut world = PhysicsWorld::new(&default_config());
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Sphere { radius: 0.5 };
    world.add_collider(e, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());

    assert!(world.remove_entity(e));
    assert_eq!(world.body_count(), 0);
    assert!(!world.has_body(e));
}

#[test]
fn remove_nonexistent_entity_returns_false() {
    let mut world = PhysicsWorld::new(&default_config());
    assert!(!world.remove_entity(entity(99)));
}

#[test]
fn ball_drops_under_gravity() {
    let mut world = PhysicsWorld::new(&default_config());
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Dynamic, [0.0, 10.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Sphere { radius: 0.5 };
    world.add_collider(e, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());

    let initial_y = world.get_transform(e).unwrap().position[1];

    for _ in 0..10 {
        world.step();
    }

    let final_y = world.get_transform(e).unwrap().position[1];
    assert!(
        final_y < initial_y,
        "Ball should fall: initial_y={initial_y}, final_y={final_y}"
    );
}

#[test]
fn zero_gravity_no_movement() {
    let config = WorldPhysicsConfig::zero_gravity();
    let mut world = PhysicsWorld::new(&config);
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Dynamic, [0.0, 10.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Sphere { radius: 0.5 };
    world.add_collider(e, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());

    for _ in 0..10 {
        world.step();
    }

    let pos = world.get_transform(e).unwrap().position;
    assert!(
        (pos[1] - 10.0).abs() < 0.001,
        "Ball should not move in zero gravity: y={}",
        pos[1]
    );
}

#[test]
fn static_body_does_not_fall() {
    let mut world = PhysicsWorld::new(&default_config());
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Static, [0.0, 5.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Sphere { radius: 0.5 };
    world.add_collider(e, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());

    for _ in 0..10 {
        world.step();
    }

    let pos = world.get_transform(e).unwrap().position;
    assert_eq!(pos[1], 5.0, "Static body should not move");
}

#[test]
fn sync_to_ecs_returns_dynamic_bodies() {
    let mut world = PhysicsWorld::new(&default_config());
    let e_dyn = entity(0);
    let e_stat = entity(1);
    world.add_rigid_body(
        e_dyn,
        BodyType::Dynamic,
        [0.0, 10.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    );
    world.add_rigid_body(
        e_stat,
        BodyType::Static,
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    );

    let shape = ColliderShape::Sphere { radius: 0.5 };
    world.add_collider(
        e_dyn,
        &shape,
        false,
        0.5,
        0.0,
        1.0,
        &CollisionLayers::default(),
    );
    world.add_collider(
        e_stat,
        &shape,
        false,
        0.5,
        0.0,
        1.0,
        &CollisionLayers::default(),
    );

    world.step();

    let results = world.sync_to_ecs();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, e_dyn);
}

#[test]
fn sync_from_ecs_moves_kinematic_body() {
    let mut world = PhysicsWorld::new(&default_config());
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Kinematic, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Sphere { radius: 0.5 };
    world.add_collider(e, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());

    let new_transform = Transform::from_position(5.0, 5.0, 5.0);
    world.sync_from_ecs(&[(e, new_transform)]);
    world.step();

    let pos = world.get_transform(e).unwrap().position;
    assert!(
        (pos[0] - 5.0).abs() < 0.1,
        "Kinematic body should move to new position: x={}",
        pos[0]
    );
}

#[test]
fn raycast_hits_body() {
    let mut world = PhysicsWorld::new(&WorldPhysicsConfig::zero_gravity());
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Static, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Sphere { radius: 1.0 };
    world.add_collider(e, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());

    world.step();

    let hit = world.raycast(
        [0.0, 10.0, 0.0],
        [0.0, -1.0, 0.0],
        100.0,
        &QueryFilter::default(),
    );

    assert!(hit.is_some(), "Raycast should hit the sphere");
    let hit = hit.unwrap();
    assert_eq!(hit.entity, e);
    assert!(
        (hit.distance - 9.0).abs() < 0.1,
        "Distance should be ~9.0, got {}",
        hit.distance
    );
}

#[test]
fn raycast_misses() {
    let mut world = PhysicsWorld::new(&WorldPhysicsConfig::zero_gravity());
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Static, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Sphere { radius: 1.0 };
    world.add_collider(e, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());
    world.step();

    let hit = world.raycast(
        [0.0, 10.0, 0.0],
        [0.0, 1.0, 0.0], // pointing away
        100.0,
        &QueryFilter::default(),
    );

    assert!(hit.is_none(), "Raycast should miss");
}

#[test]
fn raycast_with_exclude() {
    let mut world = PhysicsWorld::new(&WorldPhysicsConfig::zero_gravity());
    let e1 = entity(0);
    let e2 = entity(1);

    world.add_rigid_body(e1, BodyType::Static, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    world.add_rigid_body(e2, BodyType::Static, [0.0, -5.0, 0.0], [0.0, 0.0, 0.0, 1.0]);

    let shape = ColliderShape::Sphere { radius: 1.0 };
    world.add_collider(e1, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());
    world.add_collider(e2, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());
    world.step();

    let filter = QueryFilter::new().excluding(e1);
    let hit = world.raycast([0.0, 10.0, 0.0], [0.0, -1.0, 0.0], 100.0, &filter);

    assert!(hit.is_some(), "Should hit e2 after excluding e1");
    assert_eq!(hit.unwrap().entity, e2);
}

#[test]
fn raycast_all_hits_multiple() {
    let mut world = PhysicsWorld::new(&WorldPhysicsConfig::zero_gravity());
    let e1 = entity(0);
    let e2 = entity(1);

    world.add_rigid_body(e1, BodyType::Static, [0.0, 5.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    world.add_rigid_body(e2, BodyType::Static, [0.0, -5.0, 0.0], [0.0, 0.0, 0.0, 1.0]);

    let shape = ColliderShape::Sphere { radius: 1.0 };
    world.add_collider(e1, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());
    world.add_collider(e2, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());
    world.step();

    let hits = world.raycast_all(
        [0.0, 20.0, 0.0],
        [0.0, -1.0, 0.0],
        100.0,
        &QueryFilter::default(),
    );
    assert_eq!(hits.len(), 2, "Should hit both spheres");
}

#[test]
fn add_fixed_joint() {
    let mut world = PhysicsWorld::new(&default_config());
    let e1 = entity(0);
    let e2 = entity(1);
    world.add_rigid_body(e1, BodyType::Dynamic, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    world.add_rigid_body(e2, BodyType::Dynamic, [2.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);

    let joint = JointType::fixed([1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]);
    let handle = world.add_joint(e1, e2, &joint);
    assert!(handle.is_some());
}

#[test]
fn add_revolute_joint() {
    let mut world = PhysicsWorld::new(&default_config());
    let e1 = entity(0);
    let e2 = entity(1);
    world.add_rigid_body(e1, BodyType::Dynamic, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    world.add_rigid_body(e2, BodyType::Dynamic, [2.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);

    let joint = JointType::revolute([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]);
    let handle = world.add_joint(e1, e2, &joint);
    assert!(handle.is_some());
}

#[test]
fn add_prismatic_joint() {
    let mut world = PhysicsWorld::new(&default_config());
    let e1 = entity(0);
    let e2 = entity(1);
    world.add_rigid_body(e1, BodyType::Dynamic, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    world.add_rigid_body(e2, BodyType::Dynamic, [0.0, 2.0, 0.0], [0.0, 0.0, 0.0, 1.0]);

    let joint = JointType::prismatic(
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
        Some([-1.0, 1.0]),
    );
    let handle = world.add_joint(e1, e2, &joint);
    assert!(handle.is_some());
}

#[test]
fn joint_with_missing_entity_returns_none() {
    let mut world = PhysicsWorld::new(&default_config());
    let e1 = entity(0);
    let e2 = entity(1);
    world.add_rigid_body(e1, BodyType::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]);

    let joint = JointType::fixed([0.0; 3], [0.0; 3]);
    assert!(world.add_joint(e1, e2, &joint).is_none());
}

#[test]
fn remove_joint() {
    let mut world = PhysicsWorld::new(&default_config());
    let e1 = entity(0);
    let e2 = entity(1);
    world.add_rigid_body(e1, BodyType::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]);
    world.add_rigid_body(
        e2,
        BodyType::Dynamic,
        [2.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    );

    let joint = JointType::fixed([1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]);
    let handle = world.add_joint(e1, e2, &joint).unwrap();
    world.remove_joint(handle);
}

#[test]
fn apply_force_to_dynamic_body() {
    let mut world = PhysicsWorld::new(&WorldPhysicsConfig::zero_gravity());
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Sphere { radius: 0.5 };
    world.add_collider(e, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());

    assert!(world.apply_force(e, [100.0, 0.0, 0.0]));

    world.step();

    let vel = world.get_velocity(e).unwrap();
    assert!(
        vel.linear[0] > 0.0,
        "Body should have positive x velocity after force"
    );
}

#[test]
fn apply_impulse_to_dynamic_body() {
    let mut world = PhysicsWorld::new(&WorldPhysicsConfig::zero_gravity());
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Sphere { radius: 0.5 };
    world.add_collider(e, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());

    assert!(world.apply_impulse(e, [0.0, 10.0, 0.0]));

    world.step();

    let pos = world.get_transform(e).unwrap().position;
    assert!(pos[1] > 0.0, "Body should move upward after impulse");
}

#[test]
fn set_linear_velocity() {
    let mut world = PhysicsWorld::new(&WorldPhysicsConfig::zero_gravity());
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Sphere { radius: 0.5 };
    world.add_collider(e, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());

    assert!(world.set_linear_velocity(e, [5.0, 0.0, 0.0]));

    world.step();

    let pos = world.get_transform(e).unwrap().position;
    assert!(
        pos[0] > 0.0,
        "Body should move along x with set velocity"
    );
}

#[test]
fn set_angular_velocity() {
    let mut world = PhysicsWorld::new(&WorldPhysicsConfig::zero_gravity());
    let e = entity(0);
    world.add_rigid_body(e, BodyType::Dynamic, [0.0; 3], [0.0, 0.0, 0.0, 1.0]);
    let shape = ColliderShape::Sphere { radius: 0.5 };
    world.add_collider(e, &shape, false, 0.5, 0.0, 1.0, &CollisionLayers::default());

    assert!(world.set_angular_velocity(e, [0.0, 0.0, 5.0]));

    world.step();

    let vel = world.get_velocity(e).unwrap();
    assert!(
        vel.angular[2].abs() > 0.0,
        "Body should be rotating around z"
    );
}

#[test]
fn force_on_nonexistent_entity_returns_false() {
    let mut world = PhysicsWorld::new(&default_config());
    assert!(!world.apply_force(entity(99), [1.0, 0.0, 0.0]));
    assert!(!world.apply_impulse(entity(99), [1.0, 0.0, 0.0]));
    assert!(!world.set_linear_velocity(entity(99), [1.0, 0.0, 0.0]));
    assert!(!world.set_angular_velocity(entity(99), [1.0, 0.0, 0.0]));
}

#[test]
fn get_transform_for_nonexistent_entity() {
    let world = PhysicsWorld::new(&default_config());
    assert!(world.get_transform(entity(99)).is_none());
}

#[test]
fn get_velocity_for_nonexistent_entity() {
    let world = PhysicsWorld::new(&default_config());
    assert!(world.get_velocity(entity(99)).is_none());
}

#[test]
fn set_gravity() {
    let mut world = PhysicsWorld::new(&default_config());
    world.set_gravity([0.0, 0.0, 0.0]);
    assert_eq!(world.gravity(), [0.0, 0.0, 0.0]);
}

#[test]
fn collision_events_between_falling_bodies() {
    let mut world = PhysicsWorld::new(&default_config());

    // Static floor
    let floor = entity(0);
    world.add_rigid_body(floor, BodyType::Static, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    let floor_shape = ColliderShape::Box {
        half_extents: [50.0, 0.5, 50.0],
    };
    world.add_collider(
        floor,
        &floor_shape,
        false,
        0.5,
        0.0,
        1.0,
        &CollisionLayers::default(),
    );

    // Falling ball
    let ball = entity(1);
    world.add_rigid_body(ball, BodyType::Dynamic, [0.0, 3.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    let ball_shape = ColliderShape::Sphere { radius: 0.5 };
    world.add_collider(
        ball,
        &ball_shape,
        false,
        0.5,
        0.3,
        1.0,
        &CollisionLayers::default(),
    );

    let mut found_collision = false;
    for _ in 0..300 {
        world.step();
        let events = world.drain_collision_events();
        if events.iter().any(|e| e.is_started()) {
            found_collision = true;
            break;
        }
    }

    assert!(found_collision, "Ball should collide with floor");
}

#[test]
fn multiple_bodies_independent() {
    let mut world = PhysicsWorld::new(&default_config());
    let e1 = entity(0);
    let e2 = entity(1);
    let e3 = entity(2);

    world.add_rigid_body(e1, BodyType::Dynamic, [0.0, 10.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    world.add_rigid_body(
        e2,
        BodyType::Dynamic,
        [100.0, 20.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    );
    world.add_rigid_body(e3, BodyType::Static, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);

    assert_eq!(world.body_count(), 3);

    world.remove_entity(e2);
    assert_eq!(world.body_count(), 2);
    assert!(world.has_body(e1));
    assert!(!world.has_body(e2));
    assert!(world.has_body(e3));
}

#[test]
fn sensor_collision_events() {
    let mut world = PhysicsWorld::new(&WorldPhysicsConfig::zero_gravity());

    // Static sensor
    let sensor_entity = entity(0);
    world.add_rigid_body(
        sensor_entity,
        BodyType::Static,
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    );
    let sensor_shape = ColliderShape::Sphere { radius: 5.0 };
    world.add_collider(
        sensor_entity,
        &sensor_shape,
        true, // sensor
        0.0,
        0.0,
        1.0,
        &CollisionLayers::default(),
    );

    // Dynamic body moving into sensor
    let mover = entity(1);
    world.add_rigid_body(
        mover,
        BodyType::Dynamic,
        [10.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    );
    let mover_shape = ColliderShape::Sphere { radius: 0.5 };
    world.add_collider(
        mover,
        &mover_shape,
        false,
        0.5,
        0.0,
        1.0,
        &CollisionLayers::default(),
    );

    world.set_linear_velocity(mover, [-20.0, 0.0, 0.0]);

    let mut found_sensor_event = false;
    for _ in 0..120 {
        world.step();
        let events = world.drain_collision_events();
        if events.iter().any(|e| e.is_sensor() && e.is_started()) {
            found_sensor_event = true;
            break;
        }
    }

    assert!(found_sensor_event, "Should detect sensor intersection");
}
