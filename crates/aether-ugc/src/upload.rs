//! Upload handling with size validation and file type checking.

use crate::validation::FileType;
use uuid::Uuid;

const DEFAULT_MAX_UPLOAD_BYTES: u64 = 100 * 1024 * 1024; // 100 MB
const DEFAULT_MAX_SCRIPT_BYTES: u64 = 10 * 1024 * 1024; // 10 MB
const DEFAULT_MAX_AUDIO_BYTES: u64 = 50 * 1024 * 1024; // 50 MB
const ASSET_NAME_MAX_LEN: usize = 256;

#[derive(Debug, Clone)]
pub struct UploadRequest {
    pub creator_id: Uuid,
    pub asset_name: String,
    pub file_type: FileType,
    pub data: Vec<u8>,
    pub parent_version: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct UploadConfig {
    pub max_upload_bytes: u64,
    pub max_script_bytes: u64,
    pub max_audio_bytes: u64,
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            max_upload_bytes: DEFAULT_MAX_UPLOAD_BYTES,
            max_script_bytes: DEFAULT_MAX_SCRIPT_BYTES,
            max_audio_bytes: DEFAULT_MAX_AUDIO_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UploadError {
    EmptyData,
    InvalidName(String),
    SizeExceeded { max: u64, actual: u64 },
    UnsupportedType(String),
}

impl std::fmt::Display for UploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UploadError::EmptyData => write!(f, "upload data is empty"),
            UploadError::InvalidName(reason) => write!(f, "invalid asset name: {reason}"),
            UploadError::SizeExceeded { max, actual } => {
                write!(f, "upload size {actual} exceeds maximum {max}")
            }
            UploadError::UnsupportedType(t) => write!(f, "unsupported file type: {t}"),
        }
    }
}

impl std::error::Error for UploadError {}

impl UploadConfig {
    pub fn validate(&self, request: &UploadRequest) -> Result<(), UploadError> {
        if request.data.is_empty() {
            return Err(UploadError::EmptyData);
        }

        let name = request.asset_name.trim();
        if name.is_empty() {
            return Err(UploadError::InvalidName("name is empty".into()));
        }
        if name.len() > ASSET_NAME_MAX_LEN {
            return Err(UploadError::InvalidName("name too long".into()));
        }

        let actual = request.data.len() as u64;
        let max = self.max_for_type(&request.file_type);
        if actual > max {
            return Err(UploadError::SizeExceeded { max, actual });
        }

        if matches!(request.file_type, FileType::Unknown) {
            return Err(UploadError::UnsupportedType("Unknown".into()));
        }

        Ok(())
    }

    fn max_for_type(&self, file_type: &FileType) -> u64 {
        match file_type {
            FileType::Lua | FileType::Wasm => self.max_script_bytes,
            FileType::Wav | FileType::Mp3 => self.max_audio_bytes,
            _ => self.max_upload_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(name: &str, file_type: FileType, size: usize) -> UploadRequest {
        UploadRequest {
            creator_id: Uuid::new_v4(),
            asset_name: name.to_string(),
            file_type,
            data: vec![0u8; size],
            parent_version: None,
        }
    }

    #[test]
    fn valid_upload_passes() {
        let cfg = UploadConfig::default();
        let req = make_request("my_model.glb", FileType::Glb, 1024);
        assert!(cfg.validate(&req).is_ok());
    }

    #[test]
    fn empty_data_rejected() {
        let cfg = UploadConfig::default();
        let req = make_request("test.glb", FileType::Glb, 0);
        assert_eq!(cfg.validate(&req).unwrap_err(), UploadError::EmptyData);
    }

    #[test]
    fn empty_name_rejected() {
        let cfg = UploadConfig::default();
        let req = make_request("  ", FileType::Glb, 100);
        match cfg.validate(&req).unwrap_err() {
            UploadError::InvalidName(_) => {}
            other => panic!("expected InvalidName, got {other:?}"),
        }
    }

    #[test]
    fn name_too_long_rejected() {
        let cfg = UploadConfig::default();
        let long_name = "a".repeat(ASSET_NAME_MAX_LEN + 1);
        let req = make_request(&long_name, FileType::Glb, 100);
        match cfg.validate(&req).unwrap_err() {
            UploadError::InvalidName(_) => {}
            other => panic!("expected InvalidName, got {other:?}"),
        }
    }

    #[test]
    fn size_exceeded_for_general_file() {
        let cfg = UploadConfig {
            max_upload_bytes: 500,
            ..Default::default()
        };
        let req = make_request("big.glb", FileType::Glb, 600);
        match cfg.validate(&req).unwrap_err() {
            UploadError::SizeExceeded { max, actual } => {
                assert_eq!(max, 500);
                assert_eq!(actual, 600);
            }
            other => panic!("expected SizeExceeded, got {other:?}"),
        }
    }

    #[test]
    fn size_exceeded_for_script() {
        let cfg = UploadConfig {
            max_script_bytes: 200,
            ..Default::default()
        };
        let req = make_request("script.lua", FileType::Lua, 300);
        match cfg.validate(&req).unwrap_err() {
            UploadError::SizeExceeded { max, actual } => {
                assert_eq!(max, 200);
                assert_eq!(actual, 300);
            }
            other => panic!("expected SizeExceeded, got {other:?}"),
        }
    }

    #[test]
    fn size_exceeded_for_audio() {
        let cfg = UploadConfig {
            max_audio_bytes: 100,
            ..Default::default()
        };
        let req = make_request("sound.wav", FileType::Wav, 200);
        match cfg.validate(&req).unwrap_err() {
            UploadError::SizeExceeded { max, actual } => {
                assert_eq!(max, 100);
                assert_eq!(actual, 200);
            }
            other => panic!("expected SizeExceeded, got {other:?}"),
        }
    }

    #[test]
    fn unknown_file_type_rejected() {
        let cfg = UploadConfig::default();
        let req = make_request("file.xyz", FileType::Unknown, 100);
        match cfg.validate(&req).unwrap_err() {
            UploadError::UnsupportedType(_) => {}
            other => panic!("expected UnsupportedType, got {other:?}"),
        }
    }

    #[test]
    fn valid_audio_under_limit() {
        let cfg = UploadConfig::default();
        let req = make_request("track.mp3", FileType::Mp3, 1024);
        assert!(cfg.validate(&req).is_ok());
    }

    #[test]
    fn valid_script_under_limit() {
        let cfg = UploadConfig::default();
        let req = make_request("main.wasm", FileType::Wasm, 512);
        assert!(cfg.validate(&req).is_ok());
    }

    #[test]
    fn parent_version_does_not_affect_validation() {
        let cfg = UploadConfig::default();
        let mut req = make_request("model.glb", FileType::Glb, 100);
        req.parent_version = Some(5);
        assert!(cfg.validate(&req).is_ok());
    }
}
