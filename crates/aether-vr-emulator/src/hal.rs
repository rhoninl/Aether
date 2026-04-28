//! HAL backend over the desktop emulator (P4-C, P6-A emulator side, P7-A, P7-B).
//!
//! Wraps the existing `EmulatorSession` / `EmulatedHeadTracker` /
//! `EmulatedControllers` / `StereoDisplay` types in implementations of the
//! `aether_xr_hal` traits so application code that targets the HAL works on a
//! developer machine without any OpenXR runtime installed.
//!
//! The image type for the emulator's swapchain is `Vec<u32>` (an RGBA8 frame
//! buffer in the same format `EmulatorFrameBuffer` already uses) so this file
//! has no `wgpu` dependency. P5-A/P5-B will drive the OpenXR backend toward
//! `wgpu::Texture`; the emulator follows in a later pass when the renderer
//! actually needs GPU textures.

use std::collections::HashMap;
use std::convert::Infallible;

use aether_xr_hal::action::{
    ActionDecl, ActionKind, ActionManifest, ActionSetHandle, ActionState, XrAction, XrActionSet,
};
use aether_xr_hal::event::XrEvent;
use aether_xr_hal::frame::{XrFrame, XrTime};
use aether_xr_hal::haptics::{HapticPulse, HapticTarget, XrHaptics};
use aether_xr_hal::instance::{
    ExtensionId, GraphicsRequirements, InstanceConfig, InstanceProperties, SystemProperties,
    ViewConfigType, XrInstance,
};
use aether_xr_hal::layer::{LayerBuilder, LayerSubmission};
use aether_xr_hal::platform::{RuntimeDescriptor, XrPlatform};
use aether_xr_hal::session::{
    ReferenceSpace, ReferenceSpaceType, SessionConfig, SessionState, XrSession,
};
use aether_xr_hal::swapchain::{
    SwapchainConfig, SwapchainError, SwapchainImageIndex, SwapchainState, XrSwapchain,
};
use aether_xr_hal::tracking::Pose3;
use aether_xr_hal::view::View;

use crate::session::EmulatorSessionState;

const RUNTIME_NAME: &str = "Aether Emulator";

/// Errors the emulator backend can return. Most operations succeed
/// unconditionally — the only real fault is misuse of the swapchain
/// acquire/wait/release sequence, surfaced via [`SwapchainError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmulatorError {
    Swapchain(SwapchainError),
    SessionNotRunning,
}

impl std::fmt::Display for EmulatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Swapchain(e) => write!(f, "swapchain: {e:?}"),
            Self::SessionNotRunning => write!(f, "session is not running"),
        }
    }
}

impl std::error::Error for EmulatorError {}

impl From<SwapchainError> for EmulatorError {
    fn from(e: SwapchainError) -> Self {
        Self::Swapchain(e)
    }
}

// ---------- Platform / Instance ----------

#[derive(Debug, Default)]
pub struct EmulatorPlatform;

impl EmulatorPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl XrPlatform for EmulatorPlatform {
    type Instance = EmulatorInstance;
    type Error = Infallible;

    fn available(&self) -> Result<Vec<RuntimeDescriptor>, Self::Error> {
        Ok(vec![RuntimeDescriptor {
            name: RUNTIME_NAME.to_string(),
            extensions: Vec::new(),
        }])
    }

    fn create_instance(&self, config: InstanceConfig) -> Result<Self::Instance, Self::Error> {
        Ok(EmulatorInstance::new(config))
    }
}

#[derive(Debug)]
pub struct EmulatorInstance {
    properties: InstanceProperties,
    system_properties: SystemProperties,
    enabled_extensions: Vec<ExtensionId>,
    view_configs: Vec<ViewConfigType>,
    pending_events: Vec<XrEvent>,
}

