#[derive(Debug, Clone)]
pub enum FileType {
    Png,
    Glb,
    Gltf,
    Wav,
    Mp3,
    Wasm,
    Lua,
    Txt,
    Unknown,
}

#[derive(Debug)]
pub enum ValidationError {
    UnsupportedType,
    TooLarge,
    Corrupt,
    TypeMismatch,
}

#[derive(Debug)]
pub struct FileValidation {
    pub file_type: FileType,
    pub mime: String,
    pub max_bytes: u64,
    pub allowed: bool,
}

#[derive(Debug)]
pub struct ValidationReport {
    pub file_name: String,
    pub accepted: bool,
    pub error: Option<ValidationError>,
    pub checksum: Option<String>,
}

