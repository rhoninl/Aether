//! Host function bindings exposed to WASM scripts.
//!
//! These functions are registered in the Wasmtime linker under the `"env"`
//! namespace and allow scripts to interact with the engine via a controlled
//! interface.

use wasmtime::{Caller, Linker};

/// Per-instance state accessible from host functions.
///
/// Each WASM instance gets its own `ScriptState`. Host functions receive
/// a `Caller<ScriptState>` and can read/write this state.
#[derive(Debug)]
pub struct ScriptState {
    /// Messages logged by the script via `host_log`.
    pub log_messages: Vec<String>,
    /// Entities spawned by the script (template_name -> entity_id).
    pub spawned_entities: Vec<(String, u64)>,
    /// Next entity ID to assign on spawn.
    pub next_entity_id: u64,
    /// Entities that were despawned.
    pub despawned_entities: Vec<u64>,
    /// Entity positions set by the script: (entity_id, x, y, z).
    pub positions_set: Vec<(u64, f32, f32, f32)>,
    /// Current frame delta time.
    pub delta_time: f32,
    /// Wasmtime store limits for memory enforcement.
    pub store_limits: wasmtime::StoreLimits,
}

impl ScriptState {
    /// Creates a new script state with default values.
    pub fn new(store_limits: wasmtime::StoreLimits) -> Self {
        Self {
            log_messages: Vec::new(),
            spawned_entities: Vec::new(),
            next_entity_id: 1000,
            despawned_entities: Vec::new(),
            positions_set: Vec::new(),
            delta_time: 0.016,
            store_limits,
        }
    }
}

/// Reads a string from WASM linear memory at the given pointer and length.
///
/// Requires mutable access to `Caller` because `get_export` takes `&mut`.
fn read_wasm_string(
    caller: &mut Caller<'_, ScriptState>,
    ptr: i32,
    len: i32,
) -> Option<String> {
    let memory = caller.get_export("memory")?.into_memory()?;
    let data = memory.data(&*caller);
    let start = ptr as usize;
    let end = start + len as usize;
    if end > data.len() {
        return None;
    }
    String::from_utf8(data[start..end].to_vec()).ok()
}

