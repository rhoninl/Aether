use std::collections::HashMap;

use aether_ecs::Entity;
use crossbeam::channel::{self, Receiver};
use rapier3d::prelude::*;

use crate::components::{BodyType, ColliderShape, Transform, Velocity};
use crate::config::WorldPhysicsConfig;
use crate::events::PhysicsCollisionEvent;
use crate::joints::JointType;
use crate::layers::CollisionLayers;
use crate::query::{QueryFilter, RaycastHit};

/// Core physics simulation world wrapping Rapier3D.
///
/// Manages rigid bodies, colliders, joints, and the simulation pipeline.
/// Provides entity-to-handle mapping so callers work with ECS `Entity` values
/// instead of raw Rapier handles.
pub struct PhysicsWorld {
    pipeline: PhysicsPipeline,
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    island_manager: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    ccd_solver: CCDSolver,
    query_pipeline: QueryPipeline,
    gravity: nalgebra::Vector3<f32>,
    integration_parameters: IntegrationParameters,

    // Entity <-> Rapier handle mappings
    entity_to_body: HashMap<Entity, RigidBodyHandle>,
    body_to_entity: HashMap<RigidBodyHandle, Entity>,
    entity_to_colliders: HashMap<Entity, Vec<ColliderHandle>>,
    collider_to_entity: HashMap<ColliderHandle, Entity>,

    // Event channels
    collision_recv: Receiver<CollisionEvent>,
    contact_force_recv: Receiver<ContactForceEvent>,
    event_collector: ChannelEventCollector,
    pending_collision_events: Vec<PhysicsCollisionEvent>,
}

impl PhysicsWorld {
    /// Create a new physics world from a configuration.
    pub fn new(config: &WorldPhysicsConfig) -> Self {
        let (collision_send, collision_recv) = channel::unbounded();
        let (contact_force_send, contact_force_recv) = channel::unbounded();
        let event_collector = ChannelEventCollector::new(collision_send, contact_force_send);

        let mut integration_parameters = IntegrationParameters::default();
        integration_parameters.dt = config.time_step;
        integration_parameters.num_solver_iterations =
            std::num::NonZero::new(config.solver_iterations as usize)
                .unwrap_or(std::num::NonZero::new(4).unwrap());

        let gravity = nalgebra::vector![config.gravity[0], config.gravity[1], config.gravity[2]];

        Self {
            pipeline: PhysicsPipeline::new(),
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            ccd_solver: CCDSolver::new(),
            query_pipeline: QueryPipeline::new(),
            gravity,
            integration_parameters,
            entity_to_body: HashMap::new(),
            body_to_entity: HashMap::new(),
            entity_to_colliders: HashMap::new(),
            collider_to_entity: HashMap::new(),
            collision_recv,
            contact_force_recv,
            event_collector,
            pending_collision_events: Vec::new(),
        }
    }

    /// Add a rigid body for the given entity.
    ///
    /// Returns the Rapier `RigidBodyHandle` or `None` if the entity already has a body.
    pub fn add_rigid_body(
        &mut self,
        entity: Entity,
        body_type: BodyType,
        position: [f32; 3],
        rotation: [f32; 4],
    ) -> Option<RigidBodyHandle> {
        if self.entity_to_body.contains_key(&entity) {
            return None;
        }

        let builder = match body_type {
            BodyType::Dynamic => RigidBodyBuilder::dynamic(),
            BodyType::Kinematic => RigidBodyBuilder::kinematic_position_based(),
            BodyType::Static => RigidBodyBuilder::fixed(),
        };

        let rb = builder
            .translation(nalgebra::vector![position[0], position[1], position[2]])
            .rotation(quat_to_axis_angle(rotation))
            .build();

        let handle = self.rigid_body_set.insert(rb);
        self.entity_to_body.insert(entity, handle);
        self.body_to_entity.insert(handle, entity);
        Some(handle)
    }

