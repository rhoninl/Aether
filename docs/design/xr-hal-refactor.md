# XR HAL Refactor — OpenXR Compliance

Status: **Draft** · Owner: Aether XR
Target reviewers: runtime, rendering, input, vr-emulator, openxr leads
Companion docs: [`openxr-vr-integration.md`](./openxr-vr-integration.md),
[`vr-input-interaction-implementation.md`](./vr-input-interaction-implementation.md),
[`vr-emulator-pc-development.md`](./vr-emulator-pc-development.md)

---

## 1. Context & motivation

Aether already ships two XR-flavoured crates — `crates/aether-input` and `crates/aether-openxr` —
plus a desktop emulator (`crates/aether-vr-emulator`) and a debug overlay
(`crates/aether-vr-overlay`). Together they look like an OpenXR integration, but the integration
is incomplete in ways that block real headset support:

- **Neither crate links the `openxr` crate.** Every type that names an OpenXR concept
  (`XrSession`, `XrInstance`, `SwapchainManager`, `TrackingPipeline`, `HapticDispatcher`, …) is
  a hand-rolled stub. There is no path from runtime selection through to a real `xrCreateInstance`
  call.
- **The session state machine is duplicated.** `aether-input::openxr_session::SessionManager`
  and `aether-openxr::session::XrSession` are independent implementations of the same OpenXR
  state graph (`Idle → Ready → Synchronized → Visible → Focused → Stopping → LossPending →
  Exiting`). Neither is wired to anything.
- **The frame loop is a placeholder.** `aether-openxr::frame_loop::{wait_frame, begin_frame,
  end_frame}` return constants (`predicted_display_time_ns: 0`) and never call the real
  `xrWaitFrame / xrBeginFrame / xrEndFrame` triplet. There is no event pump (`xrPollEvent`),
  so session state can only be advanced by manual test calls.
- **Action-based input is not modelled.** `aether-openxr::input_actions::XrInputActions` is a
  stub; there is no `XrActionSet`, no `xrSuggestInteractionProfileBindings`, no
  `xrSyncActions`. The current `aether-input::mapping::ActionMap` describes button bindings but
  has no path into an OpenXR action set.
- **Swapchain lifecycle is not wired to wgpu.** `aether-input::openxr_swapchain::SwapchainManager`
  defines acquire/release/wait state, but there is no integration with `wgpu::Device` /
  `wgpu::Texture`, no per-eye view config, and no composition-layer submission at frame end.
- **Extensions are hardcoded as booleans.** `SessionConfig::enable_hand_tracking` and
  `enable_haptics` are flags rather than the result of `xrEnumerateInstanceExtensionProperties`
  + `xrCreateInstance` extension activation. Hand tracking is never actually surfaced.
- **Backends do not share an interface.** `RuntimeAdapter` is the only HAL-shaped trait
  (`adapter.rs:20-25`). It exposes `poll_frame() -> InputFrame` and `apply_locomotion_profile`
  and nothing else — no session lifecycle, no view query, no swapchain. The emulator does not
  even implement it; it produces `TrackingSnapshot` ad hoc.

"OpenXR compliance" in this document means:

1. A clean **HAL** (Hardware Abstraction Layer) trait surface that mirrors the OpenXR object
   model (Instance, System, Session, Frame, ViewConfig, ReferenceSpace, Swapchain, ActionSet,
   Action, Haptics, optional Anchor) but is **runtime-agnostic** — pluggable with the real
   OpenXR backend or the desktop emulator.
2. A **real OpenXR backend** that links `openxr` (the `OpenXR-rs` Rust binding), implements
   every HAL trait, and follows the spec's lifecycle: instance creation with extension
   negotiation, system selection, session lifecycle driven by `xrPollEvent`, frame loop using
   `xrWaitFrame / xrBeginFrame / xrEndFrame`, action-set sync per frame, swapchain acquire /
   release per eye, composition-layer submission.
3. A **first-class emulator backend** that satisfies the same HAL — so app and runtime code
   that targets the HAL works on developer machines without a headset.

## 2. Goals / non-goals

### Goals

- Define `aether-xr-hal` as the single shared XR contract for the workspace.
- Make `aether-openxr` a real, conformant OpenXR backend.
- Make `aether-vr-emulator` a HAL backend (replacing its ad-hoc `TrackingSnapshot` synthesis).
- Eliminate the duplicate session state machines and stub types.
- Move shared value types (`Pose3`, `TrackingSnapshot`, `ControllerState`, `HandJointSet`,
  `ReferenceSpace`, …) out of `aether-input` and into `aether-xr-hal`, so non-input crates
  don't have to depend on `aether-input` to talk about poses.
- Preserve existing `aether-input` interaction primitives (deadzone, mapping, locomotion,
  comfort, gesture graph, processing). Their public API should keep working through the
  refactor.
- Keep CI green on all platforms — the `openxr` runtime is not always available, so the
  backend must be feature-gated.

### Non-goals

