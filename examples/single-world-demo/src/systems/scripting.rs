//! Scripting system: executes visual scripts with a concrete EngineApi
//! that can modify entity transforms in the ECS.

use std::collections::HashMap;

use log::info;

use aether_creator_studio::visual_script::compiler::CompiledScript;
use aether_creator_studio::visual_script::runtime::engine_api::EngineApi;
use aether_creator_studio::visual_script::runtime::error::RuntimeError;
use aether_creator_studio::visual_script::runtime::vm::ScriptVm;
use aether_creator_studio::visual_script::types::Value;

use crate::components::Transform;

/// Next entity ID counter start value.
const INITIAL_ENTITY_ID: u64 = 1;

/// Simple in-memory store mapping entity IDs to transforms.
///
/// Simulates an ECS bridge so that scripts can read and write entity
/// positions and rotations without knowing the real ECS implementation.
#[derive(Debug, Default)]
pub struct EntityStore {
    transforms: HashMap<u64, Transform>,
    next_entity_id: u64,
}

impl EntityStore {
    /// Create a new empty entity store.
    pub fn new() -> Self {
        Self {
            transforms: HashMap::new(),
            next_entity_id: INITIAL_ENTITY_ID,
        }
    }

    /// Set the position of an entity, creating a default entry if needed.
    pub fn set_position(&mut self, entity_id: u64, pos: [f32; 3]) {
        let transform = self.transforms.entry(entity_id).or_default();
        transform.position = pos;
    }

    /// Get the position of an entity, if it exists.
    pub fn get_position(&self, entity_id: u64) -> Option<[f32; 3]> {
        self.transforms.get(&entity_id).map(|t| t.position)
    }

    /// Set the rotation of an entity, creating a default entry if needed.
    pub fn set_rotation(&mut self, entity_id: u64, rot: [f32; 4]) {
        let transform = self.transforms.entry(entity_id).or_default();
        transform.rotation = rot;
    }

    /// Get the rotation of an entity, if it exists.
    pub fn get_rotation(&self, entity_id: u64) -> Option<[f32; 4]> {
        self.transforms.get(&entity_id).map(|t| t.rotation)
    }

    /// List all tracked entity IDs.
    pub fn entities(&self) -> Vec<u64> {
        self.transforms.keys().copied().collect()
    }

    /// Allocate a new entity with a default transform and return its ID.
    fn spawn(&mut self) -> u64 {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        self.transforms.insert(id, Transform::default());
        id
    }
}

/// Concrete `EngineApi` implementation for the demo.
///
/// Dispatches script function calls to an `EntityStore`, allowing scripts
/// to manipulate entity transforms.
pub struct DemoEngineApi<'a> {
    pub entity_store: &'a mut EntityStore,
}

impl<'a> DemoEngineApi<'a> {
    /// Create a new `DemoEngineApi` wrapping the given entity store.
    pub fn new(entity_store: &'a mut EntityStore) -> Self {
        Self { entity_store }
    }
}

impl<'a> EngineApi for DemoEngineApi<'a> {
    fn call(&mut self, function: &str, args: &[Value]) -> Result<Value, RuntimeError> {
        match function {
            "set_position" => {
                if args.len() >= 2 {
                    if let (Value::Entity(id), Value::Vec3 { x, y, z }) = (&args[0], &args[1]) {
                        self.entity_store.set_position(*id, [*x, *y, *z]);
                    }
                }
                Ok(Value::None)
            }

            "get_position" => {
                if let Some(Value::Entity(id)) = args.first() {
                    if let Some(pos) = self.entity_store.get_position(*id) {
                        return Ok(Value::Vec3 {
                            x: pos[0],
                            y: pos[1],
                            z: pos[2],
                        });
                    }
                }
                Ok(Value::None)
            }

            "set_rotation" => {
                if args.len() >= 2 {
                    if let (Value::Entity(id), Value::Vec3 { x, y, z }) = (&args[0], &args[1]) {
                        // Vec3 provides xyz of quaternion; w defaults to 1.0
                        self.entity_store.set_rotation(*id, [*x, *y, *z, 1.0]);
                    }
                }
                Ok(Value::None)
            }

            "get_rotation" => {
                if let Some(Value::Entity(id)) = args.first() {
                    if let Some(rot) = self.entity_store.get_rotation(*id) {
                        // Return xyz of the quaternion as Vec3
                        return Ok(Value::Vec3 {
                            x: rot[0],
                            y: rot[1],
                            z: rot[2],
                        });
                    }
                }
                Ok(Value::None)
            }

            "spawn_entity" => {
                let id = self.entity_store.spawn();
                Ok(Value::Entity(id))
            }

            "log" => {
                if let Some(Value::String(msg)) = args.first() {
                    info!(target: "scripting", "{}", msg);
                }
                Ok(Value::None)
            }

            _ => Err(RuntimeError::UnknownFunction(function.to_string())),
        }
    }
}

