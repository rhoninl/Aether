use std::sync::{Arc, Mutex};

use aether_scripting::{
    AudioApi, AudioHandle, EntityApi, NetworkApi, PhysicsApi, ScriptApiError, StorageApi, Vec3,
};

/// Holds Arc-wrapped API implementations and context needed to register
/// the `aether.*` Lua namespace.
pub struct ScriptContext {
    pub entity_api: Arc<Mutex<dyn EntityApi + Send>>,
    pub physics_api: Arc<Mutex<dyn PhysicsApi + Send>>,
    pub audio_api: Arc<Mutex<dyn AudioApi + Send>>,
    pub network_api: Arc<Mutex<dyn NetworkApi + Send>>,
    pub storage_api: Arc<Mutex<dyn StorageApi + Send>>,
    pub script_id: u64,
}

fn api_err_to_lua(e: ScriptApiError) -> mlua::Error {
    mlua::Error::runtime(format!("{e:?}"))
}

/// Registers all engine API sub-tables under the `aether` global.
///
/// After calling this, Lua scripts can use `aether.entity.spawn(...)`,
/// `aether.physics.apply_force(...)`, etc.
pub fn register_all(lua: &mlua::Lua, ctx: &ScriptContext) -> Result<(), mlua::Error> {
    let aether = lua.create_table()?;

    // aether.time
    register_time_table(lua, &aether)?;

    // aether.entity
    register_entity_table(lua, &aether, ctx.entity_api.clone())?;

    // aether.physics
    register_physics_table(lua, &aether, ctx.physics_api.clone())?;

    // aether.audio
    register_audio_table(lua, &aether, ctx.audio_api.clone())?;

    // aether.network
    register_network_table(lua, &aether, ctx.network_api.clone())?;

    // aether.storage
    register_storage_table(lua, &aether, ctx.storage_api.clone())?;

    lua.globals().set("aether", aether)?;
    Ok(())
}

/// Creates the `aether.time` sub-table with initial `dt` and `tick` values.
pub fn register_time_table(lua: &mlua::Lua, aether: &mlua::Table) -> Result<(), mlua::Error> {
    let time = lua.create_table()?;
    time.set("dt", 0.0_f64)?;
    time.set("tick", 0_u64)?;
    aether.set("time", time)?;
    Ok(())
}

fn register_entity_table(
    lua: &mlua::Lua,
    aether: &mlua::Table,
    api: Arc<Mutex<dyn EntityApi + Send>>,
) -> Result<(), mlua::Error> {
    let entity = lua.create_table()?;

    // spawn(template: string) -> number
    let api_clone = api.clone();
    let spawn = lua.create_function(move |_, template: String| {
        let mut api = api_clone.lock().unwrap();
        api.spawn_entity(&template).map_err(api_err_to_lua)
    })?;
    entity.set("spawn", spawn)?;

    // despawn(id: number)
    let api_clone = api.clone();
    let despawn = lua.create_function(move |_, id: u64| {
        let mut api = api_clone.lock().unwrap();
        api.despawn_entity(id).map_err(api_err_to_lua)
    })?;
    entity.set("despawn", despawn)?;

    // set_position(id: number, x: number, y: number, z: number)
    let api_clone = api.clone();
    let set_pos = lua.create_function(move |_, (id, x, y, z): (u64, f32, f32, f32)| {
        let mut api = api_clone.lock().unwrap();
        api.set_entity_position(id, Vec3 { x, y, z })
            .map_err(api_err_to_lua)
    })?;
    entity.set("set_position", set_pos)?;

    // position(id: number) -> table {x, y, z}
    let api_clone = api.clone();
    let get_pos = lua.create_function(move |lua, id: u64| {
        let api = api_clone.lock().unwrap();
        let pos = api.entity_position(id).map_err(api_err_to_lua)?;
        let t = lua.create_table()?;
        t.set("x", pos.x)?;
        t.set("y", pos.y)?;
        t.set("z", pos.z)?;
        Ok(t)
    })?;
    entity.set("position", get_pos)?;

    aether.set("entity", entity)?;
    Ok(())
}