- Replacing wgpu or the renderer.
- Designing new input primitives — `aether-input::actions`, `mapping`, `locomotion`,
  `processing`, `graph`, `deadzone`, `movement`, `capabilities`, `haptics` keep their semantics.
- Adding new headset-only features beyond what OpenXR exposes (passthrough mesh, body tracking
  beyond hands, scene understanding) — deliberately deferred.
- Changing the `aether-vr-overlay` debug UI behaviour. It will rebuild against the new types
  but its rendering does not change.
- Designing a portable rendering abstraction over Vulkan / D3D / Metal swapchains — for V1 the
  HAL exposes wgpu textures and the OpenXR backend bridges via the `wgpu-hal` interop hooks
  the `openxr` crate already supports (Vulkan-only; D3D12 / Metal are follow-up work).

## 3. Current state audit

Verified by reading `crates/aether-input/src/{lib,adapter,openxr_session,openxr_tracking}.rs`,
`crates/aether-openxr/{Cargo.toml,src/{lib,frame_loop}.rs}`, and the workspace `Cargo.toml`.

### 3.1 Crates and ownership

| Crate | Path | Role today | Has stubs? |
|-------|------|------------|------------|
| `aether-input` | `crates/aether-input/` | Interaction primitives **+** OpenXR data-model stubs | Yes — `openxr_*.rs` |
| `aether-openxr` | `crates/aether-openxr/` | Intended OpenXR runtime integration | Yes — entire crate |
| `aether-vr-emulator` | `crates/aether-vr-emulator/` | Desktop window + stereo display + ad-hoc tracking | Partial |
| `aether-vr-overlay` | `crates/aether-vr-overlay/` | Debug overlay consumer | No |

### 3.2 The `aether-input` OpenXR modules (largest stubs)

- `openxr_session.rs` (~870 lines) — `SessionState` enum, `SessionManager`, `ReferenceSpace`,
  `SessionConfig`. **Used by:** unit tests only.
- `openxr_tracking.rs` (~700 lines) — `TrackingSnapshot`, `ControllerState`, `HandJointSet`,
  `TrackingPipeline`, `TrackingConfidence`. **Used by:** `aether-vr-emulator`,
  `aether-vr-overlay`, `aether-openxr::input_actions`, examples.
- `openxr_haptics.rs` (~630 lines) — `HapticDispatcher`, `HapticAction`, cooldowns. **Used by:**
  unit tests only.
- `openxr_swapchain.rs` (~630 lines) — `SwapchainConfig`, `SwapchainManager`, image lifecycle.
  **Used by:** unit tests only.
- `openxr.rs` (~50 lines) — `OpenXrAdapter`, a `RuntimeAdapter` impl that returns empty
  `InputFrame`s. **Used by:** examples that just want a backend that compiles.

### 3.3 The `aether-openxr` modules

- `instance.rs` — `XrInstance`, `InstanceConfig` (no `openxr` crate calls).
- `session.rs` — `XrSession` with a state enum that **shadows**
  `aether-input::openxr_session::SessionState`.
- `frame_loop.rs` — `wait_frame` / `begin_frame` / `end_frame` returning constants.
- `input_actions.rs` — `XrInputActions` that converts to `TrackingSnapshot` from a stub.
- `swapchain.rs` — `XrSwapchain` stub.
- `error.rs` — `OpenXrError` enum.

`Cargo.toml` for `aether-openxr` depends on **only** `aether-input` and `log`. No `openxr`.

### 3.4 The single existing HAL trait

```rust
// crates/aether-input/src/adapter.rs:20
pub trait RuntimeAdapter {
    fn backend(&self) -> InputBackend;
    fn advertised_capabilities(&self) -> InputFrameHint;
    fn poll_frame(&mut self) -> Result<InputFrame, InputFrameError>;
    fn apply_locomotion_profile(&mut self, profile: &LocomotionProfile);
}
```

Implementors: `OpenXrAdapter` (stub), `DesktopAdapter`, `TestAdapter` (in
`runtime.rs#cfg(test)`). The trait is sufficient for "give me an `InputFrame`" but cannot
express anything XR-shaped — no session, no frame timing, no view config, no swapchain.

### 3.5 Workspace consumers

`grep "use aether_input::"` plus `Cargo.toml` deps:

- `aether-vr-emulator` — imports `Pose3, TrackingSnapshot, TrackingConfidence, Hand`.
- `aether-vr-overlay` — imports `Pose3, TrackingSnapshot, ControllerState, Hand, HandJointSet`.
- `aether-openxr` — imports `Pose3, TrackingSnapshot, ControllerState, Hand, TrackingConfidence`.
- `examples/single-world-demo` — imports the runtime and a backend adapter.
- `aether-world-runtime`, `aether-multiplayer`, `aether-creator-studio`, etc. only depend on
  `aether-input` for the input/locomotion side, not for OpenXR types.

### 3.6 Compliance gaps summary

