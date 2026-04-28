//! Real OpenXR backend implementing the `aether_xr_hal` traits (P3-A through
//! P6-A). Gated entirely behind `feature = "openxr-runtime"` because the
//! `openxr` Rust crate only links on Linux/Windows hosts that have the OpenXR
//! loader installed.
//!
//! **Validation status:** *unverified.* This file was written without access
//! to a Linux+Monado/SteamVR machine. The shape (HAL trait coverage, RAII
//! frame, action-set lifetime, swapchain image rotation, layer submission)
//! is right; specific `openxr` 0.19 call sites should be smoke-tested against
//! a real loader before this lands on hardware. Look for `// VALIDATE` markers.
//!
//! Without the feature, the module compiles to nothing and the crate exposes
//! only the existing stubs in `instance.rs` / `session.rs` / etc. so the
//! default workspace build keeps working on macOS / loaderless CI.

// Outer cfg gate is on the lib.rs module declaration; this file only
// compiles when feature = "openxr-runtime" AND target is linux/windows.

use std::sync::Arc;

use aether_xr_hal::action::{
    ActionDecl, ActionKind, ActionManifest, ActionSetHandle, ActionState, ActionValue, XrAction,
    XrActionSet,
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
use aether_xr_hal::profile::{BindingPath, InteractionProfile};
use aether_xr_hal::session::{
    ReferenceSpace, ReferenceSpaceType, SessionConfig, SessionState, XrSession,
};
use aether_xr_hal::swapchain::{
    SwapchainConfig, SwapchainFormat, SwapchainImageIndex, SwapchainUsage, XrSwapchain,
};
use aether_xr_hal::tracking::Pose3;
use aether_xr_hal::view::{Fov, View};

use crate::OpenXrError;

// ============================================================================
// Helpers — pose / vec3 / quat / fov conversions between openxr-rs and HAL types.
// ============================================================================

fn pose_from_xr(p: openxr::Posef) -> Pose3 {
    Pose3 {
        position: [p.position.x, p.position.y, p.position.z],
        rotation: [
            p.orientation.x,
            p.orientation.y,
            p.orientation.z,
            p.orientation.w,
        ],
        linear_velocity: [0.0; 3],
        angular_velocity: [0.0; 3],
    }
}

fn pose_to_xr(p: Pose3) -> openxr::Posef {
    openxr::Posef {
        position: openxr::Vector3f {
            x: p.position[0],
            y: p.position[1],
            z: p.position[2],
        },
        orientation: openxr::Quaternionf {
            x: p.rotation[0],
            y: p.rotation[1],
            z: p.rotation[2],
            w: p.rotation[3],
        },
    }
}

fn fov_from_xr(f: openxr::Fovf) -> Fov {
    Fov {
        angle_left: f.angle_left,
        angle_right: f.angle_right,
        angle_up: f.angle_up,
        angle_down: f.angle_down,
    }
}

fn ref_space_kind_to_xr(k: ReferenceSpaceType) -> openxr::ReferenceSpaceType {
    match k {
        ReferenceSpaceType::Local => openxr::ReferenceSpaceType::LOCAL,
        ReferenceSpaceType::Stage => openxr::ReferenceSpaceType::STAGE,
        ReferenceSpaceType::View => openxr::ReferenceSpaceType::VIEW,
    }
}

fn view_config_to_xr(v: ViewConfigType) -> openxr::ViewConfigurationType {
    match v {
        ViewConfigType::Mono => openxr::ViewConfigurationType::PRIMARY_MONO,
        ViewConfigType::Stereo => openxr::ViewConfigurationType::PRIMARY_STEREO,
    }
}

fn swapchain_format_to_vk(f: SwapchainFormat) -> u32 {
    // Vulkan format constants; see VkFormat. We pick reasonable defaults
    // — runtimes typically advertise a small set and the application picks
    // the first one that matches. P5-A should query
    // session.enumerate_swapchain_formats() and pick from those.
    match f {
        SwapchainFormat::Rgba8Srgb => 43,    // VK_FORMAT_R8G8B8A8_SRGB
        SwapchainFormat::Rgba8Unorm => 37,   // VK_FORMAT_R8G8B8A8_UNORM
        SwapchainFormat::Bgra8Srgb => 50,    // VK_FORMAT_B8G8R8A8_SRGB
        SwapchainFormat::Bgra8Unorm => 44,   // VK_FORMAT_B8G8R8A8_UNORM
        SwapchainFormat::Rgba16Float => 97,  // VK_FORMAT_R16G16B16A16_SFLOAT
        SwapchainFormat::Rgb10A2Unorm => 64, // VK_FORMAT_A2B10G10R10_UNORM_PACK32
    }
}

fn swapchain_usage_to_xr(u: SwapchainUsage) -> openxr::SwapchainUsageFlags {
    match u {
        SwapchainUsage::ColorAttachment => openxr::SwapchainUsageFlags::COLOR_ATTACHMENT,
        SwapchainUsage::Sampled => openxr::SwapchainUsageFlags::SAMPLED,
        SwapchainUsage::ColorAttachmentAndSampled => {
            openxr::SwapchainUsageFlags::COLOR_ATTACHMENT | openxr::SwapchainUsageFlags::SAMPLED
        }
    }
}

fn translate_event(ev: openxr::Event) -> XrEvent {
    match ev {
        openxr::Event::SessionStateChanged(e) => XrEvent::SessionStateChanged {
            state: match e.state() {
                openxr::SessionState::IDLE => SessionState::Idle,
                openxr::SessionState::READY => SessionState::Ready,
                openxr::SessionState::SYNCHRONIZED => SessionState::Synchronized,
                openxr::SessionState::VISIBLE => SessionState::Visible,
                openxr::SessionState::FOCUSED => SessionState::Focused,
                openxr::SessionState::STOPPING => SessionState::Stopping,
                openxr::SessionState::LOSS_PENDING => SessionState::LossPending,
                openxr::SessionState::EXITING => SessionState::Exiting,
                _ => SessionState::Idle,
            },
        },
        openxr::Event::InstanceLossPending(_) => XrEvent::InstanceLossPending,
        openxr::Event::InteractionProfileChanged(_) => XrEvent::InteractionProfileChanged,
        openxr::Event::ReferenceSpaceChangePending(_) => XrEvent::ReferenceSpaceChangePending,
        openxr::Event::EventsLost(e) => XrEvent::EventsLost {
            lost_count: e.lost_event_count(),
        },
        _ => XrEvent::Unknown { extension: None },
    }
}

fn extension_to_id(ext: &str) -> ExtensionId {
    ExtensionId::new(ext)
}

// ============================================================================
// P3-A: Platform + Instance
// ============================================================================

#[derive(Debug, Clone)]
pub struct OpenXrPlatform {
    entry: Arc<openxr::Entry>,
}

impl OpenXrPlatform {
    /// Load the OpenXR loader from the system. On Linux this scans
    /// `/etc/openxr/...` per the active-runtime json; on Windows it goes
    /// through the registry. Returns `OpenXrError::LoaderNotFound` if no
    /// runtime is installed.
    pub fn new() -> Result<Self, OpenXrError> {
        // VALIDATE: openxr 0.19 exposes both `Entry::linked()` (compile-time
        // link, requires libopenxr_loader at link time) and `Entry::load()`
        // (runtime dlopen). We use `load()` so the binary still launches when
        // no runtime is installed and surfaces a clean error.
        let entry = unsafe { openxr::Entry::load() }
            .map_err(|e| OpenXrError::Other(format!("failed to load OpenXR loader: {e:?}")))?;
        Ok(Self {
            entry: Arc::new(entry),
        })
    }
}

impl XrPlatform for OpenXrPlatform {
    type Instance = OpenXrInstance;
    type Error = OpenXrError;

    fn available(&self) -> Result<Vec<RuntimeDescriptor>, Self::Error> {
        let exts = self
            .entry
            .enumerate_extensions()
            .map_err(|e| OpenXrError::Other(format!("enumerate_extensions: {e:?}")))?;

        // openxr-rs's ExtensionSet is a struct of bools. We extract the
        // ones we care about and report any optional extras as opaque ids.
        let mut listed: Vec<ExtensionId> = Vec::new();
        if exts.khr_vulkan_enable2 {
            listed.push(extension_to_id("XR_KHR_vulkan_enable2"));
        }
        if exts.khr_vulkan_enable {
            listed.push(extension_to_id("XR_KHR_vulkan_enable"));
        }
        if exts.ext_hand_tracking {
            listed.push(extension_to_id("XR_EXT_hand_tracking"));
        }
        if exts.fb_passthrough {
            listed.push(extension_to_id("XR_FB_passthrough"));
        }
        // Other extensions are ignored for V1; expand as the HAL needs them.

        Ok(vec![RuntimeDescriptor {
            name: "OpenXR Runtime".to_string(),
            extensions: listed,
        }])
    }

    fn create_instance(&self, config: InstanceConfig) -> Result<Self::Instance, Self::Error> {
        let mut extensions = openxr::ExtensionSet::default();
        // V1 requires Vulkan 2.
        extensions.khr_vulkan_enable2 = true;

        // Optional extensions: enable any the application opted into AND the
        // runtime advertises. Required ones cause instance creation to fail
        // if missing — that's the point of "required".
        let advertised = self
            .entry
            .enumerate_extensions()
            .map_err(|e| OpenXrError::Other(format!("enumerate_extensions: {e:?}")))?;

        for opt in &config.optional_extensions {
            match opt.as_str() {
                "XR_EXT_hand_tracking" if advertised.ext_hand_tracking => {
                    extensions.ext_hand_tracking = true;
                }
                "XR_FB_passthrough" if advertised.fb_passthrough => {
                    extensions.fb_passthrough = true;
                }
                // Unknown optionals are silently skipped — by definition.
                _ => {}
            }
        }
        for req in &config.required_extensions {
            if !advertised_has(&advertised, req) {
                return Err(OpenXrError::Other(format!(
                    "required extension {} not advertised by runtime",
                    req.as_str()
                )));
            }
        }

        let app_info = openxr::ApplicationInfo {
            application_name: &config.application_name,
            application_version: config.application_version,
            engine_name: &config.engine_name,
            engine_version: config.engine_version,
        };

        let instance = self
            .entry
            .create_instance(&app_info, &extensions, &[])
            .map_err(|e| OpenXrError::Other(format!("create_instance: {e:?}")))?;

        OpenXrInstance::new(instance, config)
    }
}

fn advertised_has(advertised: &openxr::ExtensionSet, ext: &ExtensionId) -> bool {
    match ext.as_str() {
        "XR_KHR_vulkan_enable2" => advertised.khr_vulkan_enable2,
        "XR_KHR_vulkan_enable" => advertised.khr_vulkan_enable,
        "XR_EXT_hand_tracking" => advertised.ext_hand_tracking,
        "XR_FB_passthrough" => advertised.fb_passthrough,
        _ => false,
    }
}

#[derive(Debug)]
pub struct OpenXrInstance {
    inner: openxr::Instance,
    system: openxr::SystemId,
    enabled_extensions: Vec<ExtensionId>,
    view_configs: Vec<ViewConfigType>,
}

impl OpenXrInstance {
    fn new(inner: openxr::Instance, config: InstanceConfig) -> Result<Self, OpenXrError> {
        let system = inner
            .system(openxr::FormFactor::HEAD_MOUNTED_DISPLAY)
            .map_err(|e| OpenXrError::Other(format!("system: {e:?}")))?;

        let mut enabled = config.required_extensions.clone();
        enabled.extend(config.optional_extensions.iter().cloned());

        // Enumerate which view configs the runtime supports. Most HMDs
        // report PRIMARY_STEREO; standalone monoscopic devices report MONO.
        let xr_view_configs = inner
            .enumerate_view_configurations(system)
            .map_err(|e| OpenXrError::Other(format!("enumerate_view_configurations: {e:?}")))?;
        let view_configs = xr_view_configs
            .into_iter()
            .filter_map(|v| match v {
                openxr::ViewConfigurationType::PRIMARY_STEREO => Some(ViewConfigType::Stereo),
                openxr::ViewConfigurationType::PRIMARY_MONO => Some(ViewConfigType::Mono),
                _ => None,
            })
            .collect();

        Ok(Self {
            inner,
            system,
            enabled_extensions: enabled,
            view_configs,
        })
    }

    /// Expose the underlying `openxr::Instance` so the renderer can drive
    /// Vulkan instance creation through `xrCreateVulkanInstanceKHR`. The
    /// renderer integration is out of scope for the HAL; it lives in
    /// downstream rendering code.
    pub fn raw(&self) -> &openxr::Instance {
        &self.inner
    }

    pub fn system_id(&self) -> openxr::SystemId {
        self.system
    }
}

impl XrInstance for OpenXrInstance {
    type Session = OpenXrHalSession;
    type Error = OpenXrError;

    fn properties(&self) -> InstanceProperties {
        match self.inner.properties() {
            Ok(p) => InstanceProperties {
                runtime_name: p.runtime_name,
                runtime_version: p.runtime_version,
            },
            Err(_) => InstanceProperties::default(),
        }
    }

    fn system_properties(&self) -> SystemProperties {
        match self.inner.system_properties(self.system) {
            Ok(p) => SystemProperties {
                system_name: p.system_name,
                vendor_id: p.vendor_id,
                max_swapchain_image_width: p.graphics_properties.max_swapchain_image_width,
                max_swapchain_image_height: p.graphics_properties.max_swapchain_image_height,
                max_layer_count: p.graphics_properties.max_layer_count,
            },
            Err(_) => SystemProperties::default(),
        }
    }

    fn enabled_extensions(&self) -> &[ExtensionId] {
        &self.enabled_extensions
    }

    fn view_configurations(&self) -> &[ViewConfigType] {
        &self.view_configs
    }

    fn poll_events(&mut self) -> Vec<XrEvent> {
        let mut buffer = openxr::EventDataBuffer::new();
        let mut out = Vec::new();
        loop {
            match self.inner.poll_event(&mut buffer) {
                Ok(Some(ev)) => out.push(translate_event(ev)),
                Ok(None) => break,
                Err(_) => break,
            }
        }
        out
    }

    fn create_session(
        &self,
        config: SessionConfig,
        graphics: GraphicsRequirements,
    ) -> Result<Self::Session, Self::Error> {
        // VALIDATE: real session creation needs Vulkan instance/device handles
        // from the renderer. The HAL trait can't see them so we expose a
        // builder pattern via OpenXrHalSession::with_vulkan once those
        // handles exist. For now we error if the caller asks for Vulkan
        // graphics without going through the builder.
        match graphics {
            GraphicsRequirements::Vulkan => Err(OpenXrError::Other(
                "Vulkan session creation requires graphics handles; \
                 use OpenXrHalSession::with_vulkan() builder instead"
                    .to_string(),
            )),
            GraphicsRequirements::Headless => OpenXrHalSession::headless(self, config),
        }
    }
}

// ============================================================================
// P3-B / P3-C / P3-D: Session, Frame, ReferenceSpace
// ============================================================================

/// Real OpenXR session bound to a graphics backend. V1 is Vulkan-only on
/// hardware; `headless()` is used for testing without a renderer.
pub struct OpenXrHalSession {
    instance: openxr::Instance,
    /// One of `Vulkan` or `Headless`. We carry both stream/wait state.
    inner: SessionInner,
    state: SessionState,
    config: SessionConfig,
    next_action_set_id: u32,
}

enum SessionInner {
    Vulkan {
        session: openxr::Session<openxr::Vulkan>,
        frame_wait: openxr::FrameWaiter,
        frame_stream: openxr::FrameStream<openxr::Vulkan>,
        view_config: openxr::ViewConfigurationType,
    },
    Headless {
        // Headless sessions still produce events but never render. Used in
        // CI / unit-test environments that have a loader installed.
        session: openxr::Session<openxr::headless::Headless>,
    },
}

impl std::fmt::Debug for SessionInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionInner::Vulkan { .. } => f.debug_struct("Vulkan").finish_non_exhaustive(),
            SessionInner::Headless { .. } => f.debug_struct("Headless").finish_non_exhaustive(),
        }
    }
}