impl EmulatorInstance {
    fn new(config: InstanceConfig) -> Self {
        // Mirror what the OpenXR backend will do once it lands: report whatever
        // extensions the caller required + opted into. The emulator implements
        // none of them, but the application sees its requested set so its
        // capability checks behave the same way against either backend.
        let mut enabled = config.required_extensions.clone();
        enabled.extend(config.optional_extensions.iter().cloned());

        Self {
            properties: InstanceProperties {
                runtime_name: RUNTIME_NAME.to_string(),
                runtime_version: 1,
            },
            system_properties: SystemProperties {
                system_name: "Desktop Window".to_string(),
                vendor_id: 0,
                max_swapchain_image_width: 4096,
                max_swapchain_image_height: 4096,
                max_layer_count: 16,
            },
            enabled_extensions: enabled,
            view_configs: vec![ViewConfigType::Stereo, ViewConfigType::Mono],
            pending_events: Vec::new(),
        }
    }

    /// Push a synthesised event onto the queue. Used by the emulator's
    /// window/session glue to surface state-change events to applications.
    pub fn push_event(&mut self, event: XrEvent) {
        self.pending_events.push(event);
    }
}

impl XrInstance for EmulatorInstance {
    type Session = EmulatorHalSession;
    type Error = EmulatorError;

    fn properties(&self) -> InstanceProperties {
        self.properties.clone()
    }

    fn system_properties(&self) -> SystemProperties {
        self.system_properties.clone()
    }

    fn enabled_extensions(&self) -> &[ExtensionId] {
        &self.enabled_extensions
    }

    fn view_configurations(&self) -> &[ViewConfigType] {
        &self.view_configs
    }

    fn poll_events(&mut self) -> Vec<XrEvent> {
        std::mem::take(&mut self.pending_events)
    }

    fn create_session(
        &self,
        config: SessionConfig,
        _graphics: GraphicsRequirements,
    ) -> Result<Self::Session, Self::Error> {
        Ok(EmulatorHalSession::new(config))
    }
}

// ---------- Session ----------

#[derive(Debug)]
pub struct EmulatorHalSession {
    state: SessionState,
    config: SessionConfig,
    frame_count: u64,
    next_action_set_id: u32,
    attached_sets: Vec<ActionSetHandle>,
}

impl EmulatorHalSession {
    fn new(config: SessionConfig) -> Self {
        Self {
            state: SessionState::Idle,
            config,
            frame_count: 0,
            next_action_set_id: 0,
            attached_sets: Vec::new(),
        }
    }

    /// Translate from the existing emulator session state to the canonical
    /// HAL state. Used by [`EmulatorHalSession::sync_with`].
    pub fn map_state(emulator_state: EmulatorSessionState) -> SessionState {
        match emulator_state {
            EmulatorSessionState::Idle => SessionState::Idle,
            EmulatorSessionState::Ready => SessionState::Ready,
            EmulatorSessionState::Running => SessionState::Focused,
            EmulatorSessionState::Paused => SessionState::Visible,
            EmulatorSessionState::Stopping => SessionState::Stopping,
        }
    }

    /// Drive the HAL session state machine from the existing
    /// `EmulatorSession`'s state. Applications using the emulator HAL backend
    /// call this each tick after stepping the underlying emulator.
    pub fn sync_with(&mut self, emulator_state: EmulatorSessionState) {
        self.state = Self::map_state(emulator_state);
    }

    /// Allocate a fresh [`ActionSetHandle`] for use by [`EmulatorActionSet`].
    pub fn allocate_action_set_handle(&mut self) -> ActionSetHandle {
        let id = self.next_action_set_id;
        self.next_action_set_id += 1;
        ActionSetHandle(id)
    }
}

impl XrSession for EmulatorHalSession {
    type Frame = EmulatorHalFrame;
    type Swapchain = EmulatorSwapchain;
    type ActionSet = EmulatorActionSet;
    type Error = EmulatorError;

    fn state(&self) -> SessionState {
        self.state
    }

    fn begin(&mut self, _view_config: ViewConfigType) -> Result<(), Self::Error> {
        if self.state == SessionState::Idle || self.state == SessionState::Ready {
            self.state = SessionState::Synchronized;
        }
        Ok(())
    }

    fn end(&mut self) -> Result<(), Self::Error> {
        self.state = SessionState::Idle;
        Ok(())
    }

