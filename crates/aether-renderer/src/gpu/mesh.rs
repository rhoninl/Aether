use bytemuck::{Pod, Zeroable};

/// Number of floats in a Vertex (3 pos + 3 normal + 2 uv).
const VERTEX_FLOAT_COUNT: usize = 8;
/// Size of a single Vertex in bytes.
pub const VERTEX_SIZE: usize = std::mem::size_of::<Vertex>();

/// PBR vertex: position, normal, UV.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex {
    /// The wgpu vertex buffer layout for this vertex type.
    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: VERTEX_SIZE as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &VERTEX_ATTRIBUTES,
        }
    }
}

/// Vertex attributes for the PBR vertex layout.
const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 3] = [
    // position
    wgpu::VertexAttribute {
        offset: 0,
        shader_location: 0,
        format: wgpu::VertexFormat::Float32x3,
    },
    // normal
    wgpu::VertexAttribute {
        offset: 12, // 3 * 4 bytes
        shader_location: 1,
        format: wgpu::VertexFormat::Float32x3,
    },
    // uv
    wgpu::VertexAttribute {
        offset: 24, // 6 * 4 bytes
        shader_location: 2,
        format: wgpu::VertexFormat::Float32x2,
    },
];

/// Identifier for a GPU mesh buffer pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshId(pub u64);

/// A mesh uploaded to the GPU as vertex + index buffers.
pub struct GpuMesh {
    pub id: MeshId,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub vertex_count: u32,
    pub index_format: wgpu::IndexFormat,
}

/// Create a vertex buffer from a slice of vertices.
pub fn create_vertex_buffer(device: &wgpu::Device, label: &str, vertices: &[Vertex]) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (vertices.len() * VERTEX_SIZE) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Create an index buffer from a slice of u32 indices.
pub fn create_index_buffer_u32(device: &wgpu::Device, label: &str, indices: &[u32]) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (indices.len() * std::mem::size_of::<u32>()) as u64,
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Create an index buffer from a slice of u16 indices.
pub fn create_index_buffer_u16(device: &wgpu::Device, label: &str, indices: &[u16]) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (indices.len() * std::mem::size_of::<u16>()) as u64,
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Upload vertex data to an existing buffer via the queue.
pub fn upload_vertices(queue: &wgpu::Queue, buffer: &wgpu::Buffer, vertices: &[Vertex]) {
    queue.write_buffer(buffer, 0, bytemuck::cast_slice(vertices));
}

/// Upload u32 index data to an existing buffer via the queue.
pub fn upload_indices_u32(queue: &wgpu::Queue, buffer: &wgpu::Buffer, indices: &[u32]) {
    queue.write_buffer(buffer, 0, bytemuck::cast_slice(indices));
}

/// Upload u16 index data to an existing buffer via the queue.
pub fn upload_indices_u16(queue: &wgpu::Queue, buffer: &wgpu::Buffer, indices: &[u16]) {
    queue.write_buffer(buffer, 0, bytemuck::cast_slice(indices));
}

/// Manages a collection of GPU mesh buffers.
pub struct MeshManager {
    meshes: std::collections::HashMap<MeshId, GpuMesh>,
    next_id: u64,
}

impl MeshManager {
    pub fn new() -> Self {
        Self {
            meshes: std::collections::HashMap::new(),
            next_id: 1,
        }
    }

    /// Upload a mesh to the GPU. Returns the assigned MeshId.
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertices: &[Vertex],
        indices: &[u32],
    ) -> MeshId {
        let id = MeshId(self.next_id);
        self.next_id += 1;

        let vb_label = format!("mesh-{}-vertex", id.0);
        let ib_label = format!("mesh-{}-index", id.0);

        let vertex_buffer = create_vertex_buffer(device, &vb_label, vertices);
        upload_vertices(queue, &vertex_buffer, vertices);

        let index_buffer = create_index_buffer_u32(device, &ib_label, indices);
        upload_indices_u32(queue, &index_buffer, indices);

        let gpu_mesh = GpuMesh {
            id,
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            vertex_count: vertices.len() as u32,
            index_format: wgpu::IndexFormat::Uint32,
        };

        self.meshes.insert(id, gpu_mesh);
        id
    }

    /// Get a reference to a GPU mesh by ID.
    pub fn get(&self, id: MeshId) -> Option<&GpuMesh> {
        self.meshes.get(&id)
    }

    /// Remove a mesh from the manager, dropping its GPU buffers.
    pub fn remove(&mut self, id: MeshId) -> bool {
        self.meshes.remove(&id).is_some()
    }

    /// Number of meshes currently managed.
    pub fn count(&self) -> usize {
        self.meshes.len()
    }
}

