#[derive(Debug, Clone)]
pub enum TextureFormat {
    Rgba8,
    BasisUniversal,
}

#[derive(Debug, Clone)]
pub enum TextureTranscode {
    BC7,
    ASTC,
    ETC2,
}

#[derive(Debug, Clone)]
pub enum VertexCompression {
    Meshopt,
    Draco,
    None,
}