1. No real `openxr` crate dependency anywhere.
2. No `xrPollEvent` event loop → session state cannot advance.
3. No real `xrWaitFrame / xrBeginFrame / xrEndFrame` → no frame timing or composition.
4. No action sets, no `xrSuggestInteractionProfileBindings`, no `xrSyncActions`.
5. View config (`xrEnumerateViewConfigurations`, `xrLocateViews`) absent — stereo layout is
   implicit.
6. Reference spaces defined but not queried (no `xrLocateSpace`).
7. Swapchain not bridged to wgpu / Vulkan; no composition-layer submission at end of frame.
8. Extensions (hand tracking, eye tracking, passthrough) modelled as booleans, not as
   `xrEnumerateInstanceExtensionProperties` + activation.
9. Haptic output never reaches a runtime call (`xrApplyHapticFeedback`).
10. Two duplicate `SessionState` types → ambiguous source of truth.

## 4. Target architecture

```
                   ┌──────────────────────────────────────────────┐
                   │                aether-xr-hal                 │
                   │  (traits + value types, no backend deps)     │
                   │                                              │
                   │  XrPlatform / XrInstance / XrSystem          │
                   │  XrSession / XrFrame / XrViewConfig          │
                   │  XrReferenceSpace / XrSwapchain              │
                   │  XrCompositionLayer                          │
                   │  XrActionSet / XrAction<T>                   │
                   │  XrHaptics                                   │
                   │  XrAnchor (optional, behind feature)         │
                   │  Pose3, TrackingSnapshot, …                  │
                   └──────────┬──────────────────────────┬────────┘
                              │ implements              │ implements
                              ▼                          ▼
       ┌──────────────────────────────┐   ┌──────────────────────────────┐
       │       aether-openxr          │   │      aether-vr-emulator      │
       │  (real OpenXR backend)       │   │  (desktop fake backend)      │
       │  depends on `openxr` crate   │   │  desktop window + KB/mouse   │
       │  XrInstance: xrCreateInstance│   │  XrInstance: in-process stub │
       │  XrFrame:    xrWaitFrame…    │   │  XrFrame:    fixed cadence   │
       │  XrSwapchain: wgpu+Vulkan    │   │  XrSwapchain: wgpu textures  │
       │  XrAction:   xrSyncActions   │   │  XrAction:   KB/mouse synth  │
       └──────────┬───────────────────┘   └──────────┬───────────────────┘
                  │                                   │
                  └──────────────┬────────────────────┘
                                 │ used by
                                 ▼
              ┌──────────────────────────────────────┐
              │            aether-input              │
              │  (interaction primitives only)       │
              │                                      │
              │  actions, mapping, locomotion,       │
              │  comfort, gesture graph, processing, │
              │  deadzone, movement, capabilities,   │
              │  haptics, desktop input source       │
              │                                      │
              │  XrPlatform-driven InputRuntime      │
              └──────────────────────────────────────┘
```

### 4.1 New crate: `aether-xr-hal`

- Pure `#![no_std]`-friendly types and traits (likely with `std` enabled by default to keep
  `String` / `Vec` ergonomic, but no I/O dependencies).
- Optional features: `serde`, `wgpu` (for the swapchain image type), `bytemuck` (for pose math).
- No dep on `openxr`, no dep on `aether-input` (the dependency arrow reverses: `aether-input`
  depends on `aether-xr-hal`).

### 4.2 `aether-openxr` becomes the real backend

- Adds `openxr = "<pinned>"` to `Cargo.toml` (see §11 for version pinning open question).
- Implements every HAL trait. No more stub `XrSession` / `XrInstance` types — those become
  newtypes around `openxr::Session<G>` / `openxr::Instance` etc.
- Owns C-API error translation in `error.rs`.
- Owns `xrPollEvent` loop and surfaces events as `XrEvent` enum on the HAL.

### 4.3 `aether-vr-emulator` becomes a HAL backend

- Replaces ad-hoc `TrackingSnapshot` synthesis with a `XrPlatform` impl that returns an emulator
  `XrInstance` → `XrSession` → `XrFrame` chain.
- Keyboard + mouse + gamepad map to synthesized `XrAction` state.
- Renders into wgpu textures the same way the real backend does, but composites them to a
  desktop window instead of submitting via `xrEndFrame`.

### 4.4 `aether-input` becomes platform-agnostic

- Drops `openxr_session.rs`, `openxr_tracking.rs`, `openxr_swapchain.rs`, `openxr_haptics.rs`,
  `openxr.rs`. The useful types migrate into `aether-xr-hal` (see §10 phases).
- `RuntimeAdapter` either retires entirely or becomes a thin façade over `XrPlatform` for a
  transition window. Final state: deleted.
- `DesktopAdapter` is rebuilt as a keyboard-to-action shim that runs on top of the emulator
  backend, *or* deleted in favour of just using the emulator backend directly. Final decision
  defers to phase P8.
- `aether-input::capabilities` learns to express extensions discovered by the HAL.

## 5. Trait-by-trait API sketch