/// Manages multiple `ScriptVm` instances, executing them each frame.
pub struct ScriptManager {
    scripts: Vec<ScriptVm>,
}

impl ScriptManager {
    /// Create a new empty script manager.
    pub fn new() -> Self {
        Self {
            scripts: Vec::new(),
        }
    }

    /// Add a script VM to the manager. Returns its script ID (index).
    pub fn add_script(&mut self, vm: ScriptVm) -> usize {
        let id = self.scripts.len();
        self.scripts.push(vm);
        id
    }

    /// Execute all managed scripts against the given engine API.
    pub fn run_all(&mut self, api: &mut dyn EngineApi) {
        for vm in &mut self.scripts {
            // Errors are logged but do not stop other scripts from running.
            if let Err(e) = vm.execute(api) {
                log::warn!(target: "scripting", "Script execution error: {}", e);
            }
        }
    }

    /// Hot-reload a specific script by index.
    pub fn reload(
        &mut self,
        script_id: usize,
        script: &CompiledScript,
    ) -> Result<(), RuntimeError> {
        let vm = self
            .scripts
            .get_mut(script_id)
            .ok_or_else(|| RuntimeError::ApiError(format!("script_id {script_id} out of range")))?;
        vm.reload(script)
    }

    /// Number of managed scripts.
    pub fn count(&self) -> usize {
        self.scripts.len()
    }
}