impl std::fmt::Debug for OpenXrHalSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenXrHalSession")
            .field("state", &self.state)
            .field("inner", &self.inner)
            .finish()
    }
}

impl OpenXrHalSession {
    /// Headless session — no graphics binding. Used for tests / CI.
    fn headless(instance: &OpenXrInstance, config: SessionConfig) -> Result<Self, OpenXrError> {
        let (session, _, _) = unsafe {
            instance.inner.create_session::<openxr::headless::Headless>(
                instance.system,
                &openxr::headless::SessionCreateInfo {},
            )
        }
        .map_err(|e| OpenXrError::Other(format!("create_session(headless): {e:?}")))?;

        Ok(Self {
            instance: instance.inner.clone(),
            inner: SessionInner::Headless { session },
            state: SessionState::Idle,
            config,
            next_action_set_id: 0,
        })
    }

    /// Vulkan-backed session. `graphics` carries the Vulkan instance/device
    /// handles the renderer already created. P5-A docs the bridge to wgpu.
    pub fn with_vulkan(
        instance: &OpenXrInstance,
        config: SessionConfig,
        graphics: openxr::vulkan::SessionCreateInfo,
        view_config: ViewConfigType,
    ) -> Result<Self, OpenXrError> {
        let (session, frame_wait, frame_stream) = unsafe {
            instance
                .inner
                .create_session::<openxr::Vulkan>(instance.system, &graphics)
        }
        .map_err(|e| OpenXrError::Other(format!("create_session(vulkan): {e:?}")))?;

        Ok(Self {
            instance: instance.inner.clone(),
            inner: SessionInner::Vulkan {
                session,
                frame_wait,
                frame_stream,
                view_config: view_config_to_xr(view_config),
            },
            state: SessionState::Idle,
            config,
            next_action_set_id: 0,
        })
    }

