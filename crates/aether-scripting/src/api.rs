#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioHandle(pub u64);

#[derive(Debug)]
pub enum ScriptApiError {
    PermissionDenied,
    NotFound,
    InvalidArgument,
    Internal(String),
}

pub type ScriptApiResult<T> = Result<T, ScriptApiError>;

pub trait EntityApi {
    fn spawn_entity(&mut self, template: &str) -> ScriptApiResult<u64>;
    fn despawn_entity(&mut self, entity_id: u64) -> ScriptApiResult<()>;
    fn set_entity_position(&mut self, entity_id: u64, position: Vec3) -> ScriptApiResult<()>;
    fn entity_position(&self, entity_id: u64) -> ScriptApiResult<Vec3>;
}

pub trait PhysicsApi {
    fn apply_force(
        &mut self,
        entity_id: u64,
        force_x: f32,
        force_y: f32,
        force_z: f32,
    ) -> ScriptApiResult<()>;

    fn raycast(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> ScriptApiResult<bool>;
}

pub trait UIApi {
    fn open_panel(&mut self, entity_id: u64, title: &str) -> ScriptApiResult<()>;
    fn close_panel(&mut self, entity_id: u64) -> ScriptApiResult<()>;
}

pub trait AudioApi {
    fn play_sound(
        &mut self,
        asset_id: &str,
        volume: f32,
        position: Vec3,
    ) -> ScriptApiResult<AudioHandle>;
    fn stop_sound(&mut self, handle: AudioHandle) -> ScriptApiResult<()>;
}

pub trait NetworkApi {
    fn emit_event(&mut self, topic: &str, payload_json: &str) -> ScriptApiResult<()>;
    fn send_rpc(&mut self, target: &str, method: &str, payload_json: &str) -> ScriptApiResult<()>;
}

pub trait StorageApi {
    fn world_get(&self, key: &str) -> ScriptApiResult<Option<Vec<u8>>>;
    fn world_set(&mut self, key: &str, value: &[u8]) -> ScriptApiResult<()>;
}
