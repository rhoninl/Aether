use std::sync::Mutex;
use std::time::{Duration, Instant};

use mlua::prelude::*;

use crate::sandbox;

const INSTRUCTION_CHECK_INTERVAL: u32 = 1000;

/// A sandboxed Lua VM instance wrapping an `mlua::Lua` state.
///
/// Each script gets its own `LuaVm` for isolation. The inner `Lua` is
/// wrapped in a `Mutex` so that `LuaVm` is `Send + Sync`.
pub struct LuaVm {
    lua: Mutex<mlua::Lua>,
    script_id: u64,
    script_name: String,
    memory_limit: usize,
}

impl LuaVm {
    /// Creates a new sandboxed Lua VM.
    ///
    /// * `script_id` - unique identifier for the script
    /// * `name` - human-readable script name
    /// * `memory_limit` - maximum bytes the VM may allocate (0 = unlimited)
    pub fn new(script_id: u64, name: &str, memory_limit: usize) -> Result<Self, mlua::Error> {
        let lua = mlua::Lua::new();
        if memory_limit > 0 {
            lua.set_memory_limit(memory_limit)?;
        }
        sandbox::apply(&lua)?;

        // Create the aether.time table so set_time works before bridge registration
        let aether = lua.create_table()?;
        let time = lua.create_table()?;
        time.set("dt", 0.0_f32)?;
        time.set("tick", 0_u64)?;
        aether.set("time", time)?;
        lua.globals().set("aether", aether)?;

        Ok(Self {
            lua: Mutex::new(lua),
            script_id,
            script_name: name.to_string(),
            memory_limit,
        })
    }

    /// Loads and executes a Lua source string in the VM.
    pub fn load_script(&self, source: &str) -> Result<(), mlua::Error> {
        let lua = self.lua.lock().unwrap();
        lua.load(source).set_name(&self.script_name).exec()?;
        Ok(())
    }

    /// Calls a top-level Lua function by name. Returns `Ok(())` if the
    /// function does not exist (missing hooks are not errors).
    pub fn call_hook(&self, hook_name: &str) -> Result<(), mlua::Error> {
        let lua = self.lua.lock().unwrap();
        let globals = lua.globals();
        let value: mlua::Value = globals.get(hook_name)?;
        if let mlua::Value::Function(func) = value {
            func.call::<()>(())?;
        }
        Ok(())
    }

    /// Updates `aether.time.dt` and `aether.time.tick` in the Lua state.
    pub fn set_time(&self, dt: f32, tick: u64) -> Result<(), mlua::Error> {
        let lua = self.lua.lock().unwrap();
        let aether: LuaTable = lua.globals().get("aether")?;
        let time: LuaTable = aether.get("time")?;
        time.set("dt", dt)?;
        time.set("tick", tick)?;
        Ok(())
    }

    /// Installs an instruction-counting hook that terminates execution
    /// when the wall-clock time exceeds `budget`.
    pub fn set_instruction_hook(&self, budget: Duration, start: Instant) {
        let lua = self.lua.lock().unwrap();
        lua.set_hook(
            mlua::HookTriggers::new().every_nth_instruction(INSTRUCTION_CHECK_INTERVAL),
            move |_lua, _debug| {
                let elapsed = start.elapsed();
                if elapsed > budget {
                    Err(mlua::Error::runtime(format!(
                        "CPU budget exceeded: used {elapsed:?}, budget {budget:?}"
                    )))
                } else {
                    Ok(mlua::VmState::Continue)
                }
            },
        );
    }

    /// Returns the script name.
    pub fn script_name(&self) -> &str {
        &self.script_name
    }

    /// Returns the script id.
    pub fn script_id(&self) -> u64 {
        self.script_id
    }

    /// Returns the configured memory limit in bytes (0 = unlimited).
    pub fn memory_limit(&self) -> usize {
        self.memory_limit
    }

    /// Returns the approximate memory used by the Lua VM in bytes.
    pub fn memory_used(&self) -> usize {
        let lua = self.lua.lock().unwrap();
        lua.used_memory()
    }