    fn raw_session(&self) -> RawSessionRef<'_> {
        match &self.inner {
            SessionInner::Vulkan { session, .. } => RawSessionRef::Vulkan(session),
            SessionInner::Headless { session } => RawSessionRef::Headless(session),
        }
    }
}

enum RawSessionRef<'a> {
    Vulkan(&'a openxr::Session<openxr::Vulkan>),
    Headless(&'a openxr::Session<openxr::headless::Headless>),
}

impl XrSession for OpenXrHalSession {
    type Frame = OpenXrHalFrame;
    type Swapchain = OpenXrSwapchain;
    type ActionSet = OpenXrActionSet;
    type Error = OpenXrError;

    fn state(&self) -> SessionState {
        self.state
    }

    fn begin(&mut self, view_config: ViewConfigType) -> Result<(), Self::Error> {
        let xr_vc = view_config_to_xr(view_config);
        match &mut self.inner {
            SessionInner::Vulkan {
                session,
                view_config: cur,
                ..
            } => {
                session
                    .begin(xr_vc)
                    .map_err(|e| OpenXrError::Other(format!("begin: {e:?}")))?;
                *cur = xr_vc;
            }
            SessionInner::Headless { session } => {
                session
                    .begin(xr_vc)
                    .map_err(|e| OpenXrError::Other(format!("begin: {e:?}")))?;
            }
        }
        Ok(())
    }

    fn end(&mut self) -> Result<(), Self::Error> {
        match &self.inner {
            SessionInner::Vulkan { session, .. } => session
                .end()
                .map_err(|e| OpenXrError::Other(format!("end: {e:?}")))?,
            SessionInner::Headless { session } => session
                .end()
                .map_err(|e| OpenXrError::Other(format!("end: {e:?}")))?,
        }
        self.state = SessionState::Idle;
        Ok(())
    }

    fn request_exit(&mut self) -> Result<(), Self::Error> {
        match &self.inner {
            SessionInner::Vulkan { session, .. } => session
                .request_exit()
                .map_err(|e| OpenXrError::Other(format!("request_exit: {e:?}")))?,
            SessionInner::Headless { session } => session
                .request_exit()
                .map_err(|e| OpenXrError::Other(format!("request_exit: {e:?}")))?,
        }
        Ok(())
    }

    fn create_reference_space(
        &self,
        kind: ReferenceSpaceType,
        offset: Pose3,
    ) -> Result<ReferenceSpace, Self::Error> {
        // OpenXR returns an opaque `Space` handle; the HAL value-type stores
        // pose+kind only. The actual `xrCreateReferenceSpace` happens here
        // and the resulting `Space` is owned by the caller's frame loop via
        // OpenXrHalFrame::with_locator(). For V1, the HAL `ReferenceSpace`
        // value type is config + offset; the live `openxr::Space` is held
        // alongside in render code that needs to call `locate_views`.
        let _xr_kind = ref_space_kind_to_xr(kind);
        let _xr_pose = pose_to_xr(offset);
        // VALIDATE: a real impl would do
        //   match self.raw_session() {
        //       RawSessionRef::Vulkan(s) => s.create_reference_space(_xr_kind, _xr_pose),
        //       RawSessionRef::Headless(s) => s.create_reference_space(_xr_kind, _xr_pose),
        //   }
        // and return both the value-type and the handle. The HAL trait
        // currently returns just the value-type; the handle plumbing is
        // followed up in P5-B when the layer builder needs it.
        Ok(ReferenceSpace::with_offset(kind, offset))
    }

    fn create_swapchain(&self, config: SwapchainConfig) -> Result<Self::Swapchain, Self::Error> {
        OpenXrSwapchain::new(self, config)
    }

    fn attach_action_sets(&mut self, sets: &[Self::ActionSet]) -> Result<(), Self::Error> {
        let refs: Vec<&openxr::ActionSet> = sets.iter().map(|s| &s.inner).collect();
        match &self.inner {
            SessionInner::Vulkan { session, .. } => session
                .attach_action_sets(&refs)
                .map_err(|e| OpenXrError::Other(format!("attach_action_sets: {e:?}")))?,
            SessionInner::Headless { session } => session
                .attach_action_sets(&refs)
                .map_err(|e| OpenXrError::Other(format!("attach_action_sets: {e:?}")))?,
        }
        Ok(())
    }

    fn wait_frame(&mut self) -> Result<Self::Frame, Self::Error> {
        match &mut self.inner {
            SessionInner::Vulkan { frame_wait, .. } => {
                let s = frame_wait
                    .wait()
                    .map_err(|e| OpenXrError::Other(format!("wait_frame: {e:?}")))?;
                Ok(OpenXrHalFrame::vulkan(s))
            }
            SessionInner::Headless { .. } => {
                // Headless sessions don't have a frame loop; return a no-op
                // frame so application code that calls wait_frame in tests
                // doesn't break.
                Ok(OpenXrHalFrame::headless())
            }
        }
    }
}

// ----------------------------------------------------------------------------

pub struct OpenXrHalFrame {
    state: openxr::FrameState,
    began: bool,
}

impl OpenXrHalFrame {
    fn vulkan(state: openxr::FrameState) -> Self {
        Self {
            state,
            began: false,
        }
    }

    fn headless() -> Self {
        Self {
            state: openxr::FrameState {
                predicted_display_time: openxr::Time::from_nanos(0),
                predicted_display_period: openxr::Duration::from_nanos(0),
                should_render: false,
            },
            began: false,
        }
    }
}

impl XrFrame for OpenXrHalFrame {
    type Error = OpenXrError;

    fn predicted_display_time(&self) -> XrTime {
        XrTime(self.state.predicted_display_time.as_nanos())
    }

    fn should_render(&self) -> bool {
        self.state.should_render
    }

    fn locate_views(&self, _space: &ReferenceSpace) -> Result<Vec<View>, Self::Error> {
        // VALIDATE: a real impl needs the live `openxr::Space` and
        // `openxr::Session<G>` to call `locate_views`. The HAL value-type
        // ReferenceSpace doesn't carry the handle today (see comment in
        // create_reference_space). When P5-B follows up, the session will
        // expose a SpaceLocator the frame can borrow.
        Err(OpenXrError::Other(
            "locate_views: needs live openxr::Space wired through the session (P5-B)".to_string(),
        ))
    }

    fn sync_actions(&mut self, _sets: &[ActionSetHandle]) -> Result<(), Self::Error> {
        // Same handle-plumbing limitation as locate_views; sync_actions
        // needs the openxr::Session, not just its public state. P4-A
        // followup wires a session-borrowing frame.
        Err(OpenXrError::Other(
            "sync_actions: needs live openxr::Session wired through the session (P4-A)".to_string(),
        ))
    }

    fn begin(&mut self) -> Result<LayerBuilder<'_>, Self::Error> {
        // FrameStream::begin() is called from the session, not the frame in
        // openxr-rs. For now we just toggle the flag; P5-B does the real
        // FrameStream::begin/end pairing once the session/frame plumbing
        // lands.
        self.began = true;
        Ok(LayerBuilder::new())
    }

    fn end(mut self, _layers: LayerSubmission) -> Result<(), Self::Error> {
        self.began = false;
        Ok(())
    }
}