    /// Add a collider to the rigid body associated with the given entity.
    ///
    /// Returns the `ColliderHandle` or `None` if the entity has no rigid body.
    pub fn add_collider(
        &mut self,
        entity: Entity,
        shape: &ColliderShape,
        is_sensor: bool,
        friction: f32,
        restitution: f32,
        density: f32,
        layers: &CollisionLayers,
    ) -> Option<ColliderHandle> {
        let body_handle = *self.entity_to_body.get(&entity)?;

        let shared_shape = shape_to_rapier(shape);
        let groups = layers_to_interaction_groups(layers);

        let collider = ColliderBuilder::new(shared_shape)
            .sensor(is_sensor)
            .friction(friction)
            .restitution(restitution)
            .density(density)
            .collision_groups(groups)
            .active_events(ActiveEvents::COLLISION_EVENTS)
            .build();

        let handle =
            self.collider_set
                .insert_with_parent(collider, body_handle, &mut self.rigid_body_set);

        self.entity_to_colliders
            .entry(entity)
            .or_default()
            .push(handle);
        self.collider_to_entity.insert(handle, entity);

        Some(handle)
    }

    /// Remove the rigid body, all attached colliders, and all joints for the given entity.
    pub fn remove_entity(&mut self, entity: Entity) -> bool {
        let Some(body_handle) = self.entity_to_body.remove(&entity) else {
            return false;
        };
        self.body_to_entity.remove(&body_handle);

        // Remove collider mappings
        if let Some(collider_handles) = self.entity_to_colliders.remove(&entity) {
            for ch in &collider_handles {
                self.collider_to_entity.remove(ch);
            }
        }

        // Rapier's remove_body also removes attached colliders and joints
        self.rigid_body_set.remove(
            body_handle,
            &mut self.island_manager,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            true,
        );

        true
    }

    /// Step the physics simulation by one timestep.
    ///
    /// Collects collision events which can be retrieved via `drain_collision_events`.
    pub fn step(&mut self) {
        self.pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            Some(&mut self.query_pipeline),
            &(),
            &self.event_collector,
        );