/// Registers all host functions on the given linker.
///
/// The functions are placed in the `"env"` namespace:
/// - `host_log(ptr: i32, len: i32)` - log a UTF-8 message
/// - `host_entity_spawn(ptr: i32, len: i32) -> i64` - spawn entity by template name
/// - `host_entity_despawn(entity_id: i64)` - remove an entity
/// - `host_entity_set_position(entity_id: i64, x: f32, y: f32, z: f32)` - set position
/// - `host_get_time_delta() -> f32` - get frame delta time
pub fn register_host_functions(linker: &mut Linker<ScriptState>) -> wasmtime::Result<()> {
    linker.func_wrap(
        "env",
        "host_log",
        |mut caller: Caller<'_, ScriptState>, ptr: i32, len: i32| {
            if let Some(msg) = read_wasm_string(&mut caller, ptr, len) {
                caller.data_mut().log_messages.push(msg);
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "host_entity_spawn",
        |mut caller: Caller<'_, ScriptState>, ptr: i32, len: i32| -> i64 {
            let template = match read_wasm_string(&mut caller, ptr, len) {
                Some(t) => t,
                None => return -1,
            };
            let state = caller.data_mut();
            let entity_id = state.next_entity_id;
            state.next_entity_id += 1;
            state.spawned_entities.push((template, entity_id));
            entity_id as i64
        },
    )?;

    linker.func_wrap(
        "env",
        "host_entity_despawn",
        |mut caller: Caller<'_, ScriptState>, entity_id: i64| {
            caller.data_mut().despawned_entities.push(entity_id as u64);
        },
    )?;

    linker.func_wrap(
        "env",
        "host_entity_set_position",
        |mut caller: Caller<'_, ScriptState>, entity_id: i64, x: f32, y: f32, z: f32| {
            caller
                .data_mut()
                .positions_set
                .push((entity_id as u64, x, y, z));
        },
    )?;

    linker.func_wrap(
        "env",
        "host_get_time_delta",
        |caller: Caller<'_, ScriptState>| -> f32 {
            caller.data().delta_time
        },
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::sandbox::SandboxConfig;

    fn create_engine() -> wasmtime::Engine {
        let mut config = wasmtime::Config::new();
        config.consume_fuel(true);
        wasmtime::Engine::new(&config).expect("engine creation")
    }

    /// A simple WAT module that calls host_log with a string.
    fn log_module_wat() -> &'static str {
        r#"
        (module
            (import "env" "host_log" (func $host_log (param i32 i32)))
            (memory (export "memory") 1)
            (data (i32.const 0) "hello from wasm")
            (func (export "run")
                (call $host_log (i32.const 0) (i32.const 15))
            )
        )
        "#
    }

    /// A WAT module that spawns an entity.
    fn spawn_module_wat() -> &'static str {
        r#"
        (module
            (import "env" "host_entity_spawn" (func $spawn (param i32 i32) (result i64)))
            (memory (export "memory") 1)
            (data (i32.const 0) "player")
            (func (export "run") (result i64)
                (call $spawn (i32.const 0) (i32.const 6))
            )
        )
        "#
    }

    /// A WAT module that calls host_get_time_delta.
    fn time_delta_module_wat() -> &'static str {
        r#"
        (module
            (import "env" "host_get_time_delta" (func $get_dt (result f32)))
            (memory (export "memory") 1)
            (func (export "get_dt") (result f32)
                (call $get_dt)
            )
        )
        "#
    }

    #[test]
    fn host_log_records_message() {
        let engine = create_engine();
        let mut linker = Linker::new(&engine);
        register_host_functions(&mut linker).unwrap();

        let wasm = wat::parse_str(log_module_wat()).unwrap();
        let module = wasmtime::Module::new(&engine, &wasm).unwrap();

        let sandbox = SandboxConfig::default();
        let limits = sandbox.to_store_limits().build();
        let state = ScriptState::new(limits);
        let mut store = wasmtime::Store::new(&engine, state);
        store.set_fuel(1_000_000).unwrap();
        store.limiter(|s| &mut s.store_limits);

        let instance = linker.instantiate(&mut store, &module).unwrap();
        let run = instance
            .get_typed_func::<(), ()>(&mut store, "run")
            .unwrap();
        run.call(&mut store, ()).unwrap();

        assert_eq!(store.data().log_messages.len(), 1);
        assert_eq!(store.data().log_messages[0], "hello from wasm");
    }

    #[test]
    fn host_entity_spawn_returns_id() {
        let engine = create_engine();
        let mut linker = Linker::new(&engine);
        register_host_functions(&mut linker).unwrap();

        let wasm = wat::parse_str(spawn_module_wat()).unwrap();
        let module = wasmtime::Module::new(&engine, &wasm).unwrap();

        let sandbox = SandboxConfig::default();
        let limits = sandbox.to_store_limits().build();
        let state = ScriptState::new(limits);
        let mut store = wasmtime::Store::new(&engine, state);
        store.set_fuel(1_000_000).unwrap();
        store.limiter(|s| &mut s.store_limits);

        let instance = linker.instantiate(&mut store, &module).unwrap();
        let run = instance
            .get_typed_func::<(), i64>(&mut store, "run")
            .unwrap();
        let entity_id = run.call(&mut store, ()).unwrap();

        assert_eq!(entity_id, 1000);
        assert_eq!(store.data().spawned_entities.len(), 1);
        assert_eq!(store.data().spawned_entities[0].0, "player");
        assert_eq!(store.data().spawned_entities[0].1, 1000);
    }

    #[test]
    fn host_entity_despawn_records_id() {
        let engine = create_engine();
        let mut linker = Linker::new(&engine);
        register_host_functions(&mut linker).unwrap();

        let wat = r#"
        (module
            (import "env" "host_entity_despawn" (func $despawn (param i64)))
            (memory (export "memory") 1)
            (func (export "run")
                (call $despawn (i64.const 42))
            )
        )
        "#;
        let wasm = wat::parse_str(wat).unwrap();
        let module = wasmtime::Module::new(&engine, &wasm).unwrap();

        let sandbox = SandboxConfig::default();
        let limits = sandbox.to_store_limits().build();
        let state = ScriptState::new(limits);
        let mut store = wasmtime::Store::new(&engine, state);
        store.set_fuel(1_000_000).unwrap();
        store.limiter(|s| &mut s.store_limits);

        let instance = linker.instantiate(&mut store, &module).unwrap();
        let run = instance
            .get_typed_func::<(), ()>(&mut store, "run")
            .unwrap();
        run.call(&mut store, ()).unwrap();

        assert_eq!(store.data().despawned_entities, vec![42u64]);
    }

    #[test]
    fn host_entity_set_position_records() {
        let engine = create_engine();
        let mut linker = Linker::new(&engine);
        register_host_functions(&mut linker).unwrap();

        let wat = r#"
        (module
            (import "env" "host_entity_set_position" (func $set_pos (param i64 f32 f32 f32)))
            (memory (export "memory") 1)
            (func (export "run")
                (call $set_pos (i64.const 7) (f32.const 1.0) (f32.const 2.5) (f32.const 3.0))
            )
        )
        "#;
        let wasm = wat::parse_str(wat).unwrap();
        let module = wasmtime::Module::new(&engine, &wasm).unwrap();

        let sandbox = SandboxConfig::default();
        let limits = sandbox.to_store_limits().build();
        let state = ScriptState::new(limits);
        let mut store = wasmtime::Store::new(&engine, state);
        store.set_fuel(1_000_000).unwrap();
        store.limiter(|s| &mut s.store_limits);

        let instance = linker.instantiate(&mut store, &module).unwrap();
        let run = instance
            .get_typed_func::<(), ()>(&mut store, "run")
            .unwrap();
        run.call(&mut store, ()).unwrap();

        assert_eq!(store.data().positions_set.len(), 1);
        let (eid, x, y, z) = store.data().positions_set[0];
        assert_eq!(eid, 7);
        assert!((x - 1.0).abs() < f32::EPSILON);
        assert!((y - 2.5).abs() < f32::EPSILON);
        assert!((z - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn host_get_time_delta_returns_value() {
        let engine = create_engine();
        let mut linker = Linker::new(&engine);
        register_host_functions(&mut linker).unwrap();

        let wasm = wat::parse_str(time_delta_module_wat()).unwrap();
        let module = wasmtime::Module::new(&engine, &wasm).unwrap();

        let sandbox = SandboxConfig::default();
        let limits = sandbox.to_store_limits().build();
        let mut state = ScriptState::new(limits);
        state.delta_time = 0.033;
        let mut store = wasmtime::Store::new(&engine, state);
        store.set_fuel(1_000_000).unwrap();
        store.limiter(|s| &mut s.store_limits);

        let instance = linker.instantiate(&mut store, &module).unwrap();
        let get_dt = instance
            .get_typed_func::<(), f32>(&mut store, "get_dt")
            .unwrap();
        let dt = get_dt.call(&mut store, ()).unwrap();

        assert!((dt - 0.033).abs() < 0.001);
    }
}