fn register_physics_table(
    lua: &mlua::Lua,
    aether: &mlua::Table,
    api: Arc<Mutex<dyn PhysicsApi + Send>>,
) -> Result<(), mlua::Error> {
    let physics = lua.create_table()?;

    // apply_force(id, fx, fy, fz)
    let api_clone = api.clone();
    let apply_force =
        lua.create_function(move |_, (id, fx, fy, fz): (u64, f32, f32, f32)| {
            let mut api = api_clone.lock().unwrap();
            api.apply_force(id, fx, fy, fz).map_err(api_err_to_lua)
        })?;
    physics.set("apply_force", apply_force)?;

    // raycast(ox, oy, oz, dx, dy, dz, max_dist) -> bool
    let api_clone = api.clone();
    let raycast = lua.create_function(
        move |_, (ox, oy, oz, dx, dy, dz, max_dist): (f32, f32, f32, f32, f32, f32, f32)| {
            let api = api_clone.lock().unwrap();
            api.raycast(
                Vec3 { x: ox, y: oy, z: oz },
                Vec3 { x: dx, y: dy, z: dz },
                max_dist,
            )
            .map_err(api_err_to_lua)
        },
    )?;
    physics.set("raycast", raycast)?;

    aether.set("physics", physics)?;
    Ok(())
}

fn register_audio_table(
    lua: &mlua::Lua,
    aether: &mlua::Table,
    api: Arc<Mutex<dyn AudioApi + Send>>,
) -> Result<(), mlua::Error> {
    let audio = lua.create_table()?;

    // play(asset: string, vol: number, x, y, z) -> number (handle)
    let api_clone = api.clone();
    let play = lua.create_function(
        move |_, (asset, vol, x, y, z): (String, f32, f32, f32, f32)| {
            let mut api = api_clone.lock().unwrap();
            let handle = api
                .play_sound(&asset, vol, Vec3 { x, y, z })
                .map_err(api_err_to_lua)?;
            Ok(handle.0)
        },
    )?;
    audio.set("play", play)?;

    // stop(handle: number)
    let api_clone = api.clone();
    let stop = lua.create_function(move |_, handle: u64| {
        let mut api = api_clone.lock().unwrap();
        api.stop_sound(AudioHandle(handle)).map_err(api_err_to_lua)
    })?;
    audio.set("stop", stop)?;

    aether.set("audio", audio)?;
    Ok(())
}

fn register_network_table(
    lua: &mlua::Lua,
    aether: &mlua::Table,
    api: Arc<Mutex<dyn NetworkApi + Send>>,
) -> Result<(), mlua::Error> {
    let network = lua.create_table()?;

    // emit(topic: string, json: string)
    let api_clone = api.clone();
    let emit = lua.create_function(move |_, (topic, json): (String, String)| {
        let mut api = api_clone.lock().unwrap();
        api.emit_event(&topic, &json).map_err(api_err_to_lua)
    })?;
    network.set("emit", emit)?;

    // rpc(target: string, method: string, json: string)
    let api_clone = api.clone();
    let rpc = lua.create_function(move |_, (target, method, json): (String, String, String)| {
        let mut api = api_clone.lock().unwrap();
        api.send_rpc(&target, &method, &json)
            .map_err(api_err_to_lua)
    })?;
    network.set("rpc", rpc)?;

    aether.set("network", network)?;
    Ok(())
}

