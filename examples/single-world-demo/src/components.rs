//! Shared ECS components used by all systems in the single-world demo.
//!
//! These components bridge the gap between subsystems — each system reads/writes
//! specific components, and entities gain behavior by combining them.

use aether_renderer::gpu::material::MaterialId;
use aether_renderer::gpu::mesh::MeshId;

/// Spatial transform: position, rotation (quaternion), and scale.
///
/// This is the canonical source of truth for where an entity is in the world.
/// Physics, input, and network systems all write to this; the render system reads it.
#[derive(Clone, Debug)]
pub struct Transform {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0], // identity quaternion
            scale: [1.0, 1.0, 1.0],
        }
    }
}

impl Transform {
    pub fn at(x: f32, y: f32, z: f32) -> Self {
        Self {
            position: [x, y, z],
            ..Default::default()
        }
    }

    pub fn with_scale(mut self, sx: f32, sy: f32, sz: f32) -> Self {
        self.scale = [sx, sy, sz];
        self
    }

    /// Build a 4x4 model matrix (column-major) from position, rotation, scale.
    pub fn model_matrix(&self) -> [f32; 16] {
        let [qx, qy, qz, qw] = self.rotation;
        let [sx, sy, sz] = self.scale;

        let x2 = qx + qx;
        let y2 = qy + qy;
        let z2 = qz + qz;
        let xx = qx * x2;
        let xy = qx * y2;
        let xz = qx * z2;
        let yy = qy * y2;
        let yz = qy * z2;
        let zz = qz * z2;
        let wx = qw * x2;
        let wy = qw * y2;
        let wz = qw * z2;

        [
            (1.0 - yy - zz) * sx, (xy + wz) * sx,       (xz - wy) * sx,       0.0,
            (xy - wz) * sy,       (1.0 - xx - zz) * sy,  (yz + wx) * sy,       0.0,
            (xz + wy) * sz,       (yz - wx) * sz,        (1.0 - xx - yy) * sz, 0.0,
            self.position[0],     self.position[1],      self.position[2],      1.0,
        ]
    }

    /// Build a 4x4 normal matrix (inverse transpose of upper-left 3x3, padded to 4x4).
    pub fn normal_matrix(&self) -> [f32; 16] {
        let m = self.model_matrix();
        // For uniform scale, normal matrix = rotation matrix (no need to invert).
        // For non-uniform scale, we compute the inverse transpose of the 3x3.
        let [sx, sy, sz] = self.scale;
        let inv_sx = if sx.abs() > f32::EPSILON { 1.0 / sx } else { 0.0 };
        let inv_sy = if sy.abs() > f32::EPSILON { 1.0 / sy } else { 0.0 };
        let inv_sz = if sz.abs() > f32::EPSILON { 1.0 / sz } else { 0.0 };

        [
            m[0] * inv_sx, m[1] * inv_sx, m[2] * inv_sx, 0.0,
            m[4] * inv_sy, m[5] * inv_sy, m[6] * inv_sy, 0.0,
            m[8] * inv_sz, m[9] * inv_sz, m[10] * inv_sz, 0.0,
            0.0,           0.0,           0.0,            1.0,
        ]
    }
}

/// Marks an entity as renderable by the GPU renderer.
#[derive(Clone, Debug)]
pub struct Renderable {
    pub mesh_id: MeshId,
    pub material_id: MaterialId,
    /// Index into the model uniform buffer for this entity.
    pub model_index: usize,
}

/// Marks an entity as having a physics rigid body.
#[derive(Clone, Debug)]
pub struct PhysicsBody {
    pub body_type: PhysicsBodyType,
}

/// Physics body type for our demo.
#[derive(Clone, Debug, PartialEq)]
pub enum PhysicsBodyType {
    Dynamic,
    Static,
    Kinematic,
}

/// Marks the locally-controlled player entity.
#[derive(Clone, Debug, Default)]
pub struct LocalPlayer;

/// Marks a remote player's avatar entity, synced over network.
#[derive(Clone, Debug)]
pub struct NetworkAvatar {
    pub player_id: uuid::Uuid,
}

/// Marks an entity with an attached visual script.
#[derive(Clone, Debug)]
pub struct ScriptAttached {
    pub script_id: usize,
}

/// Marks a static environment object (floor, walls, etc.).
#[derive(Clone, Debug, Default)]
pub struct StaticObject;

// ---- Resources (stored in ECS World.resources) ----

/// Frame timing resource.
#[derive(Clone, Debug)]
pub struct DeltaTime(pub f32);

impl Default for DeltaTime {
    fn default() -> Self {
        Self(1.0 / 60.0)
    }
}

/// Desktop input state resource, updated each frame.
#[derive(Clone, Debug, Default)]
pub struct InputState {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub yaw_left: bool,
    pub yaw_right: bool,
    pub pitch_up: bool,
    pub pitch_down: bool,
}

/// Camera state for rendering.
#[derive(Clone, Debug)]
pub struct CameraState {
    pub position: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub fov_y: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            position: [0.0, 2.0, 5.0],
            yaw: 0.0,
            pitch: 0.0,
            fov_y: std::f32::consts::FRAC_PI_4,
            aspect: 16.0 / 9.0,
            near: 0.1,
            far: 500.0,
        }
    }
}

