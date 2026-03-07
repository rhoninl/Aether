#[derive(Debug, Clone)]
pub enum PlatformKind {
    PcVr,
    Desktop,
    Quest,
    VisionPro,
    PsVr2,
}

#[derive(Debug, Clone)]
pub enum QualityClass {
    Ultra,
    High,
    Medium,
    Low,
    Accessibility,
}

#[derive(Debug, Clone)]
pub struct PlatformProfile {
    pub kind: PlatformKind,
    pub quality: QualityClass,
    pub max_fps: u32,
    pub haptics: bool,
    pub supports_mouse: bool,
}

#[derive(Debug)]
pub enum InputBackend {
    OpenXR,
    WebXR,
    Native,
}