fn register_storage_table(
    lua: &mlua::Lua,
    aether: &mlua::Table,
    api: Arc<Mutex<dyn StorageApi + Send>>,
) -> Result<(), mlua::Error> {
    let storage = lua.create_table()?;

    // get(key: string) -> string | nil
    let api_clone2 = api.clone();
    let get = lua.create_function(move |lua, key: String| {
        let api = api_clone2.lock().unwrap();
        let val = api.world_get(&key).map_err(api_err_to_lua)?;
        match val {
            Some(bytes) => {
                let s = String::from_utf8_lossy(&bytes).into_owned();
                Ok(mlua::Value::String(lua.create_string(&s)?))
            }
            None => Ok(mlua::Value::Nil),
        }
    })?;
    storage.set("get", get)?;

    // set(key: string, value: string)
    let api_clone = api.clone();
    let set = lua.create_function(move |_, (key, value): (String, String)| {
        let mut api = api_clone.lock().unwrap();
        api.world_set(&key, value.as_bytes())
            .map_err(api_err_to_lua)
    })?;
    storage.set("set", set)?;

    aether.set("storage", storage)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_scripting::*;
    use std::collections::HashMap;

    // ── Mock implementations ──────────────────────────────────────────

    struct MockEntityApi {
        spawned: Vec<String>,
        next_id: u64,
        positions: HashMap<u64, Vec3>,
        reject_template: Option<String>,
    }

    impl MockEntityApi {
        fn new() -> Self {
            Self {
                spawned: Vec::new(),
                next_id: 1,
                positions: HashMap::new(),
                reject_template: None,
            }
        }
    }

    impl EntityApi for MockEntityApi {
        fn spawn_entity(&mut self, template: &str) -> ScriptApiResult<u64> {
            if let Some(ref reject) = self.reject_template {
                if template == reject {
                    return Err(ScriptApiError::PermissionDenied);
                }
            }
            let id = self.next_id;
            self.next_id += 1;
            self.spawned.push(template.to_string());
            self.positions.insert(
                id,
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            );
            Ok(id)
        }

        fn despawn_entity(&mut self, entity_id: u64) -> ScriptApiResult<()> {
            if self.positions.remove(&entity_id).is_some() {
                Ok(())
            } else {
                Err(ScriptApiError::NotFound)
            }
        }

        fn set_entity_position(
            &mut self,
            entity_id: u64,
            position: Vec3,
        ) -> ScriptApiResult<()> {
            if self.positions.contains_key(&entity_id) {
                self.positions.insert(entity_id, position);
                Ok(())
            } else {
                Err(ScriptApiError::NotFound)
            }
        }

        fn entity_position(&self, entity_id: u64) -> ScriptApiResult<Vec3> {
            self.positions
                .get(&entity_id)
                .copied()
                .ok_or(ScriptApiError::NotFound)
        }
    }

    struct MockPhysicsApi {
        forces_applied: Vec<(u64, f32, f32, f32)>,
        raycast_result: bool,
    }

    impl MockPhysicsApi {
        fn new() -> Self {
            Self {
                forces_applied: Vec::new(),
                raycast_result: false,
            }
        }
    }

    impl PhysicsApi for MockPhysicsApi {
        fn apply_force(
            &mut self,
            entity_id: u64,
            force_x: f32,
            force_y: f32,
            force_z: f32,
        ) -> ScriptApiResult<()> {
            self.forces_applied.push((entity_id, force_x, force_y, force_z));
            Ok(())
        }

        fn raycast(
            &self,
            _origin: Vec3,
            _direction: Vec3,
            _max_distance: f32,
        ) -> ScriptApiResult<bool> {
            Ok(self.raycast_result)
        }
    }

    struct MockAudioApi {
        played: Vec<(String, f32)>,
        stopped: Vec<u64>,
        next_handle: u64,
    }

    impl MockAudioApi {
        fn new() -> Self {
            Self {
                played: Vec::new(),
                stopped: Vec::new(),
                next_handle: 100,
            }
        }
    }

    impl AudioApi for MockAudioApi {
        fn play_sound(
            &mut self,
            asset_id: &str,
            volume: f32,
            _position: Vec3,
        ) -> ScriptApiResult<AudioHandle> {
            let handle = self.next_handle;
            self.next_handle += 1;
            self.played.push((asset_id.to_string(), volume));
            Ok(AudioHandle(handle))
        }

        fn stop_sound(&mut self, handle: AudioHandle) -> ScriptApiResult<()> {
            self.stopped.push(handle.0);
            Ok(())
        }
    }

    struct MockNetworkApi {
        emitted: Vec<(String, String)>,
        rpcs: Vec<(String, String, String)>,
    }

    impl MockNetworkApi {
        fn new() -> Self {
            Self {
                emitted: Vec::new(),
                rpcs: Vec::new(),
            }
        }
    }

    impl NetworkApi for MockNetworkApi {
        fn emit_event(&mut self, topic: &str, payload_json: &str) -> ScriptApiResult<()> {
            self.emitted.push((topic.to_string(), payload_json.to_string()));
            Ok(())
        }

        fn send_rpc(
            &mut self,
            target: &str,
            method: &str,
            payload_json: &str,
        ) -> ScriptApiResult<()> {
            self.rpcs
                .push((target.to_string(), method.to_string(), payload_json.to_string()));
            Ok(())
        }
    }

    struct MockStorageApi {
        data: HashMap<String, Vec<u8>>,
    }

    impl MockStorageApi {
        fn new() -> Self {
            Self {
                data: HashMap::new(),
            }
        }
    }

    impl StorageApi for MockStorageApi {
        fn world_get(&self, key: &str) -> ScriptApiResult<Option<Vec<u8>>> {
            Ok(self.data.get(key).cloned())
        }

        fn world_set(&mut self, key: &str, value: &[u8]) -> ScriptApiResult<()> {
            self.data.insert(key.to_string(), value.to_vec());
            Ok(())
        }
    }

    // ── Test helpers ──────────────────────────────────────────────────

    #[allow(dead_code)]
    struct TestEnv {
        lua: mlua::Lua,
        entity_api: Arc<Mutex<MockEntityApi>>,
        physics_api: Arc<Mutex<MockPhysicsApi>>,
        audio_api: Arc<Mutex<MockAudioApi>>,
        network_api: Arc<Mutex<MockNetworkApi>>,
        storage_api: Arc<Mutex<MockStorageApi>>,
    }

    fn setup() -> TestEnv {
        let lua = mlua::Lua::new();
        let entity_api = Arc::new(Mutex::new(MockEntityApi::new()));
        let physics_api = Arc::new(Mutex::new(MockPhysicsApi::new()));
        let audio_api = Arc::new(Mutex::new(MockAudioApi::new()));
        let network_api = Arc::new(Mutex::new(MockNetworkApi::new()));
        let storage_api = Arc::new(Mutex::new(MockStorageApi::new()));

        let ctx = ScriptContext {
            entity_api: entity_api.clone(),
            physics_api: physics_api.clone(),
            audio_api: audio_api.clone(),
            network_api: network_api.clone(),
            storage_api: storage_api.clone(),
            script_id: 1,
        };

        register_all(&lua, &ctx).expect("register_all should succeed");

        TestEnv {
            lua,
            entity_api,
            physics_api,
            audio_api,
            network_api,
            storage_api,
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_aether_table_exists() {
        let env = setup();
        let aether: mlua::Value = env.lua.globals().get("aether").unwrap();
        assert!(
            matches!(aether, mlua::Value::Table(_)),
            "aether should be a table"
        );
    }

    #[test]
    fn test_aether_time_table() {
        let env = setup();
        let dt: f64 = env
            .lua
            .load("return aether.time.dt")
            .eval()
            .unwrap();
        let tick: u64 = env
            .lua
            .load("return aether.time.tick")
            .eval()
            .unwrap();
        assert!((dt - 0.0).abs() < f64::EPSILON, "initial dt should be 0");
        assert_eq!(tick, 0, "initial tick should be 0");
    }

    #[test]
    fn test_entity_spawn_calls_api() {
        let env = setup();
        let id: u64 = env
            .lua
            .load(r#"return aether.entity.spawn("test_npc")"#)
            .eval()
            .unwrap();
        assert_eq!(id, 1);

        let api = env.entity_api.lock().unwrap();
        assert_eq!(api.spawned, vec!["test_npc"]);
    }

    #[test]
    fn test_entity_position_returns_table() {
        let env = setup();
        // Spawn an entity first so there's something to query
        {
            let mut api = env.entity_api.lock().unwrap();
            api.spawn_entity("dummy").unwrap();
            api.set_entity_position(
                1,
                Vec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
            )
            .unwrap();
        }

        env.lua
            .load(
                r#"
                local p = aether.entity.position(1)
                _G.px = p.x
                _G.py = p.y
                _G.pz = p.z
                "#,
            )
            .exec()
            .unwrap();

        let px: f32 = env.lua.globals().get("px").unwrap();
        let py: f32 = env.lua.globals().get("py").unwrap();
        let pz: f32 = env.lua.globals().get("pz").unwrap();
        assert!((px - 1.0).abs() < 0.001);
        assert!((py - 2.0).abs() < 0.001);
        assert!((pz - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_entity_despawn_calls_api() {
        let env = setup();
        // Spawn first
        let _: u64 = env
            .lua
            .load(r#"return aether.entity.spawn("npc")"#)
            .eval()
            .unwrap();
        // Despawn
        env.lua
            .load("aether.entity.despawn(1)")
            .exec()
            .unwrap();

        let api = env.entity_api.lock().unwrap();
        assert!(
            !api.positions.contains_key(&1),
            "entity should be despawned"
        );
    }

    #[test]
    fn test_entity_set_position_calls_api() {
        let env = setup();
        // Spawn first
        let _: u64 = env
            .lua
            .load(r#"return aether.entity.spawn("npc")"#)
            .eval()
            .unwrap();

        env.lua
            .load("aether.entity.set_position(1, 5.0, 6.0, 7.0)")
            .exec()
            .unwrap();

        let api = env.entity_api.lock().unwrap();
        let pos = api.positions.get(&1).unwrap();
        assert!((pos.x - 5.0).abs() < 0.001);
        assert!((pos.y - 6.0).abs() < 0.001);
        assert!((pos.z - 7.0).abs() < 0.001);
    }

    #[test]
    fn test_physics_apply_force() {
        let env = setup();
        env.lua
            .load("aether.physics.apply_force(10, 1.0, 2.0, 3.0)")
            .exec()
            .unwrap();

        let api = env.physics_api.lock().unwrap();
        assert_eq!(api.forces_applied.len(), 1);
        let (id, fx, fy, fz) = api.forces_applied[0];
        assert_eq!(id, 10);
        assert!((fx - 1.0).abs() < 0.001);
        assert!((fy - 2.0).abs() < 0.001);
        assert!((fz - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_physics_raycast() {
        let env = setup();
        // Default raycast_result is false
        let result: bool = env
            .lua
            .load("return aether.physics.raycast(0,0,0, 1,0,0, 100.0)")
            .eval()
            .unwrap();
        assert!(!result);
    }

    #[test]
    fn test_audio_play() {
        let env = setup();
        let handle: u64 = env
            .lua
            .load(r#"return aether.audio.play("explosion", 0.8, 1.0, 2.0, 3.0)"#)
            .eval()
            .unwrap();
        assert_eq!(handle, 100); // MockAudioApi starts at 100

        let api = env.audio_api.lock().unwrap();
        assert_eq!(api.played.len(), 1);
        assert_eq!(api.played[0].0, "explosion");
        assert!((api.played[0].1 - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_audio_stop() {
        let env = setup();
        // Play first to get a handle
        let handle: u64 = env
            .lua
            .load(r#"return aether.audio.play("music", 1.0, 0,0,0)"#)
            .eval()
            .unwrap();
        env.lua
            .load(&format!("aether.audio.stop({handle})"))
            .exec()
            .unwrap();

        let api = env.audio_api.lock().unwrap();
        assert_eq!(api.stopped, vec![handle]);
    }

    #[test]
    fn test_network_emit() {
        let env = setup();
        env.lua
            .load(r#"aether.network.emit("player_joined", '{"name":"test"}')"#)
            .exec()
            .unwrap();

        let api = env.network_api.lock().unwrap();
        assert_eq!(api.emitted.len(), 1);
        assert_eq!(api.emitted[0].0, "player_joined");
        assert_eq!(api.emitted[0].1, r#"{"name":"test"}"#);
    }

    #[test]
    fn test_network_rpc() {
        let env = setup();
        env.lua
            .load(r#"aether.network.rpc("server", "update_score", '{"score":100}')"#)
            .exec()
            .unwrap();

        let api = env.network_api.lock().unwrap();
        assert_eq!(api.rpcs.len(), 1);
        assert_eq!(api.rpcs[0].0, "server");
        assert_eq!(api.rpcs[0].1, "update_score");
        assert_eq!(api.rpcs[0].2, r#"{"score":100}"#);
    }

    #[test]
    fn test_storage_set_and_get() {
        let env = setup();
        env.lua
            .load(r#"aether.storage.set("key1", "value1")"#)
            .exec()
            .unwrap();

        let result: String = env
            .lua
            .load(r#"return aether.storage.get("key1")"#)
            .eval()
            .unwrap();
        assert_eq!(result, "value1");
    }

    #[test]
    fn test_storage_get_missing_returns_nil() {
        let env = setup();
        let result: mlua::Value = env
            .lua
            .load(r#"return aether.storage.get("nonexistent")"#)
            .eval()
            .unwrap();
        assert!(matches!(result, mlua::Value::Nil));
    }

    #[test]
    fn test_error_propagation() {
        let entity_api = Arc::new(Mutex::new(MockEntityApi::new()));
        {
            let mut api = entity_api.lock().unwrap();
            api.reject_template = Some("forbidden".to_string());
        }

        let lua = mlua::Lua::new();
        let ctx = ScriptContext {
            entity_api: entity_api.clone(),
            physics_api: Arc::new(Mutex::new(MockPhysicsApi::new())),
            audio_api: Arc::new(Mutex::new(MockAudioApi::new())),
            network_api: Arc::new(Mutex::new(MockNetworkApi::new())),
            storage_api: Arc::new(Mutex::new(MockStorageApi::new())),
            script_id: 1,
        };
        register_all(&lua, &ctx).unwrap();

        let result = lua
            .load(r#"aether.entity.spawn("forbidden")"#)
            .exec();
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("PermissionDenied"),
            "error should mention PermissionDenied: {err_msg}"
        );
    }
}