impl CameraState {
    /// Build a view matrix (column-major, right-handed look-at).
    pub fn view_matrix(&self) -> [f32; 16] {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();

        let forward = [cy * cp, sp, sy * cp];
        let right = [-sy, 0.0, cy];
        let up = [
            -(cy * sp),
            cp,
            -(sy * sp),
        ];

        let p = self.position;
        let dot_r = -(right[0] * p[0] + right[1] * p[1] + right[2] * p[2]);
        let dot_u = -(up[0] * p[0] + up[1] * p[1] + up[2] * p[2]);
        let dot_f = -(forward[0] * p[0] + forward[1] * p[1] + forward[2] * p[2]);

        [
            right[0],   up[0],   forward[0],  0.0,
            right[1],   up[1],   forward[1],  0.0,
            right[2],   up[2],   forward[2],  0.0,
            dot_r,       dot_u,   dot_f,       1.0,
        ]
    }

    /// Build a perspective projection matrix (column-major).
    pub fn projection_matrix(&self) -> [f32; 16] {
        let f = 1.0 / (self.fov_y / 2.0).tan();
        let range = self.near - self.far;

        [
            f / self.aspect, 0.0, 0.0,                              0.0,
            0.0,             f,   0.0,                              0.0,
            0.0,             0.0, (self.far + self.near) / range,  -1.0,
            0.0,             0.0, 2.0 * self.far * self.near / range, 0.0,
        ]
    }
}

/// Whether we're running in offline (no network) or online mode.
#[derive(Clone, Debug, PartialEq)]
pub enum NetworkMode {
    Offline,
    Online,
}

impl Default for NetworkMode {
    fn default() -> Self {
        Self::Offline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_default_identity() {
        let t = Transform::default();
        assert_eq!(t.position, [0.0, 0.0, 0.0]);
        assert_eq!(t.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(t.scale, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn transform_at_sets_position() {
        let t = Transform::at(1.0, 2.0, 3.0);
        assert_eq!(t.position, [1.0, 2.0, 3.0]);
        assert_eq!(t.rotation, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn transform_with_scale() {
        let t = Transform::at(0.0, 0.0, 0.0).with_scale(2.0, 3.0, 4.0);
        assert_eq!(t.scale, [2.0, 3.0, 4.0]);
    }

    #[test]
    fn identity_model_matrix() {
        let t = Transform::default();
        let m = t.model_matrix();
        // Should be identity matrix
        let expected = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        for (a, b) in m.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-6, "got {a}, expected {b}");
        }
    }

    #[test]
    fn translated_model_matrix() {
        let t = Transform::at(3.0, 4.0, 5.0);
        let m = t.model_matrix();
        assert!((m[12] - 3.0).abs() < 1e-6);
        assert!((m[13] - 4.0).abs() < 1e-6);
        assert!((m[14] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn scaled_model_matrix() {
        let t = Transform::default().with_scale(2.0, 3.0, 4.0);
        let m = t.model_matrix();
        assert!((m[0] - 2.0).abs() < 1e-6);
        assert!((m[5] - 3.0).abs() < 1e-6);
        assert!((m[10] - 4.0).abs() < 1e-6);
    }

    #[test]
    fn normal_matrix_uniform_scale() {
        let t = Transform::default().with_scale(2.0, 2.0, 2.0);
        let n = t.normal_matrix();
        // For uniform scale, normal matrix should have unit-length columns
        let col0_len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((col0_len - 1.0).abs() < 1e-6);
    }

    #[test]
    fn renderable_clone() {
        let r = Renderable {
            mesh_id: MeshId(1),
            material_id: MaterialId(2),
            model_index: 0,
        };
        let r2 = r.clone();
        assert_eq!(r2.mesh_id, MeshId(1));
    }

    #[test]
    fn physics_body_types() {
        assert_ne!(PhysicsBodyType::Dynamic, PhysicsBodyType::Static);
        assert_ne!(PhysicsBodyType::Static, PhysicsBodyType::Kinematic);
    }

    #[test]
    fn delta_time_default() {
        let dt = DeltaTime::default();
        assert!((dt.0 - 1.0 / 60.0).abs() < 1e-6);
    }

    #[test]
    fn input_state_default_all_false() {
        let i = InputState::default();
        assert!(!i.forward);
        assert!(!i.backward);
        assert!(!i.left);
        assert!(!i.right);
    }

    #[test]
    fn camera_state_default() {
        let c = CameraState::default();
        assert_eq!(c.position, [0.0, 2.0, 5.0]);
        assert!((c.fov_y - std::f32::consts::FRAC_PI_4).abs() < 1e-6);
    }

    #[test]
    fn camera_view_matrix_is_4x4() {
        let c = CameraState::default();
        let v = c.view_matrix();
        assert_eq!(v.len(), 16);
    }

    #[test]
    fn camera_projection_matrix_is_4x4() {
        let c = CameraState::default();
        let p = c.projection_matrix();
        assert_eq!(p.len(), 16);
        // w column should have -1 for perspective
        assert!((p[11] - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn network_mode_default_offline() {
        assert_eq!(NetworkMode::default(), NetworkMode::Offline);
    }

    #[test]
    fn network_avatar_stores_player_id() {
        let id = uuid::Uuid::new_v4();
        let a = NetworkAvatar { player_id: id };
        assert_eq!(a.player_id, id);
    }

    #[test]
    fn script_attached_stores_id() {
        let s = ScriptAttached { script_id: 42 };
        assert_eq!(s.script_id, 42);
    }

    #[test]
    fn static_object_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<StaticObject>();
        assert_send_sync::<Transform>();
        assert_send_sync::<Renderable>();
        assert_send_sync::<LocalPlayer>();
    }
}