    fn request_exit(&mut self) -> Result<(), Self::Error> {
        self.state = SessionState::Exiting;
        Ok(())
    }

    fn create_reference_space(
        &self,
        kind: ReferenceSpaceType,
        offset: Pose3,
    ) -> Result<ReferenceSpace, Self::Error> {
        Ok(ReferenceSpace::with_offset(kind, offset))
    }

    fn create_swapchain(
        &self,
        config: SwapchainConfig,
    ) -> Result<Self::Swapchain, Self::Error> {
        Ok(EmulatorSwapchain::new(config))
    }

    fn attach_action_sets(&mut self, sets: &[Self::ActionSet]) -> Result<(), Self::Error> {
        self.attached_sets = sets.iter().map(|s| s.set.handle()).collect();
        Ok(())
    }

    fn wait_frame(&mut self) -> Result<Self::Frame, Self::Error> {
        let count = self.frame_count;
        self.frame_count = self.frame_count.saturating_add(1);
        Ok(EmulatorHalFrame::new(count, self.state, self.config.prediction_offset_ns))
    }
}

// ---------- Frame ----------

#[derive(Debug)]
pub struct EmulatorHalFrame {
    frame_index: u64,
    state: SessionState,
    predicted_display_time_ns: u64,
    began: bool,
    ended: bool,
}

impl EmulatorHalFrame {
    fn new(frame_index: u64, state: SessionState, prediction_offset_ns: u64) -> Self {
        // 90Hz cadence; one tick per wait_frame call. Keeps determinism so
        // tests can read predicted_display_time without wall-clock noise.
        let predicted = frame_index
            .saturating_mul(11_111_111)
            .saturating_add(prediction_offset_ns);
        Self {
            frame_index,
            state,
            predicted_display_time_ns: predicted,
            began: false,
            ended: false,
        }
    }
}

impl XrFrame for EmulatorHalFrame {
    type Error = EmulatorError;

    fn predicted_display_time(&self) -> XrTime {
        XrTime(self.predicted_display_time_ns as i64)
    }

    fn should_render(&self) -> bool {
        matches!(self.state, SessionState::Visible | SessionState::Focused)
    }

    fn locate_views(&self, _space: &ReferenceSpace) -> Result<Vec<View>, Self::Error> {
        // Stereo pair, fixed offset along X. Real backends drive this from
        // xrLocateViews; the emulator gives the application a stable layout
        // so render code that targets the HAL works during dev.
        let mut left = View::default();
        left.pose.position[0] = -0.032;
        let mut right = View::default();
        right.pose.position[0] = 0.032;
        Ok(vec![left, right])
    }

    fn sync_actions(&mut self, _sets: &[ActionSetHandle]) -> Result<(), Self::Error> {
        // Action state in the emulator is updated continuously by the
        // EmulatedControllers tick (see crate::controller); sync_actions is a
        // no-op here.
        Ok(())
    }

    fn begin(&mut self) -> Result<LayerBuilder<'_>, Self::Error> {
        self.began = true;
        Ok(LayerBuilder::new())
    }

    fn end(mut self, _layers: LayerSubmission) -> Result<(), Self::Error> {
        self.ended = true;
        // The actual desktop window present happens in VrEmulator::present();
        // composition layers are recorded but not used yet.
        Ok(())
    }
}

impl Drop for EmulatorHalFrame {
    fn drop(&mut self) {
        // Mirror OpenXR's contract: a frame must be ended after it begins.
        // The emulator can't enforce it through the runtime so we surface the
        // error in debug builds rather than silently swallowing it.
        debug_assert!(
            !self.began || self.ended,
            "EmulatorHalFrame {} was begun without ending",
            self.frame_index
        );
    }
}

// ---------- Swapchain ----------

#[derive(Debug)]
pub struct EmulatorSwapchain {
    config: SwapchainConfig,
    images: Vec<Vec<u32>>,
    state: SwapchainState,
    next_index: u32,
    current: Option<u32>,
}