impl Default for MeshManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_size_is_32_bytes() {
        assert_eq!(VERTEX_SIZE, 32);
    }

    #[test]
    fn vertex_float_count_is_8() {
        assert_eq!(VERTEX_FLOAT_COUNT, 8);
    }

    #[test]
    fn vertex_is_pod_and_zeroable() {
        let v = Vertex::zeroed();
        assert_eq!(v.position, [0.0, 0.0, 0.0]);
        assert_eq!(v.normal, [0.0, 0.0, 0.0]);
        assert_eq!(v.uv, [0.0, 0.0]);
    }

    #[test]
    fn vertex_buffer_layout_has_correct_stride() {
        let layout = Vertex::buffer_layout();
        assert_eq!(layout.array_stride, 32);
        assert_eq!(layout.step_mode, wgpu::VertexStepMode::Vertex);
    }

    #[test]
    fn vertex_buffer_layout_has_3_attributes() {
        let layout = Vertex::buffer_layout();
        assert_eq!(layout.attributes.len(), 3);
    }

    #[test]
    fn vertex_attributes_offsets_are_correct() {
        assert_eq!(VERTEX_ATTRIBUTES[0].offset, 0);
        assert_eq!(VERTEX_ATTRIBUTES[0].shader_location, 0);
        assert_eq!(VERTEX_ATTRIBUTES[0].format, wgpu::VertexFormat::Float32x3);

        assert_eq!(VERTEX_ATTRIBUTES[1].offset, 12);
        assert_eq!(VERTEX_ATTRIBUTES[1].shader_location, 1);
        assert_eq!(VERTEX_ATTRIBUTES[1].format, wgpu::VertexFormat::Float32x3);

        assert_eq!(VERTEX_ATTRIBUTES[2].offset, 24);
        assert_eq!(VERTEX_ATTRIBUTES[2].shader_location, 2);
        assert_eq!(VERTEX_ATTRIBUTES[2].format, wgpu::VertexFormat::Float32x2);
    }

    #[test]
    fn mesh_id_equality() {
        assert_eq!(MeshId(1), MeshId(1));
        assert_ne!(MeshId(1), MeshId(2));
    }

    #[test]
    fn mesh_manager_starts_empty() {
        let mgr = MeshManager::new();
        assert_eq!(mgr.count(), 0);
        assert!(mgr.get(MeshId(1)).is_none());
    }

    #[test]
    fn mesh_manager_default_is_empty() {
        let mgr = MeshManager::default();
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn vertex_bytemuck_cast() {
        let vertices = [
            Vertex {
                position: [1.0, 2.0, 3.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.5, 0.5],
            },
            Vertex {
                position: [4.0, 5.0, 6.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
            },
        ];
        let bytes: &[u8] = bytemuck::cast_slice(&vertices);
        assert_eq!(bytes.len(), 64); // 2 * 32 bytes
    }

    #[test]
    fn vertex_round_trip_through_bytemuck() {
        let original = Vertex {
            position: [1.5, 2.5, 3.5],
            normal: [0.0, 1.0, 0.0],
            uv: [0.25, 0.75],
        };
        let bytes: &[u8] = bytemuck::bytes_of(&original);
        let restored: &Vertex = bytemuck::from_bytes(bytes);
        assert_eq!(restored.position, original.position);
        assert_eq!(restored.normal, original.normal);
        assert_eq!(restored.uv, original.uv);
    }
}
