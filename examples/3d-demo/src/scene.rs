use aether_ecs::{
    AccessDescriptor, Authority, ComponentId, Entity, NetworkIdentity, Stage, SystemBuilder, World,
};
use aether_physics::{
    components::{ColliderComponent, RigidBodyComponent, Transform, Velocity},
    layers::CollisionLayers,
    trigger::TriggerEventQueue,
};

pub const TICK_RATE_HZ: u32 = 60;
pub const GRAVITY: f32 = -9.81;
const PLAYER_NET_ID: u64 = 1;
const DT: f32 = 1.0 / TICK_RATE_HZ as f32;

pub struct SceneEntities {
    pub ground: Entity,
    pub spheres: Vec<Entity>,
    pub cubes: Vec<Entity>,
    pub player: Entity,
    pub trigger_zone: Entity,
}

pub fn setup_scene(world: &mut World) -> SceneEntities {
    register_components(world);
    let ground = spawn_ground(world);
    let spheres = spawn_spheres(world, 5);
    let cubes = spawn_cubes(world, 3);
    let player = spawn_player(world);
    let trigger_zone = spawn_trigger_zone(world);
    add_systems(world);
    SceneEntities { ground, spheres, cubes, player, trigger_zone }
}

fn register_components(world: &mut World) {
    world.register_component::<Transform>();
    world.register_component::<Velocity>();
    world.register_component::<RigidBodyComponent>();
    world.register_component::<ColliderComponent>();
    world.register_component::<NetworkIdentity>();
}

fn spawn_ground(world: &mut World) -> Entity {
    world.spawn_with_3(
        Transform::from_position(0.0, -0.5, 0.0),
        RigidBodyComponent::fixed(),
        ColliderComponent::cuboid(50.0, 0.5, 50.0)
            .with_friction(0.8)
            .with_layers(CollisionLayers::terrain()),
    )
}

fn spawn_spheres(world: &mut World, count: usize) -> Vec<Entity> {
    (0..count)
        .map(|i| {
            let x = (i as f32) * 2.0 - (count as f32);
            world.spawn_with_3(
                Transform::from_position(x, 5.0 + i as f32, 0.0),
                Velocity::default(),
                RigidBodyComponent::dynamic(1.0),
            )
        })
        .collect()
}

fn spawn_cubes(world: &mut World, count: usize) -> Vec<Entity> {
    (0..count)
        .map(|i| {
            let z = (i as f32) * 3.0 - 3.0;
            world.spawn_with_3(
                Transform::from_position(0.0, 8.0 + i as f32 * 2.0, z),
                Velocity::default(),
                RigidBodyComponent::dynamic(2.0),
            )
        })
        .collect()
}

fn spawn_player(world: &mut World) -> Entity {
    let entity = world.spawn_with_3(
        Transform::from_position(0.0, 1.0, -5.0),
        Velocity::default(),
        NetworkIdentity {
            net_id: PLAYER_NET_ID,
            authority: Authority::Server,
        },
    );
    world.add_component(
        entity,
        ColliderComponent::capsule(0.9, 0.3).with_layers(CollisionLayers::player()),
    );
    entity
}

fn spawn_trigger_zone(world: &mut World) -> Entity {
    world.spawn_with_2(
        Transform::from_position(0.0, 1.0, 0.0),
        ColliderComponent::sphere(3.0)
            .sensor()
            .with_layers(CollisionLayers::trigger()),
    )
}

fn add_systems(world: &mut World) {
    world.add_system(
        SystemBuilder::new("input_poll", |_: &World| {}).stage(Stage::Input).build(),
    );
    world.add_system(
        SystemBuilder::new("gravity_apply", |_: &World| {})
            .stage(Stage::PrePhysics)
            .access(
                AccessDescriptor::new()
                    .read(ComponentId::of::<RigidBodyComponent>())
                    .write(ComponentId::of::<Velocity>()),
            )
            .build(),
    );
    world.add_system(
        SystemBuilder::new("physics_integrate", |_: &World| {})
            .stage(Stage::Physics)
            .access(
                AccessDescriptor::new()
                    .read(ComponentId::of::<Velocity>())
                    .write(ComponentId::of::<Transform>()),
            )
            .build(),
    );
    world.add_system(
        SystemBuilder::new("trigger_detect", |_: &World| {}).stage(Stage::PostPhysics).build(),
    );
    world.add_system(
        SystemBuilder::new("lod_select", |_: &World| {})
            .stage(Stage::PreRender)
            .access(AccessDescriptor::new().read(ComponentId::of::<Transform>()))
            .build(),
    );
    world.add_system(
        SystemBuilder::new("render_submit", |_: &World| {}).stage(Stage::Render).build(),
    );
    world.add_system(
        SystemBuilder::new("network_sync", |_: &World| {})
            .stage(Stage::NetworkSync)
            .access(AccessDescriptor::new().read(ComponentId::of::<NetworkIdentity>()))
            .build(),
    );
}

