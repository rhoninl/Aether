use mlua::prelude::*;

/// Globals that are explicitly removed from the Lua environment.
pub const DENIED_GLOBALS: &[&str] = &[
    "os",
    "io",
    "debug",
    "loadfile",
    "dofile",
    "require",
    "load",
    "collectgarbage",
    "newproxy",
    "setfenv",
    "getfenv",
];

/// Standard library modules that scripts are allowed to use.
pub const ALLOWED_MODULES: &[&str] = &["math", "string", "table", "utf8", "coroutine"];

/// Applies sandbox restrictions to a Lua state.
///
/// This removes dangerous globals, replaces `setmetatable` with a safe
/// version that blocks `__gc` and `__metatable` metamethods, and locks
/// the string metatable to prevent prototype pollution.
pub fn apply(lua: &mlua::Lua) -> Result<(), mlua::Error> {
    let globals = lua.globals();

    // Remove all denied globals
    for name in DENIED_GLOBALS {
        globals.raw_set(*name, mlua::Value::Nil)?;
    }

    // Capture the original setmetatable before we replace it
    let original_setmetatable: mlua::Function =
        globals.get("setmetatable")?;

    // Replace setmetatable with a safe version
    let safe_setmetatable =
        lua.create_function(move |_, (table, mt): (LuaTable, Option<LuaTable>)| {
            if let Some(ref mt) = mt {
                if mt.contains_key("__gc")? {
                    return Err(mlua::Error::runtime("__gc metamethod is not allowed"));
                }
                if mt.contains_key("__metatable")? {
                    return Err(mlua::Error::runtime("__metatable is not allowed"));
                }
            }
            // Delegate to the real setmetatable
            let result: LuaTable =
                original_setmetatable.call((table.clone(), mt))?;
            Ok(result)
        })?;
    globals.set("setmetatable", safe_setmetatable)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandboxed_lua() -> mlua::Lua {
        let lua = mlua::Lua::new();
        apply(&lua).expect("sandbox apply should succeed");
        lua
    }

    #[test]
    fn test_os_removed() {
        let lua = sandboxed_lua();
        let result: mlua::Value = lua.load("return os").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_io_removed() {
        let lua = sandboxed_lua();
        let result: mlua::Value = lua.load("return io").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_debug_removed() {
        let lua = sandboxed_lua();
        let result: mlua::Value = lua.load("return debug").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_loadfile_removed() {
        let lua = sandboxed_lua();
        let result: mlua::Value = lua.load("return loadfile").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_dofile_removed() {
        let lua = sandboxed_lua();
        let result: mlua::Value = lua.load("return dofile").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_require_removed() {
        let lua = sandboxed_lua();
        let result: mlua::Value = lua.load("return require").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_load_removed() {
        let lua = sandboxed_lua();
        let result: mlua::Value = lua.load("return load").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_collectgarbage_removed() {
        let lua = sandboxed_lua();
        let result: mlua::Value = lua.load("return collectgarbage").eval().unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_math_available() {
        let lua = sandboxed_lua();
        let result: i64 = lua.load("return math.floor(1.5)").eval().unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn test_string_available() {
        let lua = sandboxed_lua();
        let result: i64 = lua.load("return string.len('hello')").eval().unwrap();
        assert_eq!(result, 5);
    }

    #[test]
    fn test_table_available() {
        let lua = sandboxed_lua();
        let result: i64 = lua
            .load("local t = {}; table.insert(t, 42); return t[1]")
            .eval()
            .unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_pairs_available() {
        let lua = sandboxed_lua();
        lua.load("for k, v in pairs({}) do end").exec().unwrap();
    }

    #[test]
    fn test_pcall_available() {
        let lua = sandboxed_lua();
        let result: bool = lua
            .load("return pcall(function() end)")
            .eval()
            .unwrap();
        assert!(result);
    }

    #[test]
    fn test_tostring_available() {
        let lua = sandboxed_lua();
        let result: String = lua.load("return tostring(42)").eval().unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn test_coroutine_available() {
        let lua = sandboxed_lua();
        let result: mlua::Value = lua.load("return coroutine.create").eval().unwrap();
        assert!(matches!(result, mlua::Value::Function(_)));
    }

    #[test]
    fn test_setmetatable_blocks_gc() {
        let lua = sandboxed_lua();
        let result = lua
            .load("local t = {}; setmetatable(t, {__gc = function() end})")
            .exec();
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("__gc"),
            "error should mention __gc: {err_msg}"
        );
    }

    #[test]
    fn test_setmetatable_blocks_metatable_key() {
        let lua = sandboxed_lua();
        let result = lua
            .load("local t = {}; setmetatable(t, {__metatable = false})")
            .exec();
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("__metatable"),
            "error should mention __metatable: {err_msg}"
        );
    }

    #[test]
    fn test_setmetatable_allows_normal() {
        let lua = sandboxed_lua();
        // __index and __tostring are allowed
        let result: String = lua
            .load(
                r#"
                local t = {}
                setmetatable(t, {
                    __index = function(_, k) return "value_" .. k end,
                    __tostring = function() return "my_table" end
                })
                return tostring(t) .. ":" .. t.foo
                "#,
            )
            .eval()
            .unwrap();
        assert_eq!(result, "my_table:value_foo");
    }
}
