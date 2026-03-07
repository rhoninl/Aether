use crate::query::AccessDescriptor;
use crate::stage::Stage;
use crate::world::World;

/// A system is a function that operates on the ECS world.
pub trait System: Send + Sync {
    fn name(&self) -> &str;
    fn stage(&self) -> Stage;
    fn access(&self) -> AccessDescriptor;
    fn run(&self, world: &World);
}

/// A descriptor used by the scheduler to track systems and their metadata.
pub struct SystemDescriptor {
    pub name: String,
    pub stage: Stage,
    pub access: AccessDescriptor,
    pub system: Box<dyn System>,
}

/// Wraps a closure as a System implementation.
pub struct FnSystem<F>
where
    F: Fn(&World) + Send + Sync,
{
    pub(crate) name: String,
    pub(crate) stage: Stage,
    pub(crate) access: AccessDescriptor,
    pub(crate) func: F,
}

impl<F> System for FnSystem<F>
where
    F: Fn(&World) + Send + Sync,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn stage(&self) -> Stage {
        self.stage
    }

    fn access(&self) -> AccessDescriptor {
        self.access.clone()
    }

    fn run(&self, world: &World) {
        (self.func)(world);
    }
}

/// Builder for creating systems from closures.
pub struct SystemBuilder<F>
where
    F: Fn(&World) + Send + Sync,
{
    name: String,
    stage: Stage,
    access: AccessDescriptor,
    func: F,
}

impl<F> SystemBuilder<F>
where
    F: Fn(&World) + Send + Sync + 'static,
{
    pub fn new(name: impl Into<String>, func: F) -> Self {
        Self {
            name: name.into(),
            stage: Stage::PrePhysics,
            access: AccessDescriptor::new(),
            func,
        }
    }

    pub fn stage(mut self, stage: Stage) -> Self {
        self.stage = stage;
        self
    }

    pub fn access(mut self, access: AccessDescriptor) -> Self {
        self.access = access;
        self
    }

    pub fn build(self) -> Box<dyn System> {
        Box::new(FnSystem {
            name: self.name,
            stage: self.stage,
            access: self.access,
            func: self.func,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{Component, ComponentId};

    struct Position {
        x: f32,
        y: f32,
        z: f32,
    }
    impl Component for Position {}

    struct Velocity {
        x: f32,
        y: f32,
        z: f32,
    }
    impl Component for Velocity {}

    #[test]
    fn fn_system_basic_properties() {
        let access = AccessDescriptor::new()
            .read(ComponentId::of::<Position>())
            .write(ComponentId::of::<Velocity>());

        let system = SystemBuilder::new("test_system", |_world: &World| {})
            .stage(Stage::Physics)
            .access(access)
            .build();

        assert_eq!(system.name(), "test_system");
        assert_eq!(system.stage(), Stage::Physics);
        assert_eq!(system.access().reads.len(), 1);
        assert_eq!(system.access().writes.len(), 1);
    }

    #[test]
    fn system_builder_defaults() {
        let system = SystemBuilder::new("default_system", |_: &World| {}).build();

        assert_eq!(system.name(), "default_system");
        assert_eq!(system.stage(), Stage::PrePhysics);
        assert!(system.access().reads.is_empty());
        assert!(system.access().writes.is_empty());
    }

    #[test]
    fn system_builder_chaining() {
        let access = AccessDescriptor::new().read(ComponentId::of::<Position>());

        let system = SystemBuilder::new("chained", |_: &World| {})
            .stage(Stage::Render)
            .access(access.clone())
            .build();

        assert_eq!(system.stage(), Stage::Render);
        assert_eq!(system.access().reads, access.reads);
    }
}
