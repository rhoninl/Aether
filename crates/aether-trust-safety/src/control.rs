#[derive(Debug, Clone)]
pub struct PersonalSpaceBubble {
    pub enabled: bool,
    pub radius_m: f32,
}

#[derive(Debug, Clone)]
pub struct AnonymousMode {
    pub enabled: bool,
    pub expires_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct SafetySettings {
    pub personal_space: PersonalSpaceBubble,
    pub anonymous_mode: AnonymousMode,
    pub allow_voice: bool,
}
