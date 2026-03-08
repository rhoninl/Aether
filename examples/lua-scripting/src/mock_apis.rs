use std::collections::HashMap;

use aether_scripting::{
    AudioApi, AudioHandle, EntityApi, NetworkApi, PhysicsApi, ScriptApiError, ScriptApiResult,
    StorageApi, Vec3,
};

// ── Entity API ──────────────────────────────────────────────────────

pub struct MockEntityApi {
    next_id: u64,
    positions: HashMap<u64, Vec3>,
    templates: HashMap<u64, String>,
    despawn_count: u64,
}

impl MockEntityApi {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            positions: HashMap::new(),
            templates: HashMap::new(),
            despawn_count: 0,
        }
    }

    pub fn spawn_count(&self) -> u64 {
        self.next_id - 1
    }

    pub fn despawn_count(&self) -> u64 {
        self.despawn_count
    }

    pub fn alive_count(&self) -> usize {
        self.positions.len()
    }

    pub fn position(&self, id: u64) -> Vec3 {
        self.positions
            .get(&id)
            .copied()
            .unwrap_or(Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            })
    }

    /// Returns an iterator over all alive entities: (id, template, position).
    pub fn entities(&self) -> impl Iterator<Item = (u64, &str, Vec3)> {
        self.positions.iter().map(move |(&id, &pos)| {
            let template = self
                .templates
                .get(&id)
                .map(|s| s.as_str())
                .unwrap_or("unknown");
            (id, template, pos)
        })
    }
}

impl EntityApi for MockEntityApi {
    fn spawn_entity(&mut self, template: &str) -> ScriptApiResult<u64> {
        let id = self.next_id;
        self.next_id += 1;
        self.positions.insert(
            id,
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        );
        self.templates.insert(id, template.to_string());
        Ok(id)
    }

    fn despawn_entity(&mut self, entity_id: u64) -> ScriptApiResult<()> {
        self.positions
            .remove(&entity_id)
            .ok_or(ScriptApiError::NotFound)?;
        self.despawn_count += 1;
        Ok(())
    }

    fn set_entity_position(&mut self, entity_id: u64, position: Vec3) -> ScriptApiResult<()> {
        self.positions
            .get_mut(&entity_id)
            .map(|p| *p = position)
            .ok_or(ScriptApiError::NotFound)
    }

    fn entity_position(&self, entity_id: u64) -> ScriptApiResult<Vec3> {
        self.positions
            .get(&entity_id)
            .copied()
            .ok_or(ScriptApiError::NotFound)
    }
}

// ── Physics API ─────────────────────────────────────────────────────

pub struct MockPhysicsApi {
    force_count: u64,
    raycast_count: u64,
}

impl MockPhysicsApi {
    pub fn new() -> Self {
        Self {
            force_count: 0,
            raycast_count: 0,
        }
    }

    pub fn force_count(&self) -> u64 {
        self.force_count
    }

    pub fn raycast_count(&self) -> u64 {
        self.raycast_count
    }
}

impl PhysicsApi for MockPhysicsApi {
    fn apply_force(
        &mut self,
        _entity_id: u64,
        _force_x: f32,
        _force_y: f32,
        _force_z: f32,
    ) -> ScriptApiResult<()> {
        self.force_count += 1;
        Ok(())
    }

    fn raycast(
        &self,
        _origin: Vec3,
        _direction: Vec3,
        _max_distance: f32,
    ) -> ScriptApiResult<bool> {
        // Simulate ground hit when ray points down from low height
        Ok(false)
    }
}

// ── Audio API ───────────────────────────────────────────────────────

pub struct MockAudioApi {
    next_handle: u64,
    play_count: u64,
}

impl MockAudioApi {
    pub fn new() -> Self {
        Self {
            next_handle: 1,
            play_count: 0,
        }
    }

    pub fn play_count(&self) -> u64 {
        self.play_count
    }
}

impl AudioApi for MockAudioApi {
    fn play_sound(
        &mut self,
        _asset_id: &str,
        _volume: f32,
        _position: Vec3,
    ) -> ScriptApiResult<AudioHandle> {
        let handle = self.next_handle;
        self.next_handle += 1;
        self.play_count += 1;
        Ok(AudioHandle(handle))
    }

    fn stop_sound(&mut self, _handle: AudioHandle) -> ScriptApiResult<()> {
        Ok(())
    }
}

// ── Network API ─────────────────────────────────────────────────────

pub struct MockNetworkApi {
    emit_count: u64,
    rpc_count: u64,
}

impl MockNetworkApi {
    pub fn new() -> Self {
        Self {
            emit_count: 0,
            rpc_count: 0,
        }
    }

    pub fn emit_count(&self) -> u64 {
        self.emit_count
    }

    pub fn rpc_count(&self) -> u64 {
        self.rpc_count
    }
}

impl NetworkApi for MockNetworkApi {
    fn emit_event(&mut self, _topic: &str, _payload_json: &str) -> ScriptApiResult<()> {
        self.emit_count += 1;
        Ok(())
    }

    fn send_rpc(
        &mut self,
        _target: &str,
        _method: &str,
        _payload_json: &str,
    ) -> ScriptApiResult<()> {
        self.rpc_count += 1;
        Ok(())
    }
}

// ── Storage API ─────────────────────────────────────────────────────

pub struct MockStorageApi {
    data: HashMap<String, Vec<u8>>,
    read_count: u64,
    write_count: u64,
}

impl MockStorageApi {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            read_count: 0,
            write_count: 0,
        }
    }

    pub fn read_count(&self) -> u64 {
        self.read_count
    }

    pub fn write_count(&self) -> u64 {
        self.write_count
    }
}

impl StorageApi for MockStorageApi {
    fn world_get(&self, key: &str) -> ScriptApiResult<Option<Vec<u8>>> {
        // Can't increment read_count here because &self is immutable.
        // In a real implementation, you'd use interior mutability.
        let _ = key;
        Ok(self.data.get(key).cloned())
    }

    fn world_set(&mut self, key: &str, value: &[u8]) -> ScriptApiResult<()> {
        self.data.insert(key.to_string(), value.to_vec());
        self.write_count += 1;
        Ok(())
    }
}