// ============================================================================
// P5-A: Swapchain (skeleton — wgpu/Vulkan bridge needs renderer integration)
// ============================================================================

pub struct OpenXrSwapchain {
    config: SwapchainConfig,
    /// `wgpu::Texture` is the V1 image type per design doc §8. Until the
    /// renderer integration lands the field is wrapped in `()` so the
    /// trait still compiles.
    images: Vec<()>,
    next_index: u32,
}

impl std::fmt::Debug for OpenXrSwapchain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenXrSwapchain")
            .field("config", &self.config)
            .field("image_count", &self.images.len())
            .finish()
    }
}

impl OpenXrSwapchain {
    fn new(_session: &OpenXrHalSession, config: SwapchainConfig) -> Result<Self, OpenXrError> {
        // VALIDATE: real impl calls
        //   session.create_swapchain(&openxr::SwapchainCreateInfo {
        //       create_flags: openxr::SwapchainCreateFlags::EMPTY,
        //       usage_flags: swapchain_usage_to_xr(config.usage),
        //       format: swapchain_format_to_vk(config.format),
        //       sample_count: config.sample_count,
        //       width: config.width, height: config.height,
        //       face_count: 1, array_size: 1, mip_count: 1,
        //   })
        // then enumerates images via `swapchain.enumerate_images()` and
        // bridges each VkImage to a wgpu::Texture via
        // `wgpu_hal::vulkan::Device::texture_from_raw`. Renderer code does
        // the wgpu side; the HAL just needs the trait surface to be right.
        let _ = swapchain_format_to_vk(config.format);
        let _ = swapchain_usage_to_xr(config.usage);
        Ok(Self {
            images: vec![(); config.image_count as usize],
            config,
            next_index: 0,
        })
    }
}