    /// Returns a lock guard to the inner `mlua::Lua` state.
    ///
    /// Use this when you need direct access to the Lua state, e.g. to
    /// register API bridges before loading scripts.
    pub fn lua_lock(&self) -> std::sync::MutexGuard<'_, mlua::Lua> {
        self.lua.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LuaVm>();
    }

    #[test]
    fn test_load_and_call_hook() {
        let vm = LuaVm::new(1, "test.lua", 0).unwrap();
        vm.load_script(
            r#"
            _G.init_called = false
            function on_init()
                _G.init_called = true
            end
            "#,
        )
        .unwrap();
        vm.call_hook("on_init").unwrap();

        // Verify the hook ran by checking the global
        let lua = vm.lua.lock().unwrap();
        let called: bool = lua.globals().get("init_called").unwrap();
        assert!(called, "on_init should have been called");
    }

    #[test]
    fn test_call_missing_hook_is_ok() {
        let vm = LuaVm::new(1, "test.lua", 0).unwrap();
        vm.load_script("-- no hooks defined").unwrap();
        let result = vm.call_hook("on_tick");
        assert!(result.is_ok(), "calling missing hook should return Ok");
    }

    #[test]
    fn test_syntax_error_returns_err() {
        let vm = LuaVm::new(1, "bad.lua", 0).unwrap();
        let result = vm.load_script("function oops(");
        assert!(result.is_err(), "syntax error should return Err");
    }

    #[test]
    fn test_runtime_error_propagates() {
        let vm = LuaVm::new(1, "err.lua", 0).unwrap();
        vm.load_script(
            r#"
            function on_tick()
                error("something went wrong")
            end
            "#,
        )
        .unwrap();
        let result = vm.call_hook("on_tick");
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("something went wrong"),
            "error message should propagate: {err_msg}"
        );
    }

    #[test]
    fn test_set_time_updates_aether_time() {
        let vm = LuaVm::new(1, "time.lua", 0).unwrap();
        vm.set_time(0.016, 42).unwrap();
        vm.load_script(
            r#"
            _G.captured_dt = aether.time.dt
            _G.captured_tick = aether.time.tick
            "#,
        )
        .unwrap();

        let lua = vm.lua.lock().unwrap();
        let dt: f32 = lua.globals().get("captured_dt").unwrap();
        let tick: u64 = lua.globals().get("captured_tick").unwrap();
        assert!((dt - 0.016).abs() < 0.001, "dt should be ~0.016, got {dt}");
        assert_eq!(tick, 42);
    }

    #[test]
    fn test_memory_limit_enforced() {
        // 32KB is very small - allocating a large table should fail
        let vm = LuaVm::new(1, "mem.lua", 32 * 1024).unwrap();
        let result = vm.load_script(
            r#"
            local t = {}
            for i = 1, 100000 do
                t[i] = string.rep("x", 1000)
            end
            "#,
        );
        assert!(
            result.is_err(),
            "allocating beyond memory limit should error"
        );
    }

    #[test]
    fn test_instruction_hook_terminates_infinite_loop() {
        let vm = LuaVm::new(1, "loop.lua", 0).unwrap();
        vm.load_script(
            r#"
            function on_tick()
                while true do end
            end
            "#,
        )
        .unwrap();

        let start = Instant::now();
        let budget = Duration::from_millis(1);
        vm.set_instruction_hook(budget, start);

        let result = vm.call_hook("on_tick");
        assert!(result.is_err(), "infinite loop should be terminated");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("CPU budget exceeded"),
            "error should mention CPU budget: {err_msg}"
        );
        // Should not have taken too long
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "should terminate quickly"
        );
    }

    #[test]
    fn test_separate_vms_isolated() {
        let vm1 = LuaVm::new(1, "a.lua", 0).unwrap();
        let vm2 = LuaVm::new(2, "b.lua", 0).unwrap();

        vm1.load_script("_G.shared_value = 42").unwrap();

        // vm2 should not see vm1's global
        vm2.load_script("_G.check = _G.shared_value").unwrap();
        let lua2 = vm2.lua.lock().unwrap();
        let check: mlua::Value = lua2.globals().get("check").unwrap();
        assert!(
            matches!(check, mlua::Value::Nil),
            "vm2 should not see vm1's globals"
        );
    }

    #[test]
    fn test_script_name_set_correctly() {
        let vm = LuaVm::new(7, "my_script.lua", 0).unwrap();
        assert_eq!(vm.script_name(), "my_script.lua");
        assert_eq!(vm.script_id(), 7);
    }
}