        self.collect_events();
    }

    /// Collect raw Rapier events and translate them to entity-based events.
    fn collect_events(&mut self) {
        while let Ok(event) = self.collision_recv.try_recv() {
            let (ch1, ch2, started, sensor) = match event {
                CollisionEvent::Started(h1, h2, flags) => {
                    (h1, h2, true, flags.contains(CollisionEventFlags::SENSOR))
                }
                CollisionEvent::Stopped(h1, h2, flags) => {
                    (h1, h2, false, flags.contains(CollisionEventFlags::SENSOR))
                }
            };

            // Look up entities from collider handles
            let entity1 = self.collider_to_entity.get(&ch1).copied();
            let entity2 = self.collider_to_entity.get(&ch2).copied();

            if let (Some(e1), Some(e2)) = (entity1, entity2) {
                let physics_event = if started {
                    PhysicsCollisionEvent::Started {
                        entity1: e1,
                        entity2: e2,
                        is_sensor: sensor,
                    }
                } else {
                    PhysicsCollisionEvent::Stopped {
                        entity1: e1,
                        entity2: e2,
                        is_sensor: sensor,
                    }
                };
                self.pending_collision_events.push(physics_event);
            }
        }

        // Drain contact force events (we don't use them yet, but keep the channel clear)
        while self.contact_force_recv.try_recv().is_ok() {}
    }

    /// Drain all collision events since the last drain.
    pub fn drain_collision_events(&mut self) -> Vec<PhysicsCollisionEvent> {
        std::mem::take(&mut self.pending_collision_events)
    }

    /// Get a read-only reference to pending collision events.
    pub fn collision_events(&self) -> &[PhysicsCollisionEvent] {
        &self.pending_collision_events
    }

    /// Synchronize kinematic bodies from ECS transforms.
    ///
    /// For each (entity, transform) pair, if the entity has a kinematic body,
    /// its Rapier position is updated to match.
    pub fn sync_from_ecs(&mut self, updates: &[(Entity, Transform)]) {
        for (entity, transform) in updates {
            if let Some(&handle) = self.entity_to_body.get(entity) {
                if let Some(body) = self.rigid_body_set.get_mut(handle) {
                    if body.is_kinematic() {
                        let translation = nalgebra::vector![
                            transform.position[0],
                            transform.position[1],
                            transform.position[2]
                        ];
                        let rotation = nalgebra::UnitQuaternion::from_quaternion(
                            nalgebra::Quaternion::new(
                                transform.rotation[3],
                                transform.rotation[0],
                                transform.rotation[1],
                                transform.rotation[2],
                            ),
                        );
                        body.set_next_kinematic_translation(translation);
                        body.set_next_kinematic_rotation(rotation);
                    }
                }
            }
        }
    }

    /// Read simulation results for all dynamic bodies.
    ///
    /// Returns `(Entity, Transform, Velocity)` for each dynamic body in the world.
    pub fn sync_to_ecs(&self) -> Vec<(Entity, Transform, Velocity)> {
        let mut results = Vec::new();

        for (&entity, &handle) in &self.entity_to_body {
            if let Some(body) = self.rigid_body_set.get(handle) {
                if !body.is_dynamic() {
                    continue;
                }

                let pos = body.translation();
                let rot = body.rotation();
                let linvel = body.linvel();
                let angvel = body.angvel();

                let transform = Transform {
                    position: [pos.x, pos.y, pos.z],
                    rotation: [rot.i, rot.j, rot.k, rot.w],
                };

                let velocity = Velocity {
                    linear: [linvel.x, linvel.y, linvel.z],
                    angular: [angvel.x, angvel.y, angvel.z],
                };

                results.push((entity, transform, velocity));
            }
        }

        results
    }

    /// Cast a ray and return the closest hit.
    pub fn raycast(
        &self,
        origin: [f32; 3],
        direction: [f32; 3],
        max_distance: f32,
        filter: &QueryFilter,
    ) -> Option<RaycastHit> {
        let ray = Ray::new(
            nalgebra::point![origin[0], origin[1], origin[2]],
            nalgebra::vector![direction[0], direction[1], direction[2]],
        );

        let exclude_body = filter
            .exclude_entity
            .and_then(|e| self.entity_to_body.get(&e).copied());

        let rapier_filter = build_rapier_query_filter(exclude_body, filter.include_sensors);

        let result = self.query_pipeline.cast_ray_and_get_normal(
            &self.rigid_body_set,
            &self.collider_set,
            &ray,
            max_distance,
            true,
            rapier_filter,
        );

        result.and_then(|(collider_handle, intersection)| {
            let entity = self.collider_to_entity.get(&collider_handle).copied()?;
            let point = ray.point_at(intersection.time_of_impact);
            Some(RaycastHit {
                entity,
                point: [point.x, point.y, point.z],
                normal: [intersection.normal.x, intersection.normal.y, intersection.normal.z],
                distance: intersection.time_of_impact,
            })
        })
    }

    /// Cast a ray and return all hits (unsorted).
    pub fn raycast_all(
        &self,
        origin: [f32; 3],
        direction: [f32; 3],
        max_distance: f32,
        filter: &QueryFilter,
    ) -> Vec<RaycastHit> {
        let ray = Ray::new(
            nalgebra::point![origin[0], origin[1], origin[2]],
            nalgebra::vector![direction[0], direction[1], direction[2]],
        );

        let exclude_body = filter
            .exclude_entity
            .and_then(|e| self.entity_to_body.get(&e).copied());

        let rapier_filter = build_rapier_query_filter(exclude_body, filter.include_sensors);

        let mut hits = Vec::new();

        self.query_pipeline.intersections_with_ray(
            &self.rigid_body_set,
            &self.collider_set,
            &ray,
            max_distance,
            true,
            rapier_filter,
            |collider_handle, intersection| {
                if let Some(&entity) = self.collider_to_entity.get(&collider_handle) {
                    let point = ray.point_at(intersection.time_of_impact);
                    hits.push(RaycastHit {
                        entity,
                        point: [point.x, point.y, point.z],
                        normal: [
                            intersection.normal.x,
                            intersection.normal.y,
                            intersection.normal.z,
                        ],
                        distance: intersection.time_of_impact,
                    });
                }
                true // continue searching
            },
        );

        hits
    }

    /// Add a joint between two entities.
    ///
    /// Returns the joint handle or `None` if either entity has no rigid body.
    pub fn add_joint(
        &mut self,
        entity1: Entity,
        entity2: Entity,
        joint_type: &JointType,
    ) -> Option<ImpulseJointHandle> {
        let &handle1 = self.entity_to_body.get(&entity1)?;
        let &handle2 = self.entity_to_body.get(&entity2)?;

        let joint = build_rapier_joint(joint_type);
        let joint_handle = self
            .impulse_joint_set
            .insert(handle1, handle2, joint, true);
        Some(joint_handle)
    }

    /// Remove a joint by its handle.
    pub fn remove_joint(&mut self, handle: ImpulseJointHandle) {
        self.impulse_joint_set.remove(handle, true);
    }

    /// Apply a force to a dynamic body (resets each step).
    pub fn apply_force(&mut self, entity: Entity, force: [f32; 3]) -> bool {
        if let Some(&handle) = self.entity_to_body.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.add_force(
                    nalgebra::vector![force[0], force[1], force[2]],
                    true,
                );
                return true;
            }
        }
        false
    }

    /// Apply an impulse to a dynamic body (instantaneous velocity change).
    pub fn apply_impulse(&mut self, entity: Entity, impulse: [f32; 3]) -> bool {
        if let Some(&handle) = self.entity_to_body.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.apply_impulse(
                    nalgebra::vector![impulse[0], impulse[1], impulse[2]],
                    true,
                );
                return true;
            }
        }
        false
    }

    /// Set the linear velocity of a body.
    pub fn set_linear_velocity(&mut self, entity: Entity, velocity: [f32; 3]) -> bool {
        if let Some(&handle) = self.entity_to_body.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_linvel(
                    nalgebra::vector![velocity[0], velocity[1], velocity[2]],
                    true,
                );
                return true;
            }
        }
        false
    }

    /// Set the angular velocity of a body.
    pub fn set_angular_velocity(&mut self, entity: Entity, velocity: [f32; 3]) -> bool {
        if let Some(&handle) = self.entity_to_body.get(&entity) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_angvel(
                    nalgebra::vector![velocity[0], velocity[1], velocity[2]],
                    true,
                );
                return true;
            }
        }
        false
    }

    /// Get the number of rigid bodies in the world.
    pub fn body_count(&self) -> usize {
        self.entity_to_body.len()
    }

    /// Get the number of colliders in the world.
    pub fn collider_count(&self) -> usize {
        self.collider_to_entity.len()
    }

    /// Check if an entity has a rigid body in this world.
    pub fn has_body(&self, entity: Entity) -> bool {
        self.entity_to_body.contains_key(&entity)
    }

    /// Get the current gravity vector.
    pub fn gravity(&self) -> [f32; 3] {
        [self.gravity.x, self.gravity.y, self.gravity.z]
    }

    /// Set the gravity vector.
    pub fn set_gravity(&mut self, gravity: [f32; 3]) {
        self.gravity = nalgebra::vector![gravity[0], gravity[1], gravity[2]];
    }

    /// Get the transform of an entity's body.
    pub fn get_transform(&self, entity: Entity) -> Option<Transform> {
        let &handle = self.entity_to_body.get(&entity)?;
        let body = self.rigid_body_set.get(handle)?;
        let pos = body.translation();
        let rot = body.rotation();
        Some(Transform {
            position: [pos.x, pos.y, pos.z],
            rotation: [rot.i, rot.j, rot.k, rot.w],
        })
    }

    /// Get the velocity of an entity's body.
    pub fn get_velocity(&self, entity: Entity) -> Option<Velocity> {
        let &handle = self.entity_to_body.get(&entity)?;
        let body = self.rigid_body_set.get(handle)?;
        let linvel = body.linvel();
        let angvel = body.angvel();
        Some(Velocity {
            linear: [linvel.x, linvel.y, linvel.z],
            angular: [angvel.x, angvel.y, angvel.z],
        })
    }
}

