//! glTF 2.0 import and parsing.

use crate::mesh::{ImportedMesh, Vertex};

/// A fully imported scene from a glTF file.
#[derive(Debug, Clone)]
pub struct ImportedScene {
    pub meshes: Vec<ImportedMesh>,
    pub materials: Vec<ImportedMaterial>,
    pub textures: Vec<ImportedTexture>,
}

impl ImportedScene {
    /// Total polygon (triangle) count across all meshes.
    pub fn total_triangles(&self) -> u32 {
        self.meshes.iter().map(|m| m.triangle_count()).sum()
    }

    /// Total texture memory in bytes.
    pub fn total_texture_bytes(&self) -> u64 {
        self.textures.iter().map(|t| t.size_bytes()).sum()
    }
}

/// PBR material properties extracted from glTF.
#[derive(Debug, Clone)]
pub struct ImportedMaterial {
    pub name: String,
    pub base_color: [f32; 4],
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub base_color_texture_index: Option<usize>,
}

/// Raw texture data extracted from glTF.
#[derive(Debug, Clone)]
pub struct ImportedTexture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Raw RGBA pixel data.
    pub data: Vec<u8>,
}

impl ImportedTexture {
    pub fn size_bytes(&self) -> u64 {
        self.data.len() as u64
    }
}

/// Errors during glTF import.
#[derive(Debug)]
pub enum GltfImportError {
    ParseError(String),
    MissingBuffer(String),
    UnsupportedFeature(String),
}

impl std::fmt::Display for GltfImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GltfImportError::ParseError(msg) => write!(f, "glTF parse error: {}", msg),
            GltfImportError::MissingBuffer(msg) => write!(f, "missing buffer: {}", msg),
            GltfImportError::UnsupportedFeature(msg) => {
                write!(f, "unsupported glTF feature: {}", msg)
            }
        }
    }
}

impl std::error::Error for GltfImportError {}

/// Imports glTF 2.0 files (.gltf/.glb) into the engine's internal format.
pub struct GltfImporter;

