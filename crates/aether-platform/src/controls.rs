#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FidelityMode {
    Full,
    Balanced,
    Low,
    Accessibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneScaleMode {
    Metres,
    Scaled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualMode {
    Standard,
    Spectator,
}