impl EmulatorSwapchain {
    fn new(config: SwapchainConfig) -> Self {
        let pixel_count = (config.width as usize) * (config.height as usize);
        let images = (0..config.image_count)
            .map(|_| vec![0u32; pixel_count])
            .collect();
        Self {
            config,
            images,
            state: SwapchainState::Idle,
            next_index: 0,
            current: None,
        }
    }

    pub fn config(&self) -> &SwapchainConfig {
        &self.config
    }
}

impl XrSwapchain for EmulatorSwapchain {
    type Image = Vec<u32>;
    type Error = EmulatorError;

    fn images(&self) -> &[Self::Image] {
        &self.images
    }

    fn acquire(&mut self) -> Result<SwapchainImageIndex, Self::Error> {
        if self.state != SwapchainState::Idle {
            return Err(SwapchainError::AlreadyAcquired.into());
        }
        let index = self.next_index;
        self.next_index = (self.next_index + 1) % self.config.image_count.max(1);
        self.current = Some(index);
        self.state = SwapchainState::Acquired;
        Ok(SwapchainImageIndex(index))
    }

    fn wait(&mut self, _timeout_ns: u64) -> Result<(), Self::Error> {
        if self.state != SwapchainState::Acquired {
            return Err(SwapchainError::NotAcquired.into());
        }
        self.state = SwapchainState::Ready;
        Ok(())
    }

    fn release(&mut self) -> Result<(), Self::Error> {
        match self.state {
            SwapchainState::Idle => Err(SwapchainError::NoImageToRelease.into()),
            SwapchainState::Acquired => Err(SwapchainError::NotWaited.into()),
            SwapchainState::Ready => {
                self.state = SwapchainState::Idle;
                self.current = None;
                Ok(())
            }
        }
    }
}

// ---------- Action set / actions (P4-C) ----------

/// Emulator-side action-set handle. Wraps the HAL's [`XrActionSet`] plus
/// per-action backing storage so applications can write action values
/// directly during emulator ticks (e.g. when a key is pressed) and read them
/// back through `XrAction::current` exactly as they would against a real
/// OpenXR backend.
#[derive(Debug)]
pub struct EmulatorActionSet {
    set: XrActionSet,
    bool_actions: HashMap<String, bool>,
    float_actions: HashMap<String, f32>,
}

impl EmulatorActionSet {
    /// Build an emulator action set from a manifest. Allocates a handle from
    /// `session` so subsequent `XrSession::attach_action_sets` calls match.
    pub fn from_manifest(session: &mut EmulatorHalSession, manifest: &ActionManifest) -> Self {
        let handle = session.allocate_action_set_handle();
        let mut bool_actions = HashMap::new();
        let mut float_actions = HashMap::new();
        for ActionDecl { name, kind, .. } in manifest.actions() {
            match kind {
                ActionKind::Boolean => {
                    bool_actions.insert(name.clone(), false);
                }
                ActionKind::Float => {
                    float_actions.insert(name.clone(), 0.0);
                }
                // Vector2 / Pose / HapticVibration aren't surfaced through the
                // simple HashMap storage; leave them unbacked for V1. P4-C
                // followup wires axes-pair / pose actions if needed.
                _ => {}
            }
        }
        Self {
            set: XrActionSet::new(handle, manifest.name()),
            bool_actions,
            float_actions,
        }
    }

    pub fn set(&self) -> &XrActionSet {
        &self.set
    }

    pub fn set_bool(&mut self, action: &str, value: bool) {
        if let Some(slot) = self.bool_actions.get_mut(action) {
            *slot = value;
        }
    }

    pub fn set_float(&mut self, action: &str, value: f32) {
        if let Some(slot) = self.float_actions.get_mut(action) {
            *slot = value;
        }
    }

    pub fn bool(&self, action: &str) -> bool {
        *self.bool_actions.get(action).unwrap_or(&false)
    }

    pub fn float(&self, action: &str) -> f32 {
        *self.float_actions.get(action).unwrap_or(&0.0)
    }
}

/// Borrowed handle to a single boolean action inside an [`EmulatorActionSet`].
/// `XrAction::current` reads the current snapshot held in the set.
pub struct EmulatorBoolAction<'set> {
    name: String,
    set: &'set EmulatorActionSet,
}