/// Convert a quaternion [x, y, z, w] to a scaled-axis (axis-angle) rotation vector.
fn quat_to_axis_angle(q: [f32; 4]) -> nalgebra::Vector3<f32> {
    let unit_quat = nalgebra::UnitQuaternion::from_quaternion(nalgebra::Quaternion::new(
        q[3], q[0], q[1], q[2],
    ));
    let axis_angle = unit_quat.scaled_axis();
    axis_angle
}

/// Convert a `ColliderShape` to a Rapier `SharedShape`.
fn shape_to_rapier(shape: &ColliderShape) -> SharedShape {
    match shape {
        ColliderShape::Sphere { radius } => SharedShape::ball(*radius),
        ColliderShape::Box { half_extents } => {
            SharedShape::cuboid(half_extents[0], half_extents[1], half_extents[2])
        }
        ColliderShape::Capsule {
            half_height,
            radius,
        } => SharedShape::capsule_y(*half_height, *radius),
        ColliderShape::Cylinder {
            half_height,
            radius,
        } => SharedShape::cylinder(*half_height, *radius),
    }
}

/// Convert `CollisionLayers` to Rapier `InteractionGroups`.
///
/// Rapier uses a u32 with membership in the high 16 bits and filter in the low 16 bits
/// via `Group::from_bits_truncate`.
fn layers_to_interaction_groups(layers: &CollisionLayers) -> InteractionGroups {
    InteractionGroups::new(
        Group::from_bits_truncate(layers.membership as u32),
        Group::from_bits_truncate(layers.filter as u32),
    )
}