These signatures are illustrative — final names settle during implementation. Every trait's
methods enumerate the OpenXR call(s) it abstracts so reviewers can sanity-check coverage.

### 5.1 `XrPlatform` — the entry point

```rust
pub trait XrPlatform {
    type Instance: XrInstance;
    type Error: std::error::Error + Send + Sync + 'static;

    /// Enumerate available runtimes / configurations. For OpenXR backend this enumerates
    /// instance extensions; for emulator it returns a single static descriptor.
    fn available(&self) -> Result<Vec<RuntimeDescriptor>, Self::Error>;

    /// Create an instance. Maps to xrCreateInstance + xrGetSystem.
    fn create_instance(
        &self,
        config: InstanceConfig,
    ) -> Result<Self::Instance, Self::Error>;
}
```

### 5.2 `XrInstance` — owns extensions, system, and session creation

```rust
pub trait XrInstance {
    type Session: XrSession;
    type Error: std::error::Error + Send + Sync + 'static;

    fn properties(&self) -> InstanceProperties;          // xrGetInstanceProperties
    fn system_properties(&self) -> SystemProperties;     // xrGetSystemProperties
    fn enabled_extensions(&self) -> &[ExtensionId];
    fn view_configurations(&self) -> &[ViewConfigType];  // xrEnumerateViewConfigurations

    /// Pumps the runtime event queue. Returns events since last poll.
    fn poll_events(&mut self) -> Vec<XrEvent>;           // xrPollEvent

    fn create_session(
        &self,
        config: SessionConfig,
        graphics: GraphicsRequirements,
    ) -> Result<Self::Session, Self::Error>;             // xrCreateSession
}
```

### 5.3 `XrSession` — owns lifecycle, reference spaces, action attachment

```rust
pub trait XrSession {
    type Frame: XrFrame;
    type Swapchain: XrSwapchain;
    type ActionSet: XrActionSet;
    type Error: std::error::Error + Send + Sync + 'static;

    fn state(&self) -> SessionState;
    fn begin(&mut self, view_config: ViewConfigType) -> Result<(), Self::Error>;  // xrBeginSession
    fn end(&mut self) -> Result<(), Self::Error>;                                  // xrEndSession
    fn request_exit(&mut self) -> Result<(), Self::Error>;                         // xrRequestExitSession

    fn create_reference_space(&self, kind: ReferenceSpaceType, offset: Pose3)
        -> Result<ReferenceSpace, Self::Error>;                                    // xrCreateReferenceSpace
    fn create_swapchain(&self, config: SwapchainConfig) -> Result<Self::Swapchain, Self::Error>;
    fn attach_action_sets(&mut self, sets: &[Self::ActionSet]) -> Result<(), Self::Error>;
                                                                                    // xrAttachSessionActionSets
    fn wait_frame(&mut self) -> Result<Self::Frame, Self::Error>;                   // xrWaitFrame
}
```

### 5.4 `XrFrame` — RAII wrapper around `xrBeginFrame` / `xrEndFrame`

```rust
pub trait XrFrame {
    type Error: std::error::Error + Send + Sync + 'static;

    fn predicted_display_time(&self) -> XrTime;
    fn should_render(&self) -> bool;

    fn locate_views(&self, space: &ReferenceSpace) -> Result<Vec<View>, Self::Error>;
                                                                            // xrLocateViews
    fn sync_actions(&mut self, sets: &[ActionSetHandle]) -> Result<(), Self::Error>;
                                                                            // xrSyncActions

    /// Begin recording layers; returns a builder to add composition layers.
    fn begin(&mut self) -> Result<LayerBuilder<'_>, Self::Error>;           // xrBeginFrame

    /// Submit recorded layers and end the frame.
    fn end(self, layers: LayerSubmission) -> Result<(), Self::Error>;       // xrEndFrame
}
```

### 5.5 `XrSwapchain` — wgpu-bridged image rotation

```rust
pub trait XrSwapchain {
    type Image; // wgpu::Texture for both backends in V1
    type Error: std::error::Error + Send + Sync + 'static;

    fn images(&self) -> &[Self::Image];                          // xrEnumerateSwapchainImages
    fn acquire(&mut self) -> Result<SwapchainImageIndex, Self::Error>;  // xrAcquireSwapchainImage
    fn wait(&mut self, timeout_ns: u64) -> Result<(), Self::Error>;     // xrWaitSwapchainImage
    fn release(&mut self) -> Result<(), Self::Error>;                   // xrReleaseSwapchainImage
}
```

### 5.6 `XrActionSet` / `XrAction<T>` — typed actions

```rust
pub struct XrActionSet { /* opaque handle */ }

pub trait XrAction<T: ActionValue> {
    fn name(&self) -> &str;
    fn current(&self, frame: &impl XrFrame) -> ActionState<T>;
    /// Suggest bindings for an interaction profile. xrSuggestInteractionProfileBindings.
    fn suggest_bindings(&self, profile: InteractionProfile, paths: &[BindingPath]);
}

pub trait ActionValue: Sized { /* sealed: bool, f32, Vec2, Pose3, … */ }
```