impl<'set> EmulatorBoolAction<'set> {
    pub fn new(set: &'set EmulatorActionSet, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            set,
        }
    }
}

impl<'set> XrAction<bool> for EmulatorBoolAction<'set> {
    fn name(&self) -> &str {
        &self.name
    }

    fn current(&self, _frame: &impl XrFrame) -> ActionState<bool> {
        ActionState {
            current: self.set.bool(&self.name),
            changed_since_last_sync: false,
            is_active: true,
        }
    }

    fn suggest_bindings(
        &self,
        _profile: aether_xr_hal::profile::InteractionProfile,
        _paths: &[aether_xr_hal::profile::BindingPath],
    ) {
        // No-op: emulator binds keyboard/mouse continuously; suggested
        // bindings are recorded by the manifest but not consumed here.
    }
}

pub struct EmulatorFloatAction<'set> {
    name: String,
    set: &'set EmulatorActionSet,
}

impl<'set> EmulatorFloatAction<'set> {
    pub fn new(set: &'set EmulatorActionSet, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            set,
        }
    }
}

impl<'set> XrAction<f32> for EmulatorFloatAction<'set> {
    fn name(&self) -> &str {
        &self.name
    }

    fn current(&self, _frame: &impl XrFrame) -> ActionState<f32> {
        ActionState {
            current: self.set.float(&self.name),
            changed_since_last_sync: false,
            is_active: true,
        }
    }

    fn suggest_bindings(
        &self,
        _profile: aether_xr_hal::profile::InteractionProfile,
        _paths: &[aether_xr_hal::profile::BindingPath],
    ) {
    }
}

// ---------- Haptics (P6-A emulator side) ----------

/// No-op haptic backend that logs each call at debug level. Lets application
/// code that depends on `XrHaptics` run unchanged in the emulator.
#[derive(Debug, Default)]
pub struct EmulatorHaptics;

impl XrHaptics for EmulatorHaptics {
    type Error = Infallible;

    fn apply(&self, target: HapticTarget, pulse: HapticPulse) -> Result<(), Self::Error> {
        log::debug!(
            "EmulatorHaptics::apply target={target:?} dur_ns={} freq={} amp={}",
            pulse.duration_ns,
            pulse.frequency_hz,
            pulse.amplitude
        );
        Ok(())
    }