impl GltfImporter {
    /// Import a glTF file from raw bytes (supports both .gltf JSON and .glb binary).
    pub fn import(data: &[u8]) -> Result<ImportedScene, GltfImportError> {
        let (document, buffers, _images) = gltf::import_slice(data)
            .map_err(|e| GltfImportError::ParseError(e.to_string()))?;

        let mut meshes = Vec::new();
        let mut materials = Vec::new();
        let mut textures = Vec::new();

        // Extract materials
        for material in document.materials() {
            let pbr = material.pbr_metallic_roughness();
            materials.push(ImportedMaterial {
                name: material
                    .name()
                    .unwrap_or("unnamed_material")
                    .to_string(),
                base_color: pbr.base_color_factor(),
                metallic_factor: pbr.metallic_factor(),
                roughness_factor: pbr.roughness_factor(),
                base_color_texture_index: pbr
                    .base_color_texture()
                    .map(|t| t.texture().index()),
            });
        }

        // Extract meshes
        for mesh in document.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                let positions: Vec<[f32; 3]> = reader
                    .read_positions()
                    .map(|iter| iter.collect())
                    .unwrap_or_default();

                let normals: Vec<[f32; 3]> = reader
                    .read_normals()
                    .map(|iter| iter.collect())
                    .unwrap_or_else(|| vec![[0.0, 0.0, 1.0]; positions.len()]);

                let tex_coords: Vec<[f32; 2]> = reader
                    .read_tex_coords(0)
                    .map(|iter| iter.into_f32().collect())
                    .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

                let vertices: Vec<Vertex> = positions
                    .iter()
                    .enumerate()
                    .map(|(i, &pos)| {
                        Vertex::new(
                            pos,
                            normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]),
                            tex_coords.get(i).copied().unwrap_or([0.0, 0.0]),
                        )
                    })
                    .collect();

                let indices: Vec<u32> = reader
                    .read_indices()
                    .map(|iter| iter.into_u32().collect())
                    .unwrap_or_else(|| (0..vertices.len() as u32).collect());

                let name = mesh.name().unwrap_or("unnamed_mesh").to_string();
                meshes.push(ImportedMesh {
                    name,
                    vertices,
                    indices,
                });
            }
        }

        // Extract textures (from images in the glTF)
        for image in document.images() {
            let name = image.name().unwrap_or("unnamed_texture").to_string();
            // Images from gltf::import_slice are already decoded by the crate
            // but we track metadata from the document
            textures.push(ImportedTexture {
                name,
                width: 0,  // Will be populated from actual image data
                height: 0, // Will be populated from actual image data
                data: Vec::new(),
            });
        }

        Ok(ImportedScene {
            meshes,
            materials,
            textures,
        })
    }

    /// Import from a minimal glTF JSON + binary buffer pair.
    /// This is useful for testing and for .gltf files with external buffers.
    pub fn import_from_json_and_buffers(
        json: &str,
        buffers: &[Vec<u8>],
    ) -> Result<ImportedScene, GltfImportError> {
        // Build a GLB from the JSON + first buffer for simplicity
        let json_bytes = json.as_bytes();
        let json_len = json_bytes.len();
        // Pad JSON to 4-byte alignment
        let json_padded_len = (json_len + 3) & !3;
        let json_padding = json_padded_len - json_len;

        let bin_data = buffers.first().cloned().unwrap_or_default();
        let bin_len = bin_data.len();
        let bin_padded_len = (bin_len + 3) & !3;
        let bin_padding = bin_padded_len - bin_len;

        let total_len = 12 + 8 + json_padded_len + 8 + bin_padded_len;

        let mut glb = Vec::with_capacity(total_len);

        // GLB header
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes()); // version
        glb.extend_from_slice(&(total_len as u32).to_le_bytes());

        // JSON chunk
        glb.extend_from_slice(&(json_padded_len as u32).to_le_bytes());
        glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // "JSON"
        glb.extend_from_slice(json_bytes);
        glb.extend(std::iter::repeat(b' ').take(json_padding));

        // BIN chunk
        glb.extend_from_slice(&(bin_padded_len as u32).to_le_bytes());
        glb.extend_from_slice(&0x004E4942u32.to_le_bytes()); // "BIN\0"
        glb.extend_from_slice(&bin_data);
        glb.extend(std::iter::repeat(0u8).take(bin_padding));

        Self::import(&glb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid glTF JSON for a single triangle with a buffer.
    fn make_triangle_gltf() -> (String, Vec<u8>) {
        // Triangle vertices: 3 positions (3*3*4=36 bytes) + 3 indices as u16 (3*2=6 bytes, padded to 8)
        // Total buffer: 36 + 8 = 44 bytes
        let mut buffer = Vec::new();

        // 3 positions (float32 x 3)
        let positions: [[f32; 3]; 3] = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        for pos in &positions {
            for &v in pos {
                buffer.extend_from_slice(&v.to_le_bytes());
            }
        }
        // positions: 36 bytes (offset 0)

        // 3 indices as unsigned short
        let indices: [u16; 3] = [0, 1, 2];
        for &idx in &indices {
            buffer.extend_from_slice(&idx.to_le_bytes());
        }
        // indices: 6 bytes (offset 36), pad to 8
        buffer.extend_from_slice(&[0u8; 2]); // padding

        // Total: 44 bytes

        let json = serde_json::json!({
            "asset": { "version": "2.0" },
            "scene": 0,
            "scenes": [{ "nodes": [0] }],
            "nodes": [{ "mesh": 0 }],
            "meshes": [{
                "primitives": [{
                    "attributes": { "POSITION": 0 },
                    "indices": 1
                }]
            }],
            "accessors": [
                {
                    "bufferView": 0,
                    "componentType": 5126,
                    "count": 3,
                    "type": "VEC3",
                    "max": [1.0, 1.0, 0.0],
                    "min": [0.0, 0.0, 0.0]
                },
                {
                    "bufferView": 1,
                    "componentType": 5123,
                    "count": 3,
                    "type": "SCALAR",
                    "max": [2],
                    "min": [0]
                }
            ],
            "bufferViews": [
                {
                    "buffer": 0,
                    "byteOffset": 0,
                    "byteLength": 36,
                    "target": 34962
                },
                {
                    "buffer": 0,
                    "byteOffset": 36,
                    "byteLength": 6,
                    "target": 34963
                }
            ],
            "buffers": [{
                "byteLength": 44
            }]
        });

        (json.to_string(), buffer)
    }

    /// Build a glTF with a material.
    fn make_material_gltf() -> (String, Vec<u8>) {
        let mut buffer = Vec::new();

        let positions: [[f32; 3]; 3] = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        for pos in &positions {
            for &v in pos {
                buffer.extend_from_slice(&v.to_le_bytes());
            }
        }

        let indices: [u16; 3] = [0, 1, 2];
        for &idx in &indices {
            buffer.extend_from_slice(&idx.to_le_bytes());
        }
        buffer.extend_from_slice(&[0u8; 2]);

        let json = serde_json::json!({
            "asset": { "version": "2.0" },
            "scene": 0,
            "scenes": [{ "nodes": [0] }],
            "nodes": [{ "mesh": 0 }],
            "meshes": [{
                "primitives": [{
                    "attributes": { "POSITION": 0 },
                    "indices": 1,
                    "material": 0
                }]
            }],
            "materials": [{
                "name": "RedMaterial",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [1.0, 0.0, 0.0, 1.0],
                    "metallicFactor": 0.5,
                    "roughnessFactor": 0.8
                }
            }],
            "accessors": [
                {
                    "bufferView": 0,
                    "componentType": 5126,
                    "count": 3,
                    "type": "VEC3",
                    "max": [1.0, 1.0, 0.0],
                    "min": [0.0, 0.0, 0.0]
                },
                {
                    "bufferView": 1,
                    "componentType": 5123,
                    "count": 3,
                    "type": "SCALAR",
                    "max": [2],
                    "min": [0]
                }
            ],
            "bufferViews": [
                {
                    "buffer": 0,
                    "byteOffset": 0,
                    "byteLength": 36,
                    "target": 34962
                },
                {
                    "buffer": 0,
                    "byteOffset": 36,
                    "byteLength": 6,
                    "target": 34963
                }
            ],
            "buffers": [{
                "byteLength": 44
            }]
        });

        (json.to_string(), buffer)
    }

    #[test]
    fn import_triangle_from_glb() {
        let (json, buffer) = make_triangle_gltf();
        let scene = GltfImporter::import_from_json_and_buffers(&json, &[buffer]).unwrap();
        assert_eq!(scene.meshes.len(), 1);
        assert_eq!(scene.meshes[0].vertices.len(), 3);
        assert_eq!(scene.meshes[0].indices.len(), 3);
        assert_eq!(scene.meshes[0].triangle_count(), 1);
    }

    #[test]
    fn import_triangle_vertex_positions() {
        let (json, buffer) = make_triangle_gltf();
        let scene = GltfImporter::import_from_json_and_buffers(&json, &[buffer]).unwrap();
        let mesh = &scene.meshes[0];
        assert_eq!(mesh.vertices[0].position, [0.0, 0.0, 0.0]);
        assert_eq!(mesh.vertices[1].position, [1.0, 0.0, 0.0]);
        assert_eq!(mesh.vertices[2].position, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn import_triangle_indices() {
        let (json, buffer) = make_triangle_gltf();
        let scene = GltfImporter::import_from_json_and_buffers(&json, &[buffer]).unwrap();
        assert_eq!(scene.meshes[0].indices, vec![0, 1, 2]);
    }

    #[test]
    fn import_default_normals_when_absent() {
        let (json, buffer) = make_triangle_gltf();
        let scene = GltfImporter::import_from_json_and_buffers(&json, &[buffer]).unwrap();
        // When normals are absent, should default to [0,0,1]
        for vertex in &scene.meshes[0].vertices {
            assert_eq!(vertex.normal, [0.0, 0.0, 1.0]);
        }
    }

    #[test]
    fn import_default_uvs_when_absent() {
        let (json, buffer) = make_triangle_gltf();
        let scene = GltfImporter::import_from_json_and_buffers(&json, &[buffer]).unwrap();
        for vertex in &scene.meshes[0].vertices {
            assert_eq!(vertex.uv, [0.0, 0.0]);
        }
    }

    #[test]
    fn import_material_properties() {
        let (json, buffer) = make_material_gltf();
        let scene = GltfImporter::import_from_json_and_buffers(&json, &[buffer]).unwrap();
        assert_eq!(scene.materials.len(), 1);
        let mat = &scene.materials[0];
        assert_eq!(mat.name, "RedMaterial");
        assert_eq!(mat.base_color, [1.0, 0.0, 0.0, 1.0]);
        assert!((mat.metallic_factor - 0.5).abs() < f32::EPSILON);
        assert!((mat.roughness_factor - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn import_scene_total_triangles() {
        let (json, buffer) = make_triangle_gltf();
        let scene = GltfImporter::import_from_json_and_buffers(&json, &[buffer]).unwrap();
        assert_eq!(scene.total_triangles(), 1);
    }

    #[test]
    fn import_invalid_data_fails() {
        let result = GltfImporter::import(b"this is not valid gltf data");
        assert!(result.is_err());
    }

    #[test]
    fn import_empty_data_fails() {
        let result = GltfImporter::import(b"");
        assert!(result.is_err());
    }

    #[test]
    fn import_no_textures_when_absent() {
        let (json, buffer) = make_triangle_gltf();
        let scene = GltfImporter::import_from_json_and_buffers(&json, &[buffer]).unwrap();
        assert!(scene.textures.is_empty());
    }

    #[test]
    fn import_no_materials_when_absent() {
        let (json, buffer) = make_triangle_gltf();
        let scene = GltfImporter::import_from_json_and_buffers(&json, &[buffer]).unwrap();
        assert!(scene.materials.is_empty());
    }

    #[test]
    fn gltf_import_error_display() {
        let err = GltfImportError::ParseError("bad json".to_string());
        assert!(format!("{}", err).contains("bad json"));

        let err = GltfImportError::MissingBuffer("buffer 0".to_string());
        assert!(format!("{}", err).contains("buffer 0"));

        let err = GltfImportError::UnsupportedFeature("morph targets".to_string());
        assert!(format!("{}", err).contains("morph targets"));
    }

    #[test]
    fn imported_scene_total_texture_bytes_empty() {
        let scene = ImportedScene {
            meshes: vec![],
            materials: vec![],
            textures: vec![],
        };
        assert_eq!(scene.total_texture_bytes(), 0);
    }

    #[test]
    fn imported_texture_size_bytes() {
        let tex = ImportedTexture {
            name: "test".to_string(),
            width: 2,
            height: 2,
            data: vec![0u8; 16],
        };
        assert_eq!(tex.size_bytes(), 16);
    }

    #[test]
    fn imported_material_no_texture_index() {
        let mat = ImportedMaterial {
            name: "plain".to_string(),
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic_factor: 0.0,
            roughness_factor: 1.0,
            base_color_texture_index: None,
        };
        assert!(mat.base_color_texture_index.is_none());
    }
}