/// Build a Rapier `rapier3d::pipeline::QueryFilter` from our filter options.
fn build_rapier_query_filter(
    exclude_body: Option<RigidBodyHandle>,
    _include_sensors: bool,
) -> rapier3d::pipeline::QueryFilter<'static> {
    let mut filter = rapier3d::pipeline::QueryFilter::default();
    if let Some(body) = exclude_body {
        filter = filter.exclude_rigid_body(body);
    }
    filter
}

/// Build a Rapier joint from our `JointType` description.
fn build_rapier_joint(joint_type: &JointType) -> GenericJoint {
    match joint_type {
        JointType::Fixed { anchor1, anchor2 } => FixedJointBuilder::new()
            .local_anchor1(nalgebra::point![anchor1[0], anchor1[1], anchor1[2]])
            .local_anchor2(nalgebra::point![anchor2[0], anchor2[1], anchor2[2]])
            .build()
            .into(),
        JointType::Revolute {
            axis,
            anchor1,
            anchor2,
        } => {
            let unit_axis = nalgebra::Unit::new_normalize(nalgebra::vector![
                axis[0], axis[1], axis[2]
            ]);
            RevoluteJointBuilder::new(unit_axis)
                .local_anchor1(nalgebra::point![anchor1[0], anchor1[1], anchor1[2]])
                .local_anchor2(nalgebra::point![anchor2[0], anchor2[1], anchor2[2]])
                .build()
                .into()
        }
        JointType::Prismatic {
            axis,
            anchor1,
            anchor2,
            limits,
        } => {
            let unit_axis = nalgebra::Unit::new_normalize(nalgebra::vector![
                axis[0], axis[1], axis[2]
            ]);
            let mut builder = PrismaticJointBuilder::new(unit_axis)
                .local_anchor1(nalgebra::point![anchor1[0], anchor1[1], anchor1[2]])
                .local_anchor2(nalgebra::point![anchor2[0], anchor2[1], anchor2[2]]);
            if let Some([min, max]) = limits {
                builder = builder.limits([*min, *max]);
            }
            builder.build().into()
        }
    }
}

#[cfg(test)]
#[path = "world_tests.rs"]
mod tests;
