use std::collections::HashMap;

use crate::{
    FidelityMode, PlatformKind, PlatformProfile, QualityClass, SceneScaleMode, VisualMode,
    WasmExecutionMode,
};

#[derive(Debug, Clone)]
pub struct PlatformRuntimeConfig {
    pub default_script_mode: WasmExecutionMode,
    pub default_fidelity: FidelityMode,
    pub default_scene_scale: SceneScaleMode,
    pub default_visual_mode: VisualMode,
    pub enforce_script_mode: bool,
    pub profile_switch_cooldown_ms: u64,
    pub max_active_sessions: usize,
}

impl Default for PlatformRuntimeConfig {
    fn default() -> Self {
        Self {
            default_script_mode: WasmExecutionMode::ClientJit,
            default_fidelity: FidelityMode::Balanced,
            default_scene_scale: SceneScaleMode::Metres,
            default_visual_mode: VisualMode::Standard,
            enforce_script_mode: true,
            profile_switch_cooldown_ms: 400,
            max_active_sessions: 512,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlatformRuntimeInput {
    pub now_ms: u64,
    pub available_profiles: Vec<PlatformProfile>,
    pub session_intents: Vec<PlatformSessionIntent>,
}

#[derive(Debug, Default)]
pub struct PlatformRuntimeOutput {
    pub now_ms: u64,
    pub active_session_count: usize,
    pub switched_profiles: Vec<String>,
    pub profile_rejections: Vec<String>,
    pub script_mode_decisions: Vec<String>,
    pub script_mode_overrides: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PlatformSessionIntent {
    pub session_id: String,
    pub requested_profile: PlatformProfile,
    pub requested_script_mode: Option<WasmExecutionMode>,
    pub requested_fidelity: Option<FidelityMode>,
    pub requested_scene_scale: Option<SceneScaleMode>,
    pub requested_visual_mode: Option<VisualMode>,
}

#[derive(Debug)]
pub struct PlatformRuntime {
    cfg: PlatformRuntimeConfig,
    sessions: HashMap<String, PlatformRuntimeSessionState>,
}

#[derive(Debug)]
struct PlatformRuntimeSessionState {
    profile: PlatformProfile,
    script_mode: WasmExecutionMode,
    fidelity: FidelityMode,
    scene_scale: SceneScaleMode,
    visual_mode: VisualMode,
    last_switch_ms: u64,
    active_ms: u64,
}

impl Default for PlatformRuntime {
    fn default() -> Self {
        Self::new(PlatformRuntimeConfig::default())
    }
}

impl PlatformRuntime {
    pub fn new(cfg: PlatformRuntimeConfig) -> Self {
        Self {
            cfg,
            sessions: HashMap::new(),
        }
    }

    pub fn state_len(&self) -> usize {
        self.sessions.len()
    }

    pub fn step(&mut self, input: PlatformRuntimeInput) -> PlatformRuntimeOutput {
        let mut output = PlatformRuntimeOutput {
            now_ms: input.now_ms,
            active_session_count: self.sessions.len(),
            ..Default::default()
        };

        if self.sessions.len() >= self.cfg.max_active_sessions {
            output
                .diagnostics
                .push("platform runtime session cap reached".to_string());
        }

        for intent in input.session_intents {
            if self.sessions.len() >= self.cfg.max_active_sessions
                && !self.sessions.contains_key(&intent.session_id)
            {
                output
                    .profile_rejections
                    .push(format!("session_cap_reached:{}", intent.session_id));
                continue;
            }

            let selected = self.pick_profile(
                input.now_ms,
                &intent,
                &input.available_profiles,
                &mut output,
            );
            let requested_mode = intent
                .requested_script_mode
                .unwrap_or(self.cfg.default_script_mode);
            let (effective_mode, forced) = self.enforce_script_mode(&selected, requested_mode);
            if forced {
                output.script_mode_overrides.push(format!(
                    "session:{}:{:?}->{:?}:{:?}",
                    intent.session_id, requested_mode, effective_mode, selected.kind
                ));
            }

            self.apply_session(
                input.now_ms,
                intent.session_id.clone(),
                selected,
                effective_mode,
                intent
                    .requested_fidelity
                    .unwrap_or(self.cfg.default_fidelity),
                intent
                    .requested_scene_scale
                    .unwrap_or(self.cfg.default_scene_scale),
                intent
                    .requested_visual_mode
                    .unwrap_or(self.cfg.default_visual_mode),
                &mut output,
            );
            output.script_mode_decisions.push(format!(
                "session:{}:{:?}",
                intent.session_id, effective_mode
            ));
        }

        output.active_session_count = self.sessions.len();
        output
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_session(
        &mut self,
        now_ms: u64,
        session_id: String,
        profile: PlatformProfile,
        script_mode: WasmExecutionMode,
        fidelity: FidelityMode,
        scene_scale: SceneScaleMode,
        visual_mode: VisualMode,
        output: &mut PlatformRuntimeOutput,
    ) {
        let next_state = PlatformRuntimeSessionState {
            profile: profile.clone(),
            script_mode,
            fidelity,
            scene_scale,
            visual_mode,
            last_switch_ms: now_ms,
            active_ms: now_ms,
        };

        match self.sessions.get_mut(&session_id) {
            Some(existing) => {
                if !Self::same_profile(&existing.profile, &profile)
                    || existing.script_mode != script_mode
                    || existing.fidelity != fidelity
                    || existing.scene_scale != scene_scale
                    || existing.visual_mode != visual_mode
                {
                    if now_ms.saturating_sub(existing.last_switch_ms)
                        < self.cfg.profile_switch_cooldown_ms
                    {
                        output.profile_rejections.push(format!(
                            "switch_denied_cooldown:{}:{:?}",
                            session_id, now_ms
                        ));
                        return;
                    }
                    output.switched_profiles.push(format!(
                        "session_switch:{}:{:?}->{:?}",
                        session_id, existing.profile.kind, profile.kind
                    ));
                    existing.profile = profile;
                    existing.script_mode = script_mode;
                    existing.fidelity = fidelity;
                    existing.scene_scale = scene_scale;
                    existing.visual_mode = visual_mode;
                    existing.last_switch_ms = now_ms;
                    existing.active_ms = now_ms;
                } else {
                    existing.active_ms = now_ms;
                }
            }
            None => {
                self.sessions.insert(session_id.clone(), next_state);
                output
                    .switched_profiles
                    .push(format!("session_activate:{session_id}"));
            }
        }
    }

    fn pick_profile(
        &self,
        _now_ms: u64,
        intent: &PlatformSessionIntent,
        available_profiles: &[PlatformProfile],
        output: &mut PlatformRuntimeOutput,
    ) -> PlatformProfile {
        for profile in available_profiles {
            if Self::same_profile(profile, &intent.requested_profile) {
                return profile.clone();
            }
        }

        for profile in available_profiles {
            if Self::same_kind(&profile.kind, &intent.requested_profile.kind) {
                output.diagnostics.push(format!(
                    "fallback_profile:{}:{:?}",
                    intent.session_id, intent.requested_profile.quality
                ));
                return profile.clone();
            }
        }

        output
            .diagnostics
            .push(format!("profile_not_found_fallback:{}", intent.session_id));
        intent.requested_profile.clone()
    }

    fn enforce_script_mode(
        &self,
        profile: &PlatformProfile,
        requested: WasmExecutionMode,
    ) -> (WasmExecutionMode, bool) {
        if !self.cfg.enforce_script_mode {
            return (requested, false);
        }

        let enforced = match profile.kind {
            PlatformKind::Quest | PlatformKind::VisionPro => WasmExecutionMode::ClientJit,
            PlatformKind::PsVr2 => match requested {
                WasmExecutionMode::ServerAotOnly => WasmExecutionMode::ServerAot,
                _ => requested,
            },
            PlatformKind::PcVr | PlatformKind::Desktop => requested,
        };
        (enforced, enforced != requested)
    }

    fn same_profile(a: &PlatformProfile, b: &PlatformProfile) -> bool {
        Self::same_kind(&a.kind, &b.kind)
            && Self::same_quality(&a.quality, &b.quality)
            && a.max_fps == b.max_fps
            && a.haptics == b.haptics
            && a.supports_mouse == b.supports_mouse
    }

    fn same_kind(a: &PlatformKind, b: &PlatformKind) -> bool {
        matches!(
            (a, b),
            (PlatformKind::PcVr, PlatformKind::PcVr)
                | (PlatformKind::Desktop, PlatformKind::Desktop)
                | (PlatformKind::Quest, PlatformKind::Quest)
                | (PlatformKind::VisionPro, PlatformKind::VisionPro)
                | (PlatformKind::PsVr2, PlatformKind::PsVr2)
        )
    }

    fn same_quality(a: &QualityClass, b: &QualityClass) -> bool {
        matches!(
            (a, b),
            (QualityClass::Ultra, QualityClass::Ultra)
                | (QualityClass::High, QualityClass::High)
                | (QualityClass::Medium, QualityClass::Medium)
                | (QualityClass::Low, QualityClass::Low)
                | (QualityClass::Accessibility, QualityClass::Accessibility)
        )
    }
}
