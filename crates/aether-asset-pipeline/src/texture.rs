//! Texture processing and compression abstractions.

use crate::compression::TextureTranscode;

/// Raw texture input data.
#[derive(Debug, Clone)]
pub struct TextureInput {
    pub width: u32,
    pub height: u32,
    /// RGBA pixel data (4 bytes per pixel).
    pub data: Vec<u8>,
    pub name: String,
}

impl TextureInput {
    /// Create a new texture input with RGBA data.
    pub fn new(name: String, width: u32, height: u32, data: Vec<u8>) -> Result<Self, TextureError> {
        let expected_size = (width as usize) * (height as usize) * 4;
        if data.len() != expected_size {
            return Err(TextureError::InvalidDimensions {
                expected: expected_size,
                actual: data.len(),
            });
        }
        Ok(Self {
            width,
            height,
            data,
            name,
        })
    }

    /// Memory size of the raw texture in bytes.
    pub fn size_bytes(&self) -> u64 {
        self.data.len() as u64
    }
}

/// A compressed texture with format metadata.
#[derive(Debug, Clone)]
pub struct CompressedTexture {
    pub width: u32,
    pub height: u32,
    pub format: TextureTranscode,
    pub data: Vec<u8>,
    pub name: String,
}

impl CompressedTexture {
    /// Size of the compressed data in bytes.
    pub fn size_bytes(&self) -> u64 {
        self.data.len() as u64
    }
}

/// Trait for texture compression backends.
pub trait TextureCompressor {
    /// Compress a raw texture input to the specified format.
    fn compress(
        &self,
        input: &TextureInput,
        target: TextureTranscode,
    ) -> Result<CompressedTexture, TextureError>;
}

/// Errors during texture processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextureError {
    InvalidDimensions { expected: usize, actual: usize },
    UnsupportedFormat,
    CompressionFailed(String),
}

impl std::fmt::Display for TextureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextureError::InvalidDimensions { expected, actual } => {
                write!(
                    f,
                    "invalid texture dimensions: expected {} bytes, got {}",
                    expected, actual
                )
            }
            TextureError::UnsupportedFormat => write!(f, "unsupported texture format"),
            TextureError::CompressionFailed(msg) => {
                write!(f, "texture compression failed: {}", msg)
            }
        }
    }
}

impl std::error::Error for TextureError {}

/// Built-in passthrough texture compressor for testing.
///
/// Tags the data with the target format but does not perform actual compression.
/// Real compression (Basis Universal) should be provided via feature-gated backends.
pub struct PassthroughCompressor;

impl TextureCompressor for PassthroughCompressor {
    fn compress(
        &self,
        input: &TextureInput,
        target: TextureTranscode,
    ) -> Result<CompressedTexture, TextureError> {
        Ok(CompressedTexture {
            width: input.width,
            height: input.height,
            format: target,
            data: input.data.clone(),
            name: input.name.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_2x2_rgba() -> Vec<u8> {
        // 2x2 RGBA = 16 bytes
        vec![
            255, 0, 0, 255, // red
            0, 255, 0, 255, // green
            0, 0, 255, 255, // blue
            255, 255, 0, 255, // yellow
        ]
    }

    #[test]
    fn texture_input_valid_dimensions() {
        let data = make_2x2_rgba();
        let input = TextureInput::new("test".to_string(), 2, 2, data);
        assert!(input.is_ok());
        let input = input.unwrap();
        assert_eq!(input.width, 2);
        assert_eq!(input.height, 2);
    }

    #[test]
    fn texture_input_invalid_dimensions() {
        let data = vec![0u8; 10]; // Wrong size for any reasonable dimensions
        let result = TextureInput::new("test".to_string(), 2, 2, data);
        assert!(result.is_err());
        match result {
            Err(TextureError::InvalidDimensions { expected, actual }) => {
                assert_eq!(expected, 16);
                assert_eq!(actual, 10);
            }
            _ => panic!("expected InvalidDimensions error"),
        }
    }

    #[test]
    fn texture_input_size_bytes() {
        let data = make_2x2_rgba();
        let input = TextureInput::new("test".to_string(), 2, 2, data).unwrap();
        assert_eq!(input.size_bytes(), 16);
    }

    #[test]
    fn texture_input_1x1() {
        let data = vec![128, 128, 128, 255];
        let input = TextureInput::new("single_pixel".to_string(), 1, 1, data);
        assert!(input.is_ok());
        assert_eq!(input.unwrap().size_bytes(), 4);
    }

    #[test]
    fn texture_input_empty_zero_dimensions() {
        let data = vec![];
        let input = TextureInput::new("empty".to_string(), 0, 0, data);
        assert!(input.is_ok());
    }

    #[test]
    fn passthrough_compressor_bc7() {
        let data = make_2x2_rgba();
        let input = TextureInput::new("test".to_string(), 2, 2, data.clone()).unwrap();
        let compressor = PassthroughCompressor;
        let result = compressor.compress(&input, TextureTranscode::BC7);
        assert!(result.is_ok());
        let compressed = result.unwrap();
        assert_eq!(compressed.width, 2);
        assert_eq!(compressed.height, 2);
        assert!(matches!(compressed.format, TextureTranscode::BC7));
        assert_eq!(compressed.data, data);
    }

    #[test]
    fn passthrough_compressor_astc() {
        let data = make_2x2_rgba();
        let input = TextureInput::new("test".to_string(), 2, 2, data).unwrap();
        let compressor = PassthroughCompressor;
        let result = compressor.compress(&input, TextureTranscode::ASTC);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap().format, TextureTranscode::ASTC));
    }

    #[test]
    fn passthrough_compressor_etc2() {
        let data = make_2x2_rgba();
        let input = TextureInput::new("test".to_string(), 2, 2, data).unwrap();
        let compressor = PassthroughCompressor;
        let result = compressor.compress(&input, TextureTranscode::ETC2);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap().format, TextureTranscode::ETC2));
    }

    #[test]
    fn passthrough_compressor_preserves_name() {
        let data = make_2x2_rgba();
        let input = TextureInput::new("my_texture".to_string(), 2, 2, data).unwrap();
        let compressor = PassthroughCompressor;
        let result = compressor.compress(&input, TextureTranscode::BC7).unwrap();
        assert_eq!(result.name, "my_texture");
    }

    #[test]
    fn compressed_texture_size_bytes() {
        let data = make_2x2_rgba();
        let input = TextureInput::new("test".to_string(), 2, 2, data).unwrap();
        let compressor = PassthroughCompressor;
        let result = compressor.compress(&input, TextureTranscode::BC7).unwrap();
        assert_eq!(result.size_bytes(), 16);
    }

    #[test]
    fn texture_error_display_invalid_dimensions() {
        let err = TextureError::InvalidDimensions {
            expected: 16,
            actual: 10,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("16"));
        assert!(msg.contains("10"));
    }

    #[test]
    fn texture_error_display_unsupported() {
        let err = TextureError::UnsupportedFormat;
        let msg = format!("{}", err);
        assert!(msg.contains("unsupported"));
    }

    #[test]
    fn texture_error_display_compression_failed() {
        let err = TextureError::CompressionFailed("out of memory".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("out of memory"));
    }

    #[test]
    fn larger_texture_4x4() {
        let data = vec![0u8; 4 * 4 * 4]; // 4x4 RGBA
        let input = TextureInput::new("large".to_string(), 4, 4, data).unwrap();
        assert_eq!(input.size_bytes(), 64);
    }
}