impl Default for ScriptManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_creator_studio::visual_script::compiler::{CompiledScript, IrInstruction};
    use aether_creator_studio::visual_script::runtime::vm::VmConfig;

    // -- EntityStore tests --

    #[test]
    fn entity_store_starts_empty() {
        let store = EntityStore::new();
        assert!(store.entities().is_empty());
    }

    #[test]
    fn entity_store_default_is_empty() {
        let store = EntityStore::default();
        assert!(store.entities().is_empty());
    }

    #[test]
    fn set_and_get_position() {
        let mut store = EntityStore::new();
        store.set_position(1, [3.0, 4.0, 5.0]);
        assert_eq!(store.get_position(1), Some([3.0, 4.0, 5.0]));
    }

    #[test]
    fn get_position_missing_entity() {
        let store = EntityStore::new();
        assert_eq!(store.get_position(999), None);
    }

    #[test]
    fn set_position_creates_entry() {
        let mut store = EntityStore::new();
        store.set_position(42, [1.0, 2.0, 3.0]);
        assert!(store.entities().contains(&42));
    }

    #[test]
    fn set_and_get_rotation() {
        let mut store = EntityStore::new();
        store.set_rotation(1, [0.1, 0.2, 0.3, 0.9]);
        assert_eq!(store.get_rotation(1), Some([0.1, 0.2, 0.3, 0.9]));
    }

    #[test]
    fn get_rotation_missing_entity() {
        let store = EntityStore::new();
        assert_eq!(store.get_rotation(999), None);
    }

    #[test]
    fn set_rotation_creates_entry() {
        let mut store = EntityStore::new();
        store.set_rotation(7, [0.0, 0.0, 0.0, 1.0]);
        assert!(store.entities().contains(&7));
    }

    #[test]
    fn overwrite_position() {
        let mut store = EntityStore::new();
        store.set_position(1, [1.0, 1.0, 1.0]);
        store.set_position(1, [2.0, 2.0, 2.0]);
        assert_eq!(store.get_position(1), Some([2.0, 2.0, 2.0]));
    }

    #[test]
    fn position_and_rotation_independent() {
        let mut store = EntityStore::new();
        store.set_position(1, [5.0, 6.0, 7.0]);
        store.set_rotation(1, [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(store.get_position(1), Some([5.0, 6.0, 7.0]));
        assert_eq!(store.get_rotation(1), Some([0.1, 0.2, 0.3, 0.4]));
    }

    #[test]
    fn spawn_returns_unique_ids() {
        let mut store = EntityStore::new();
        let id1 = store.spawn();
        let id2 = store.spawn();
        assert_ne!(id1, id2);
        assert!(store.entities().contains(&id1));
        assert!(store.entities().contains(&id2));
    }

    #[test]
    fn entities_lists_all() {
        let mut store = EntityStore::new();
        store.set_position(10, [0.0, 0.0, 0.0]);
        store.set_position(20, [0.0, 0.0, 0.0]);
        store.set_position(30, [0.0, 0.0, 0.0]);
        let ids = store.entities();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&10));
        assert!(ids.contains(&20));
        assert!(ids.contains(&30));
    }

    // -- DemoEngineApi tests --

    fn make_api(store: &mut EntityStore) -> DemoEngineApi<'_> {
        DemoEngineApi::new(store)
    }

    #[test]
    fn api_set_position() {
        let mut store = EntityStore::new();
        {
            let mut api = make_api(&mut store);
            let result = api.call(
                "set_position",
                &[
                    Value::Entity(1),
                    Value::Vec3 {
                        x: 10.0,
                        y: 20.0,
                        z: 30.0,
                    },
                ],
            );
            assert_eq!(result.unwrap(), Value::None);
        }
        assert_eq!(store.get_position(1), Some([10.0, 20.0, 30.0]));
    }

    #[test]
    fn api_get_position() {
        let mut store = EntityStore::new();
        store.set_position(1, [5.0, 6.0, 7.0]);
        let mut api = make_api(&mut store);
        let result = api.call("get_position", &[Value::Entity(1)]).unwrap();
        assert_eq!(
            result,
            Value::Vec3 {
                x: 5.0,
                y: 6.0,
                z: 7.0
            }
        );
    }

    #[test]
    fn api_get_position_missing() {
        let mut store = EntityStore::new();
        let mut api = make_api(&mut store);
        let result = api.call("get_position", &[Value::Entity(999)]).unwrap();
        assert_eq!(result, Value::None);
    }

    #[test]
    fn api_set_rotation() {
        let mut store = EntityStore::new();
        {
            let mut api = make_api(&mut store);
            let result = api.call(
                "set_rotation",
                &[
                    Value::Entity(2),
                    Value::Vec3 {
                        x: 0.1,
                        y: 0.2,
                        z: 0.3,
                    },
                ],
            );
            assert_eq!(result.unwrap(), Value::None);
        }
        // w defaults to 1.0 when setting via Vec3
        assert_eq!(store.get_rotation(2), Some([0.1, 0.2, 0.3, 1.0]));
    }

    #[test]
    fn api_get_rotation() {
        let mut store = EntityStore::new();
        store.set_rotation(3, [0.5, 0.6, 0.7, 0.8]);
        let mut api = make_api(&mut store);
        let result = api.call("get_rotation", &[Value::Entity(3)]).unwrap();
        // Returns xyz of the quaternion as Vec3
        assert_eq!(
            result,
            Value::Vec3 {
                x: 0.5,
                y: 0.6,
                z: 0.7
            }
        );
    }

    #[test]
    fn api_get_rotation_missing() {
        let mut store = EntityStore::new();
        let mut api = make_api(&mut store);
        let result = api.call("get_rotation", &[Value::Entity(999)]).unwrap();
        assert_eq!(result, Value::None);
    }

    #[test]
    fn api_spawn_entity() {
        let mut store = EntityStore::new();
        let mut api = make_api(&mut store);
        let result = api.call("spawn_entity", &[]).unwrap();
        if let Value::Entity(id) = result {
            assert!(id > 0);
            assert!(store.entities().contains(&id));
        } else {
            panic!("Expected Value::Entity, got {result:?}");
        }
    }

    #[test]
    fn api_spawn_multiple_entities() {
        let mut store = EntityStore::new();
        let mut api = make_api(&mut store);
        let id1 = api.call("spawn_entity", &[]).unwrap();
        let id2 = api.call("spawn_entity", &[]).unwrap();
        assert_ne!(id1, id2);
    }

    #[test]
    fn api_log_returns_none() {
        let mut store = EntityStore::new();
        let mut api = make_api(&mut store);
        let result = api
            .call("log", &[Value::String("hello world".to_string())])
            .unwrap();
        assert_eq!(result, Value::None);
    }

    #[test]
    fn api_unknown_function_returns_error() {
        let mut store = EntityStore::new();
        let mut api = make_api(&mut store);
        let result = api.call("nonexistent_function", &[]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RuntimeError::UnknownFunction(_)
        ));
    }

    #[test]
    fn api_set_position_wrong_args_is_noop() {
        let mut store = EntityStore::new();
        let mut api = make_api(&mut store);
        // Wrong arg types: should not panic, just do nothing.
        let result = api.call("set_position", &[Value::Int(1)]).unwrap();
        assert_eq!(result, Value::None);
    }

    #[test]
    fn api_get_position_no_args_returns_none() {
        let mut store = EntityStore::new();
        let mut api = make_api(&mut store);
        let result = api.call("get_position", &[]).unwrap();
        assert_eq!(result, Value::None);
    }

    // -- ScriptManager tests --

    fn make_trivial_script() -> CompiledScript {
        CompiledScript {
            instructions: vec![IrInstruction::Return],
            node_instruction_map: HashMap::new(),
            register_count: 1,
            wasm_bytes: vec![],
        }
    }

    fn make_vm(script: &CompiledScript) -> ScriptVm {
        ScriptVm::new(script, VmConfig::default()).unwrap()
    }

    #[test]
    fn script_manager_starts_empty() {
        let mgr = ScriptManager::new();
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn script_manager_default_is_empty() {
        let mgr = ScriptManager::default();
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn add_script_returns_incrementing_ids() {
        let mut mgr = ScriptManager::new();
        let script = make_trivial_script();
        let id0 = mgr.add_script(make_vm(&script));
        let id1 = mgr.add_script(make_vm(&script));
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        assert_eq!(mgr.count(), 2);
    }

    #[test]
    fn run_all_executes_scripts() {
        let mut mgr = ScriptManager::new();
        let script = make_trivial_script();
        mgr.add_script(make_vm(&script));

        let mut store = EntityStore::new();
        let mut api = DemoEngineApi::new(&mut store);
        // Should not panic.
        mgr.run_all(&mut api);
    }

    #[test]
    fn run_all_with_no_scripts_is_noop() {
        let mut mgr = ScriptManager::new();
        let mut store = EntityStore::new();
        let mut api = DemoEngineApi::new(&mut store);
        mgr.run_all(&mut api);
    }

    #[test]
    fn reload_valid_script() {
        let mut mgr = ScriptManager::new();
        let script = make_trivial_script();
        let id = mgr.add_script(make_vm(&script));

        let new_script = CompiledScript {
            instructions: vec![IrInstruction::Nop, IrInstruction::Return],
            node_instruction_map: HashMap::new(),
            register_count: 2,
            wasm_bytes: vec![],
        };
        assert!(mgr.reload(id, &new_script).is_ok());
    }

    #[test]
    fn reload_invalid_script_id_returns_error() {
        let mut mgr = ScriptManager::new();
        let script = make_trivial_script();
        let result = mgr.reload(999, &script);
        assert!(result.is_err());
    }

    #[test]
    fn script_manager_run_all_with_entity_mutation() {
        // Build a script that calls set_position(Entity(1), Vec3(10, 20, 30))
        let script = CompiledScript {
            instructions: vec![
                IrInstruction::LoadConst(0, Value::Entity(1)),
                IrInstruction::LoadConst(
                    1,
                    Value::Vec3 {
                        x: 10.0,
                        y: 20.0,
                        z: 30.0,
                    },
                ),
                IrInstruction::Call {
                    function: "set_position".to_string(),
                    args: vec![0, 1],
                    result: None,
                },
                IrInstruction::Return,
            ],
            node_instruction_map: HashMap::new(),
            register_count: 2,
            wasm_bytes: vec![],
        };

        let mut mgr = ScriptManager::new();
        mgr.add_script(make_vm(&script));

        let mut store = EntityStore::new();
        let mut api = DemoEngineApi::new(&mut store);
        mgr.run_all(&mut api);

        assert_eq!(store.get_position(1), Some([10.0, 20.0, 30.0]));
    }
}