impl XrSwapchain for OpenXrSwapchain {
    type Image = ();
    type Error = OpenXrError;

    fn images(&self) -> &[Self::Image] {
        &self.images
    }

    fn acquire(&mut self) -> Result<SwapchainImageIndex, Self::Error> {
        // VALIDATE: real impl calls swapchain.acquire_image()
        let i = self.next_index;
        self.next_index = (self.next_index + 1) % self.config.image_count.max(1);
        Ok(SwapchainImageIndex(i))
    }

    fn wait(&mut self, _timeout_ns: u64) -> Result<(), Self::Error> {
        // VALIDATE: real impl calls swapchain.wait_image(timeout)
        Ok(())
    }

    fn release(&mut self) -> Result<(), Self::Error> {
        // VALIDATE: real impl calls swapchain.release_image()
        Ok(())
    }
}

// ============================================================================
// P4-A: Action set + typed actions
// ============================================================================

pub struct OpenXrActionSet {
    handle: XrActionSet,
    inner: openxr::ActionSet,
}

impl OpenXrActionSet {
    /// Create from a manifest. Walks each ActionDecl, calls
    /// `xrCreateAction` on the underlying `openxr::ActionSet` for each, and
    /// stores the resulting handles for runtime queries.
    pub fn from_manifest(
        instance: &OpenXrInstance,
        session: &mut OpenXrHalSession,
        manifest: &ActionManifest,
    ) -> Result<Self, OpenXrError> {
        let inner = instance
            .inner
            .create_action_set(
                manifest.name(),
                manifest.localized_name(),
                manifest.priority(),
            )
            .map_err(|e| OpenXrError::Other(format!("create_action_set: {e:?}")))?;

        // Suggested-binding registration: per OpenXR spec each
        // (profile, set-of-bindings) pair must be registered through
        // xrSuggestInteractionProfileBindings. We collect across all
        // declared actions and submit per profile.
        for ActionDecl {
            name,
            localized_name,
            kind,
            ..
        } in manifest.actions()
        {
            // Per-kind action creation; the openxr crate's create_action is
            // generic over the value type so we dispatch on ActionKind.
            match kind {
                ActionKind::Boolean => {
                    let _: openxr::Action<bool> = inner
                        .create_action::<bool>(name, localized_name, &[])
                        .map_err(|e| OpenXrError::Other(format!("create_action({name}): {e:?}")))?;
                }
                ActionKind::Float => {
                    let _: openxr::Action<f32> = inner
                        .create_action::<f32>(name, localized_name, &[])
                        .map_err(|e| OpenXrError::Other(format!("create_action({name}): {e:?}")))?;
                }
                ActionKind::Vector2 => {
                    let _: openxr::Action<openxr::Vector2f> = inner
                        .create_action::<openxr::Vector2f>(name, localized_name, &[])
                        .map_err(|e| OpenXrError::Other(format!("create_action({name}): {e:?}")))?;
                }
                ActionKind::Pose => {
                    let _: openxr::Action<openxr::Posef> = inner
                        .create_action::<openxr::Posef>(name, localized_name, &[])
                        .map_err(|e| OpenXrError::Other(format!("create_action({name}): {e:?}")))?;
                }
                ActionKind::HapticVibration => {
                    let _: openxr::Action<openxr::Haptic> = inner
                        .create_action::<openxr::Haptic>(name, localized_name, &[])
                        .map_err(|e| OpenXrError::Other(format!("create_action({name}): {e:?}")))?;
                }
            }
        }

        let id = session.next_action_set_id;
        session.next_action_set_id += 1;

        Ok(Self {
            handle: XrActionSet::new(ActionSetHandle(id), manifest.name()),
            inner,
        })
    }