pub fn apply_gravity(world: &mut World, scene: &SceneEntities) {
    for &entity in scene.spheres.iter().chain(scene.cubes.iter()) {
        if let Some(vel) = world.get_component_mut::<Velocity>(entity) {
            vel.linear[1] += GRAVITY * DT;
        }
    }
}

pub fn integrate_physics(world: &mut World, scene: &SceneEntities) {
    let entities: Vec<Entity> = scene.spheres.iter()
        .chain(scene.cubes.iter())
        .chain(std::iter::once(&scene.player))
        .copied()
        .collect();

    for entity in entities {
        let vel = match world.get_component::<Velocity>(entity) {
            Some(v) => *v,
            None => continue,
        };
        if let Some(transform) = world.get_component_mut::<Transform>(entity) {
            transform.position[0] += vel.linear[0] * DT;
            transform.position[1] += vel.linear[1] * DT;
            transform.position[2] += vel.linear[2] * DT;

            if transform.position[1] < 0.0 {
                transform.position[1] = 0.0;
                if let Some(vel) = world.get_component_mut::<Velocity>(entity) {
                    if vel.linear[1] < 0.0 {
                        vel.linear[1] = 0.0;
                    }
                }
            }
        }
    }
}

pub fn detect_triggers(world: &World, queue: &mut TriggerEventQueue, scene: &SceneEntities) {
    let trigger_pos = match world.get_component::<Transform>(scene.trigger_zone) {
        Some(t) => t.position,
        None => return,
    };
    let trigger_radius = 3.0f32;

    for &entity in scene.spheres.iter().chain(scene.cubes.iter()).chain(std::iter::once(&scene.player)) {
        if let Some(transform) = world.get_component::<Transform>(entity) {
            let dx = transform.position[0] - trigger_pos[0];
            let dy = transform.position[1] - trigger_pos[1];
            let dz = transform.position[2] - trigger_pos[2];
            let dist_sq = dx * dx + dy * dy + dz * dz;
            if dist_sq < trigger_radius * trigger_radius {
                queue.report_intersection(scene.trigger_zone, entity);
            }
        }
    }
}

pub fn move_player(world: &mut World, scene: &SceneEntities, dx: f32, dz: f32) {
    if let Some(transform) = world.get_component_mut::<Transform>(scene.player) {
        transform.position[0] += dx;
        transform.position[2] += dz;
    }
}

pub fn reset_physics(world: &mut World, scene: &SceneEntities) {
    for (i, &e) in scene.spheres.iter().enumerate() {
        if let Some(t) = world.get_component_mut::<Transform>(e) {
            let x = (i as f32) * 2.0 - (scene.spheres.len() as f32);
            t.position = [x, 5.0 + i as f32, 0.0];
        }
        if let Some(v) = world.get_component_mut::<Velocity>(e) {
            v.linear = [0.0; 3];
        }
    }
    for (i, &e) in scene.cubes.iter().enumerate() {
        if let Some(t) = world.get_component_mut::<Transform>(e) {
            let z = (i as f32) * 3.0 - 3.0;
            t.position = [0.0, 8.0 + i as f32 * 2.0, z];
        }
        if let Some(v) = world.get_component_mut::<Velocity>(e) {
            v.linear = [0.0; 3];
        }
    }
}

pub fn get_position(world: &World, entity: Entity) -> [f32; 3] {
    world.get_component::<Transform>(entity)
        .map(|t| t.position)
        .unwrap_or([0.0; 3])
}
