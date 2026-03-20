//! Asset type classification from file extensions.

use std::path::Path;

/// Supported asset categories for the hot-reload system.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetType {
    /// 3D mesh formats: glTF, GLB, FBX, OBJ
    Mesh,
    /// Image/texture formats: PNG, JPG, JPEG, KTX2, BMP, TGA, HDR
    Texture,
    /// Script formats: WASM, Lua
    Script,
    /// Material definition formats: JSON, TOML (material-specific)
    Material,
    /// Audio formats: OGG, WAV, MP3, FLAC
    Audio,
    /// Unrecognized file type
    Unknown,
}

impl AssetType {
    /// Classify a file path into an asset type based on its extension.
    pub fn from_path(path: &Path) -> Self {
        let extension = match path.extension() {
            Some(ext) => ext.to_string_lossy().to_lowercase(),
            None => return AssetType::Unknown,
        };

        match extension.as_str() {
            // Mesh formats
            "gltf" | "glb" | "fbx" | "obj" => AssetType::Mesh,
            // Texture formats
            "png" | "jpg" | "jpeg" | "ktx2" | "bmp" | "tga" | "hdr" => AssetType::Texture,
            // Script formats
            "wasm" | "lua" => AssetType::Script,
            // Material formats
            "material" | "mat" => AssetType::Material,
            // Audio formats
            "ogg" | "wav" | "mp3" | "flac" => AssetType::Audio,
            _ => AssetType::Unknown,
        }
    }
}

impl std::fmt::Display for AssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetType::Mesh => write!(f, "mesh"),
            AssetType::Texture => write!(f, "texture"),
            AssetType::Script => write!(f, "script"),
            AssetType::Material => write!(f, "material"),
            AssetType::Audio => write!(f, "audio"),
            AssetType::Unknown => write!(f, "unknown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_gltf() {
        assert_eq!(
            AssetType::from_path(Path::new("model.gltf")),
            AssetType::Mesh
        );
    }

    #[test]
    fn classify_glb() {
        assert_eq!(
            AssetType::from_path(Path::new("model.glb")),
            AssetType::Mesh
        );
    }

    #[test]
    fn classify_fbx() {
        assert_eq!(
            AssetType::from_path(Path::new("model.fbx")),
            AssetType::Mesh
        );
    }

    #[test]
    fn classify_obj() {
        assert_eq!(
            AssetType::from_path(Path::new("model.obj")),
            AssetType::Mesh
        );
    }

    #[test]
    fn classify_png() {
        assert_eq!(
            AssetType::from_path(Path::new("tex.png")),
            AssetType::Texture
        );
    }

    #[test]
    fn classify_jpg() {
        assert_eq!(
            AssetType::from_path(Path::new("tex.jpg")),
            AssetType::Texture
        );
    }

    #[test]
    fn classify_jpeg() {
        assert_eq!(
            AssetType::from_path(Path::new("tex.jpeg")),
            AssetType::Texture
        );
    }

    #[test]
    fn classify_ktx2() {
        assert_eq!(
            AssetType::from_path(Path::new("tex.ktx2")),
            AssetType::Texture
        );
    }

    #[test]
    fn classify_bmp() {
        assert_eq!(
            AssetType::from_path(Path::new("tex.bmp")),
            AssetType::Texture
        );
    }

    #[test]
    fn classify_tga() {
        assert_eq!(
            AssetType::from_path(Path::new("tex.tga")),
            AssetType::Texture
        );
    }

    #[test]
    fn classify_hdr() {
        assert_eq!(
            AssetType::from_path(Path::new("env.hdr")),
            AssetType::Texture
        );
    }

    #[test]
    fn classify_wasm() {
        assert_eq!(
            AssetType::from_path(Path::new("script.wasm")),
            AssetType::Script
        );
    }

    #[test]
    fn classify_lua() {
        assert_eq!(
            AssetType::from_path(Path::new("script.lua")),
            AssetType::Script
        );
    }

    #[test]
    fn classify_material() {
        assert_eq!(
            AssetType::from_path(Path::new("metal.material")),
            AssetType::Material
        );
    }

    #[test]
    fn classify_mat() {
        assert_eq!(
            AssetType::from_path(Path::new("metal.mat")),
            AssetType::Material
        );
    }

    #[test]
    fn classify_ogg() {
        assert_eq!(
            AssetType::from_path(Path::new("sound.ogg")),
            AssetType::Audio
        );
    }

    #[test]
    fn classify_wav() {
        assert_eq!(
            AssetType::from_path(Path::new("sound.wav")),
            AssetType::Audio
        );
    }

    #[test]
    fn classify_mp3() {
        assert_eq!(
            AssetType::from_path(Path::new("sound.mp3")),
            AssetType::Audio
        );
    }

    #[test]
    fn classify_flac() {
        assert_eq!(
            AssetType::from_path(Path::new("sound.flac")),
            AssetType::Audio
        );
    }

    #[test]
    fn classify_unknown_extension() {
        assert_eq!(
            AssetType::from_path(Path::new("readme.txt")),
            AssetType::Unknown
        );
    }

    #[test]
    fn classify_no_extension() {
        assert_eq!(
            AssetType::from_path(Path::new("Makefile")),
            AssetType::Unknown
        );
    }

    #[test]
    fn classify_case_insensitive() {
        assert_eq!(
            AssetType::from_path(Path::new("MODEL.GLB")),
            AssetType::Mesh
        );
        assert_eq!(
            AssetType::from_path(Path::new("TEX.PNG")),
            AssetType::Texture
        );
        assert_eq!(
            AssetType::from_path(Path::new("Script.LUA")),
            AssetType::Script
        );
    }

    #[test]
    fn classify_nested_path() {
        assert_eq!(
            AssetType::from_path(Path::new("assets/models/character/body.glb")),
            AssetType::Mesh
        );
    }

    #[test]
    fn classify_dotfile() {
        assert_eq!(
            AssetType::from_path(Path::new(".hidden")),
            AssetType::Unknown
        );
    }

    #[test]
    fn display_asset_types() {
        assert_eq!(format!("{}", AssetType::Mesh), "mesh");
        assert_eq!(format!("{}", AssetType::Texture), "texture");
        assert_eq!(format!("{}", AssetType::Script), "script");
        assert_eq!(format!("{}", AssetType::Material), "material");
        assert_eq!(format!("{}", AssetType::Audio), "audio");
        assert_eq!(format!("{}", AssetType::Unknown), "unknown");
    }
}