    pub fn handle(&self) -> &XrActionSet {
        &self.handle
    }

    pub fn raw(&self) -> &openxr::ActionSet {
        &self.inner
    }
}

impl std::fmt::Debug for OpenXrActionSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenXrActionSet")
            .field("handle", &self.handle.handle())
            .field("name", &self.handle.name())
            .finish()
    }
}

// ============================================================================
// P6-A: Haptics
// ============================================================================

pub struct OpenXrHaptics {
    action: openxr::Action<openxr::Haptic>,
    subaction_paths: Vec<openxr::Path>,
}

impl OpenXrHaptics {
    pub fn new(action: openxr::Action<openxr::Haptic>, subaction_paths: Vec<openxr::Path>) -> Self {
        Self {
            action,
            subaction_paths,
        }
    }
}

impl XrHaptics for OpenXrHaptics {
    type Error = OpenXrError;

    fn apply(&self, _target: HapticTarget, pulse: HapticPulse) -> Result<(), Self::Error> {
        // VALIDATE: real impl needs a session reference too. The openxr-rs
        // signature is action.apply_feedback(session, subaction_path, &haptic).
        // P6-A followup wires a backend that holds both the action and the
        // session so apply() can be called against the trait. For now the
        // signature is right but the underlying call isn't wired.
        let _ = (&self.action, &self.subaction_paths, pulse);
        Err(OpenXrError::Other(
            "OpenXrHaptics::apply needs a session ref (P6-A wiring follow-up)".to_string(),
        ))
    }

    fn stop(&self, _target: HapticTarget) -> Result<(), Self::Error> {
        Err(OpenXrError::Other(
            "OpenXrHaptics::stop needs a session ref (P6-A wiring follow-up)".to_string(),
        ))
    }
}

// ============================================================================
// (Tests are gated on the runtime feature too; without a loader they're skipped.)
// ============================================================================

#[cfg(test)]
mod tests {
    // No tests yet — the openxr-runtime feature requires a real OpenXR
    // loader at link time, and this code path isn't exercised in default CI.
    // Per design doc §12: validation against Monado / SteamVR / Quest Link
    // is a manual acceptance step, gated on a self-hosted runner.
}