`aether-input::mapping::ActionMap` becomes a *manifest* — a serialisable description of named
actions and their suggested bindings. The HAL takes that manifest and produces typed
`XrAction<T>` handles.

### 5.7 `XrHaptics`

```rust
pub trait XrHaptics {
    type Error: std::error::Error + Send + Sync + 'static;
    fn apply(&self, target: HapticTarget, effect: HapticEffect) -> Result<(), Self::Error>;
                                                                          // xrApplyHapticFeedback
    fn stop(&self, target: HapticTarget) -> Result<(), Self::Error>;      // xrStopHapticFeedback
}
```

### 5.8 `XrAnchor` (optional, feature `anchors`)

For spatial anchors via `XR_MSFT_spatial_anchor` / `XR_FB_spatial_entity`. Out of scope for V1
delivery but the trait is reserved so the OpenXR backend can grow into it without breaking the
HAL.

## 6. Frame loop & session state machine

```
 +----------------+   xrPollEvent / SessionStateChanged
 | Idle           |─────────────────────────────────────┐
 +----------------+                                     │
        │ Ready                                         │
        ▼                                               │
 +----------------+                                     │
 | Ready          |──── xrBeginSession ───┐             │
 +----------------+                       ▼             │
                                   +-------------+      │
                                   | Synchronized|      │
                                   +-------------+      │
                                          │             │
                                          │ Visible     │
                                          ▼             │
                                   +-------------+      │
                                   | Visible     |      │
                                   +-------------+      │
                                          │             │
                                          │ Focused     │
                                          ▼             │
                                   +-------------+      │
                                   | Focused     |      │
                                   +-------------+      │
                                          │             │
                                          │ Stopping    │
                                          ▼             │
                                   +-------------+      │
                                   | Stopping    |──────┤
                                   +-------------+      │
                                          │ xrEndSession │
                                          ▼             │
 +----------------+                +-------------+      │
 | LossPending    |◄──── runtime ─►| Exiting     |◄─────┘
 +----------------+                +-------------+
```

Every frame, on the render thread:

```
loop {
    for ev in instance.poll_events() {
        session.advance_state(ev);   // updates state per OpenXR rules
    }
    if !session.is_running() { sleep(); continue; }

    let frame = session.wait_frame()?;          // xrWaitFrame
    let views = frame.locate_views(&local_ref)?; // xrLocateViews
    frame.sync_actions(&action_sets)?;           // xrSyncActions

    let mut builder = frame.begin()?;            // xrBeginFrame
    if frame.should_render() {
        for (eye, view) in views.iter().enumerate() {
            let img = swapchains[eye].acquire()?;
            swapchains[eye].wait(TIMEOUT)?;
            // wgpu render pass into img …
            swapchains[eye].release()?;
            builder.add_projection_layer(eye, view, &swapchains[eye]);
        }
    }
    frame.end(builder.finish())?;                // xrEndFrame
}
```

The `XrRuntime` helper in `aether-xr-hal` packages this loop so applications don't have to
re-implement it. The emulator backend obeys the same contract — it just composites to a
desktop window in `frame.end()`.

## 7. Action system

Today, `aether-input::mapping::ActionMap` describes per-button bindings. To map onto OpenXR
actions:

- A **manifest** (`ActionManifest`) declares named actions and their value types. Serialisable
  to JSON for tooling, but the canonical form is a Rust builder.
- A **profile-binding registry** declares suggested bindings per interaction profile (Touch,
  Index, Vive, Hand). Maps to `xrSuggestInteractionProfileBindings`.
- The HAL turns the manifest into `XrActionSet` + `XrAction<T>` handles.
- Per frame, `XrFrame::sync_actions` calls `xrSyncActions` for the attached sets, and action
  state is queried through `XrAction::current(&frame)`.
- `aether-input::graph::GestureDetector` and `aether-input::actions::InteractionEvent` are
  built on top of these typed action handles. The existing `XRButton` enum keeps its place as
  a presentation-layer label but is no longer the source of truth for input.

The emulator backend implements the same `XrAction<T>::current` contract by reading
`DesktopInputState` (keyboard, mouse, gamepad) and synthesising values.

## 8. Swapchain & rendering integration

V1 scope:

- HAL exposes `XrSwapchain::Image = wgpu::Texture`. Both backends produce wgpu textures so
  application render code is identical.
