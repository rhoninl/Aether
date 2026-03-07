#[derive(Debug)]
pub struct PersonalSpaceBubble {
    pub enabled: bool,
    pub radius_m: f32,
}

#[derive(Debug)]
pub struct AnonymousMode {
    pub enabled: bool,
    pub expires_ms: Option<u64>,
}

#[derive(Debug)]
pub struct SafetySettings {
    pub personal_space: PersonalSpaceBubble,
    pub anonymous_mode: AnonymousMode,
    pub allow_voice: bool,
}