    fn stop(&self, target: HapticTarget) -> Result<(), Self::Error> {
        log::debug!("EmulatorHaptics::stop target={target:?}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance() -> EmulatorInstance {
        let platform = EmulatorPlatform::new();
        platform.create_instance(InstanceConfig::default()).unwrap()
    }

    #[test]
    fn platform_lists_one_runtime() {
        let p = EmulatorPlatform::new();
        let avail = p.available().unwrap();
        assert_eq!(avail.len(), 1);
        assert_eq!(avail[0].name, RUNTIME_NAME);
    }

    #[test]
    fn instance_reports_extensions_caller_requested() {
        let platform = EmulatorPlatform::new();
        let cfg = InstanceConfig {
            required_extensions: vec![ExtensionId::new("XR_KHR_vulkan_enable2")],
            optional_extensions: vec![ExtensionId::new("XR_EXT_hand_tracking")],
            ..InstanceConfig::default()
        };
        let inst = platform.create_instance(cfg).unwrap();
        assert_eq!(inst.enabled_extensions().len(), 2);
    }

    #[test]
    fn poll_events_drains_queue() {
        let mut inst = instance();
        inst.push_event(XrEvent::SessionStateChanged {
            state: SessionState::Ready,
        });
        inst.push_event(XrEvent::InteractionProfileChanged);
        let events = inst.poll_events();
        assert_eq!(events.len(), 2);
        assert!(inst.poll_events().is_empty());
    }

    #[test]
    fn session_state_walks_idle_to_focused() {
        let inst = instance();
        let mut session = inst
            .create_session(SessionConfig::default(), GraphicsRequirements::Headless)
            .unwrap();
        assert_eq!(session.state(), SessionState::Idle);
        session.begin(ViewConfigType::Stereo).unwrap();
        assert_eq!(session.state(), SessionState::Synchronized);
        session.sync_with(EmulatorSessionState::Running);
        assert_eq!(session.state(), SessionState::Focused);
        session.request_exit().unwrap();
        assert_eq!(session.state(), SessionState::Exiting);
    }

    #[test]
    fn frame_should_render_only_when_visible_or_focused() {
        let inst = instance();
        let mut session = inst
            .create_session(SessionConfig::default(), GraphicsRequirements::Headless)
            .unwrap();
        session.sync_with(EmulatorSessionState::Running);
        let frame = session.wait_frame().unwrap();
        assert!(frame.should_render());
        let _ = frame; // drop without begin/end is fine when not begun
    }

    #[test]
    fn frame_locate_views_returns_stereo_pair() {
        let inst = instance();
        let mut session = inst
            .create_session(SessionConfig::default(), GraphicsRequirements::Headless)
            .unwrap();
        let frame = session.wait_frame().unwrap();
        let views = frame
            .locate_views(&ReferenceSpace::new(ReferenceSpaceType::Local))
            .unwrap();
        assert_eq!(views.len(), 2);
        assert!(views[0].pose.position[0] < views[1].pose.position[0]);
    }

    #[test]
    fn swapchain_acquire_wait_release_cycle() {
        let inst = instance();
        let session = inst
            .create_session(SessionConfig::default(), GraphicsRequirements::Headless)
            .unwrap();
        let mut sc = session
            .create_swapchain(SwapchainConfig::default())
            .unwrap();
        assert_eq!(sc.images().len(), SwapchainConfig::default().image_count as usize);
        let idx = sc.acquire().unwrap();
        assert_eq!(idx, SwapchainImageIndex(0));
        // can't release before wait
        assert_eq!(
            sc.release(),
            Err(EmulatorError::Swapchain(SwapchainError::NotWaited))
        );
        sc.wait(0).unwrap();
        sc.release().unwrap();
        // and another cycle
        let idx2 = sc.acquire().unwrap();
        assert_eq!(idx2, SwapchainImageIndex(1));
    }

    #[test]
    fn frame_begin_end_records_layers() {
        let inst = instance();
        let mut session = inst
            .create_session(SessionConfig::default(), GraphicsRequirements::Headless)
            .unwrap();
        let mut frame = session.wait_frame().unwrap();
        let mut builder = frame.begin().unwrap();
        builder.add_projection_layer(0, View::default(), SwapchainImageIndex(0));
        builder.add_projection_layer(1, View::default(), SwapchainImageIndex(1));
        let submission = builder.finish();
        assert_eq!(submission.projection_views.len(), 2);
        frame.end(submission).unwrap();
    }

    #[test]
    fn action_set_from_manifest_registers_actions() {
        let inst = instance();
        let mut session = inst
            .create_session(SessionConfig::default(), GraphicsRequirements::Headless)
            .unwrap();
        let manifest = ActionManifest::new("gameplay", "Gameplay", 0)
            .action("jump", ActionKind::Boolean, |a| a)
            .action("look_x", ActionKind::Float, |a| a);
        let mut set = EmulatorActionSet::from_manifest(&mut session, &manifest);

        set.set_bool("jump", true);
        set.set_float("look_x", 0.42);

        let frame = session.wait_frame().unwrap();
        let jump = EmulatorBoolAction::new(&set, "jump");
        assert!(jump.current(&frame).current);
        let look_x = EmulatorFloatAction::new(&set, "look_x");
        assert!((look_x.current(&frame).current - 0.42).abs() < f32::EPSILON);
    }

    #[test]
    fn haptics_apply_and_stop_succeed() {
        let h = EmulatorHaptics;
        h.apply(
            HapticTarget::Left,
            HapticPulse {
                duration_ns: 100_000_000,
                frequency_hz: 320.0,
                amplitude: 0.5,
            },
        )
        .unwrap();
        h.stop(HapticTarget::Left).unwrap();
    }
}