- The OpenXR backend uses Vulkan only (the `openxr` crate's first-class graphics binding) and
  bridges via `wgpu::hal::vulkan` / `wgpu_hal::vulkan::Device::texture_from_raw`. Per-eye
  swapchain creation uses the view-config-recommended dimensions
  (`xrEnumerateViewConfigurationViews`).
- The emulator backend allocates wgpu textures sized for a desktop window split-stereo display.
- Composition: V1 supports `XrCompositionLayerProjection` only. Quad layers, cylinder layers,
  cube layers, and passthrough layers are deferred to follow-ups but the trait surface
  (`LayerBuilder`) is designed to accept them later without a breaking change.
- D3D12 (Windows native) and Metal (macOS via SteamLink-like bridges) are deferred.

`aether-input::openxr_swapchain.rs` is replaced — its useful constants (`SwapchainFormat`,
`SwapchainUsage`) move into `aether-xr-hal` with wgpu-aligned values.

## 9. Extension policy

| Extension | Status in HAL | OpenXR backend behaviour |
|-----------|---------------|--------------------------|
| `XR_KHR_vulkan_enable2` | Required | Always activate; refuse instance creation if missing |
| `XR_EXT_hand_tracking` | Optional, surfaced as a `Capability` | Detect at instance enum, activate if present, surface absence to `aether-input::capabilities` |
| `XR_FB_passthrough` | Optional | Detect, no V1 trait support yet — surface as a capability flag |
| `XR_FB_eye_tracking_social` / `XR_EXT_eye_gaze_interaction` | Optional | Detect, surface as `Capability::EyeGaze` |
| `XR_KHR_composition_layer_depth` | Optional | Activate if present; depth submission improves reprojection |

Discovery flow:

1. `xrEnumerateInstanceExtensionProperties` at platform start.
2. `XrPlatform::available` returns `RuntimeDescriptor { extensions: Vec<ExtensionId> }`.
3. `InstanceConfig::required_extensions` and `optional_extensions` drive `xrCreateInstance`.
4. After session creation, `XrInstance::enabled_extensions` reflects the negotiated set.
5. `aether-input::capabilities::Capability` enum learns variants for each extension we surface;
   downstream code can branch on capability rather than backend type.

## 10. Migration plan (phased)

The follow-up `/batch` will spawn one worker per **unit** below. Units within the same phase
can run in parallel; each phase depends on the prior phase having merged. Sizes are rough
(small ≈ 1–2 files; medium ≈ 3–6 files; large ≈ workspace-wide consumer touch).

### P0 — Scaffold (parallel: 2 units)

| # | Unit | Files | Size |
|---|------|-------|------|
| P0-A | Create `crates/aether-xr-hal` with empty trait stubs and add to workspace | `Cargo.toml`, `crates/aether-xr-hal/{Cargo.toml,src/lib.rs}` | small |
| P0-B | Add pinned `openxr` dep to `aether-openxr/Cargo.toml`; gate behind feature `openxr-runtime` (default off) so CI stays green where the loader isn't installed | `crates/aether-openxr/Cargo.toml`, `src/lib.rs` | small |

### P1 — Type migration (parallel: 3 units)

| # | Unit | Files | Size |
|---|------|-------|------|
| P1-A | Move `Pose3`, `TrackingSnapshot`, `ControllerState`, `ControllerAnalog`, `ControllerButtons`, `Hand`, `HandJoint`, `HandJointSet`, `TrackingConfidence` from `aether-input::actions/openxr_tracking` into `aether-xr-hal::tracking` (re-export from `aether-input` for source-compat) | `crates/aether-xr-hal/src/tracking.rs`, `crates/aether-input/src/{actions,openxr_tracking,lib}.rs` | medium |
| P1-B | Move `SessionState`, `SessionTransitionError`, `ReferenceSpace`, `ReferenceSpaceType`, `SessionConfig` into `aether-xr-hal::session` | `crates/aether-xr-hal/src/session.rs`, `crates/aether-input/src/openxr_session.rs` | medium |
| P1-C | Move `SwapchainConfig`, `SwapchainFormat`, `SwapchainUsage`, `HapticAction`, `HapticTarget`, `SwapchainError` into `aether-xr-hal::{swapchain,haptics}` | `crates/aether-xr-hal/src/{swapchain,haptics}.rs`, `crates/aether-input/src/{openxr_swapchain,openxr_haptics}.rs` | medium |

After P1, every crate that today imports these types from `aether-input` continues to compile
via the re-exports. P9 removes those re-exports.

### P2 — Trait definitions (parallel: 3 units)

| # | Unit | Files | Size |
|---|------|-------|------|
| P2-A | Define `XrPlatform`, `XrInstance`, `XrEvent`, `InstanceConfig`, `RuntimeDescriptor` | `crates/aether-xr-hal/src/{platform,instance,event}.rs` | medium |
| P2-B | Define `XrSession`, `XrFrame`, `XrViewConfig`, `XrReferenceSpace`, `LayerBuilder`, `LayerSubmission` | `crates/aether-xr-hal/src/{session,frame,view,layer}.rs` | medium |
| P2-C | Define `XrSwapchain`, `XrActionSet`, `XrAction<T>`, `ActionManifest`, `InteractionProfile`, `XrHaptics` | `crates/aether-xr-hal/src/{swapchain,action,haptics,profile}.rs` | medium |

### P3 — OpenXR backend (mostly serial, then parallel sub-units)

| # | Unit | Files | Size |
|---|------|-------|------|
| P3-A | Real `XrPlatform` + `XrInstance` impls in `aether-openxr` (replaces `instance.rs`); `xrCreateInstance` with extension negotiation; `xrPollEvent` loop | `crates/aether-openxr/src/{instance,platform,event}.rs` | large |
| P3-B | Real `XrSession` impl; delete `aether-openxr::session::XrSession` stub; wire HAL state advancement from events | `crates/aether-openxr/src/session.rs` | medium |
| P3-C | Real `XrFrame` impl with `xrWaitFrame/xrLocateViews/xrBeginFrame/xrEndFrame`; replaces `frame_loop.rs` | `crates/aether-openxr/src/frame.rs` | medium |
| P3-D | Real `XrReferenceSpace` impl with `xrCreateReferenceSpace` + `xrLocateSpace` | `crates/aether-openxr/src/reference_space.rs` | small |

P3-B / P3-C / P3-D depend on P3-A landing first because they need `XrInstance`.

### P4 — Action system (parallel: 2 units after P4-A)

| # | Unit | Files | Size |
|---|------|-------|------|
| P4-A | `XrActionSet` + `XrAction<T>` impl in `aether-openxr`: `xrCreateActionSet`, `xrCreateAction`, `xrSuggestInteractionProfileBindings`, `xrAttachSessionActionSets`, `xrSyncActions` | `crates/aether-openxr/src/action.rs` | large |
| P4-B | `aether-input::mapping::ActionMap` adapter that builds an `ActionManifest` and exposes `XrAction<T>` to `GestureDetector` / `InteractionEvent` | `crates/aether-input/src/{mapping,graph,actions}.rs` | medium |
| P4-C | Emulator backend `XrAction<T>` impls reading `DesktopInputState` | `crates/aether-vr-emulator/src/action.rs` | medium |

### P5 — Swapchain & composition (serial within unit)

| # | Unit | Files | Size |
|---|------|-------|------|
| P5-A | OpenXR `XrSwapchain` impl with `wgpu_hal::vulkan` interop; per-eye creation from view-config recommended size | `crates/aether-openxr/src/swapchain.rs` | large |
| P5-B | `LayerBuilder` projection-layer support; submission via `xrEndFrame` | `crates/aether-openxr/src/{frame,layer}.rs` | medium |
| P5-C | Update render path in `aether-vr-overlay` (debug overlay) to draw via the new `XrSwapchain` images | `crates/aether-vr-overlay/src/*.rs` | medium |

### P6 — Haptics

| # | Unit | Files | Size |
|---|------|-------|------|
| P6-A | OpenXR `XrHaptics` impl using `xrApplyHapticFeedback` / `xrStopHapticFeedback`; emulator no-op impl with logging | `crates/aether-openxr/src/haptics.rs`, `crates/aether-vr-emulator/src/haptics.rs` | medium |
| P6-B | Wire `aether-input::haptics::HapticDispatcher` to `XrHaptics`; delete `aether-input/src/openxr_haptics.rs` | `crates/aether-input/src/haptics.rs` | small |

### P7 — Emulator parity

| # | Unit | Files | Size |
|---|------|-------|------|
| P7-A | `XrPlatform` / `XrInstance` / `XrSession` impls in `aether-vr-emulator` that replace its ad-hoc `TrackingSnapshot` synthesis | `crates/aether-vr-emulator/src/{platform,instance,session}.rs` | large |
| P7-B | `XrFrame` + `XrSwapchain` (wgpu desktop window) emulator impls; verify the same `XrRuntime` loop drives both backends | `crates/aether-vr-emulator/src/{frame,swapchain}.rs` | medium |

### P8 — `RuntimeAdapter` removal

| # | Unit | Files | Size |
|---|------|-------|------|
| P8-A | Replace `RuntimeAdapter` consumers with `XrRuntime` / `XrPlatform`; rebuild `aether-input::runtime::InputRuntime` on top of the HAL | `crates/aether-input/src/{runtime,adapter}.rs`, `examples/single-world-demo/**` | large |
| P8-B | Delete `OpenXrAdapter` (`crates/aether-input/src/openxr.rs`); decide `DesktopAdapter` fate (rebuild as keyboard-shim or delete entirely) | `crates/aether-input/src/{openxr,desktop,lib}.rs` | medium |

### P9 — Cleanup

| # | Unit | Files | Size |
|---|------|-------|------|
| P9-A | Delete `aether-input/src/openxr_session.rs`, `openxr_tracking.rs`, `openxr_swapchain.rs`, `openxr_haptics.rs`; drop transition re-exports from `lib.rs`; update consumers to import from `aether-xr-hal` directly | `crates/aether-input/src/lib.rs` + every consumer | large |
| P9-B | Delete `aether-openxr::input_actions` stub; update `lib.rs`; doc pass on `aether-xr-hal` README | `crates/aether-openxr/src/{lib,input_actions}.rs`, `crates/aether-xr-hal/README.md` | small |

Total: **~22 work units** across 10 phases. Phases P0–P2 are pure scaffolding (low risk).
P3–P7 are the meat. P8–P9 are cleanup.

## 11. Risks & open questions

These need answers before P0 starts:

1. **`openxr` crate version.** The `openxr` crate (the Rust bindings around the OpenXR loader)
   has not had a 1.0 release; pinning a specific version + git rev is recommended. Decide
   between the published `openxr` crate vs. a workspace-vendored fork.
2. **Action manifest format.** Pure-Rust builder, JSON file loaded at runtime, or both?
   Recommendation: Rust builder is the source of truth, JSON exists only for tooling export.
3. **Should `aether-input` be folded into `aether-xr-hal`?** Counter-argument: keep the
   interaction primitives (locomotion, comfort, deadzone, gesture graph) separate from the
   raw HAL so non-XR code can depend on input semantics without pulling in the HAL.
   Recommendation: keep the split.
4. **Linux runtime selection.** Monado vs. SteamVR vs. Wivrn — at instance creation, the
   loader picks, but we may want to expose a runtime hint via env var. Defer to docs.
5. **Headless CI strategy.** When no OpenXR loader is installed (which will be true on most
   CI runners), the `aether-openxr` crate must compile but not link. Use the
   `openxr-runtime` feature flag introduced in P0-B; CI builds without it; a separate
   GPU-runner job builds with it and runs basic instance-creation tests against Monado.
6. **D3D12 / Metal swapchain bridging.** Out of V1 scope. The HAL trait is generic enough to
   add later via additional `XrSwapchain` impls without breaking the trait surface.
7. **Hand-tracking trait shape.** Should `XrHandTracking` be its own trait or a method on
   `XrSession` gated by capability? Recommendation: separate trait, returned by
   `XrSession::hand_tracking()` only when the extension is active.

## 12. Verification

Per phase:

- **P0–P2 (scaffolding & types):** `cargo check --workspace` and `cargo test --workspace`
  must stay green. No behaviour changes; type re-exports preserve compatibility.
- **P3–P5 (real backend):** add `cargo test -p aether-openxr --features openxr-runtime` to a
  GPU-enabled CI runner; minimum coverage = "create instance, create session, run 10 frames,
  destroy session" against Monado in headless mode.
- **P6 (haptics):** unit test that `HapticDispatcher::dispatch` reaches a recording mock
  `XrHaptics` impl.
- **P7 (emulator parity):** `examples/single-world-demo` runs end-to-end against the
  emulator backend with no `aether-input::OpenXrAdapter` references remaining. Manual
  smoke-test: keyboard + mouse drives a synthesized head pose visible in the overlay.
- **P8–P9 (cleanup):** `grep -r "RuntimeAdapter\|openxr_session\|openxr_tracking\|openxr_haptics\|openxr_swapchain\|aether_input::openxr" crates/ examples/ services/`
  must return zero hits at the end of P9. Final `cargo test --workspace` green.
- **End-to-end:** with a real headset (Quest 3 over Link) on a Linux dev machine running
  Monado, the demo app launches via `cargo run -p single-world-demo --features
  openxr-runtime` and shows a tracked head + controllers in the world. This is a manual
  acceptance test before declaring V1 done.

CI matrix for the refactor branch:

| Job | OS | Features | Expected |
|-----|----|----------|----------|
| `check` | linux/macos/windows | default | green |
| `test` | linux/macos/windows | default | green |
| `openxr-headless` | linux | `openxr-runtime` | green (Monado headless) |
| `openxr-headed` | linux self-hosted | `openxr-runtime` | gated, manual trigger |

---

## Appendix A — File-touch summary

Files known to need changes (not exhaustive — discovered consumer chasing during P1/P9 may
add to this list):

- `Cargo.toml` (workspace) — add `crates/aether-xr-hal`.
- `crates/aether-xr-hal/**` — new crate.
- `crates/aether-openxr/Cargo.toml` — add `openxr` dep + feature.
- `crates/aether-openxr/src/{lib,instance,session,frame_loop,input_actions,swapchain,error}.rs` — rewritten.
- `crates/aether-input/Cargo.toml` — depend on `aether-xr-hal`.
- `crates/aether-input/src/{lib,adapter,actions,openxr,openxr_session,openxr_tracking,openxr_haptics,openxr_swapchain,desktop,runtime,mapping,graph,haptics}.rs` — varying changes.
- `crates/aether-vr-emulator/**` — becomes a HAL backend.
- `crates/aether-vr-overlay/**` — rebuild against new types.
- `examples/single-world-demo/**` — switch to `XrRuntime`.

## Appendix B — Naming guide

- `Xr*` prefix for HAL traits, `XrSomething` for value types only when ambiguity with std
  (e.g. `XrTime` vs `std::time::Instant`).
- Extension activation: `ExtensionId("XR_EXT_hand_tracking")` not `enum`, so unknown
  extensions discovered via enumeration can still be reported.
- Backends: `aether-openxr::OpenXrPlatform`, `aether-vr-emulator::EmulatorPlatform`. Both
  implement `XrPlatform`.
