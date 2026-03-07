#[derive(Debug, Clone)]
pub enum FbxImport {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone)]
pub enum GltfImport {
    Draco,
    Uncompressed,
}

#[derive(Debug, Clone)]
pub enum ObjImport {
    Triangulate,
    KeepQuads,
}

#[derive(Debug, Clone)]
pub struct ImportTask {
    pub task_id: String,
    pub source_path: String,
    pub source_type: String,
    pub requested_formats: Vec<TextureTranscode>,
}

pub use crate::compression::TextureTranscode;

