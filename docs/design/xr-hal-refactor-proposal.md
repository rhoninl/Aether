# XR HAL Refactor — Headset Abstraction Layer

> **Status:** Proposal / TODO. Not yet scheduled. Discussed 2026-04-28.
> **Owner:** TBD. To be executed in a dedicated session.
> **Supersedes (partially):** `openxr-vr-integration.md` — that doc's scope is kept, but its abstractions are promoted out of `aether-input` into a dedicated HAL crate.

## Background

Today the engine has three crates that touch the headset boundary:

- `aether-openxr` — concrete OpenXR session, frame loop, swapchain, input actions, instance.
- `aether-platform` — capability/profile/feature/budget descriptors (mostly metadata, not a runtime contract).
- `aether-vr-emulator` — desktop window + simulated head/controller for testing.

Several engine crates (`aether-input`, `aether-renderer`, `aether-world-runtime`, `aether-avatar`, `aether-multiplayer`) need access to headset poses, frames, inputs, anchors, and composition. There is no single HAL contract they depend on; couplings drift toward `aether-openxr` or duplicate types per crate.

## Motivation

Every commercial XR headset (Meta Quest, Pico, HTC, Valve Index, HoloLens, Magic Leap 2) only exposes **OpenXR** as a stable runtime contract. Meta has deprecated their proprietary mobile SDK in favor of OpenXR. Apple Vision Pro is the only meaningful exception (visionOS APIs, no OpenXR).

This means the design question is **not** "OpenXR vs custom HAL" — that's a false binary, since any non-OpenXR layer would re-wrap OpenXR for every commercial device. The real question is:

> **How much of OpenXR's worldview is allowed to leak up into the engine layer?**

## Tradeoff Analysis

Five dimensions worth weighing:

1. **Thickness of the abstraction.** OpenXR has strong opinions on session lifecycle, frame loop, swapchain ownership, action sets, reference spaces, composition layers. The engine can either expose those concepts directly (fast now, painful later) or define Aether-native concepts and translate (slower now, testable + future-proof).

2. **Coverage of OpenXR's surface.** OpenXR's required core is small. Differentiation lives in extensions — Meta passthrough, Quest hand/body/face tracking, scene understanding, eye tracking. Raw extension escape hatches re-couple to vendors. Capability traits decouple them.

3. **Non-headset targets.** `aether-vr-emulator` already exists; WebXR (creator preview), headless multiplayer servers, and eventually visionOS are all on the road. None of these can pretend to be OpenXR cleanly.

4. **Layering line.** Even granting a HAL, you decide what it owns: frame loop, swapchain, input bindings, anchors. Adopting OpenXR-shaped semantics (xrWaitFrame / xrBeginFrame / xrEndFrame style) inside the HAL is fine — every backend ends up implementing roughly that — as long as OpenXR *types* don't leak.

5. **Determinism and testing.** Multiplayer, economy, UGC, behavior-DSL all need deterministic headless tests. A concept-layer HAL gives a trivial `MockHal`. Pure OpenXR pass-through forces mock OpenXR runtimes everywhere.

### Resolution

Because every commercial headset terminates in OpenXR, a HAL surface that mirrors OpenXR's stable core *shapes* is automatically universal — no portability cost. What's left is one-time translation cost (~15–25% more adapter code than a thin pass-through) in exchange for testability, simulator support, WebXR path, and visionOS readiness.

## Proposal

### Architecture

```
┌────────────────────────────────────────────────────────┐
│ Engine crates                                          │
│ aether-input · aether-renderer · aether-world-runtime  │
│ aether-avatar · aether-multiplayer · aether-vr-overlay │
└────────────────────────┬───────────────────────────────┘
                         │ depends only on
                         ▼
┌────────────────────────────────────────────────────────┐
│ aether-xr-hal  (NEW — contract crate)                  │
│   core traits:    Session, Frame, Headset, Pose,       │
│                   Space/ReferenceFrame, Swapchain,     │
│                   CompositionLayer, InputAction,       │
│                   Anchor                               │
│   capability traits: Passthrough, HandTracking,        │
│                   EyeTracking, FaceTracking,           │
│                   BodyTracking, SceneUnderstanding,    │
│                   Haptics                              │
└─────────┬────────────────────┬────────────────┬────────┘
          │                    │                │
          ▼                    ▼                ▼
   ┌──────────────┐    ┌───────────────┐  ┌──────────────┐
   │ aether-openxr│    │aether-vr-     │  │ future:      │
   │  (backend)   │    │  emulator     │  │ aether-webxr │
   │  Quest, Pico,│    │  (backend)    │  │ aether-      │
   │  Vive, Index,│    │  desktop test │  │   visionos   │
   │  HoloLens... │    │               │  │              │
   └──────────────┘    └───────────────┘  └──────────────┘
```

### Core HAL surface (target)

Small required core — every backend must implement:

- `Session` — lifecycle (init, begin, end, poll events), reference-frame creation.
- `Frame` — predicted display time, view poses, begin/end semantics.
- `Headset` — device descriptor, view configuration, IPD, refresh rate.
- `Pose` — position + orientation + tracking confidence + timestamp.
- `Space` / `ReferenceFrame` — local, stage, view, custom anchor-relative.
- `Swapchain` — image acquisition/release, format negotiation, wgpu interop.
- `CompositionLayer` — projection, quad, cylinder layers; alpha modes.
- `InputAction` — action sets, suggested bindings per interaction profile, action state queries.
- `Anchor` — persistent or session-scoped spatial anchors.

### Capability traits (optional, runtime-queried)

Vendor extensions plug in here; engine code uses the trait, not the vendor:

- `Passthrough` — Meta Quest passthrough, Vive passthrough, etc.
- `HandTracking` — Quest v2, Pico v1, etc. (unified joint model).
- `EyeTracking` — Quest Pro, Vive Pro Eye, Vision Pro (when added).
- `FaceTracking` — Quest Pro face, etc.
- `BodyTracking` — Quest body, full-body solutions.
- `SceneUnderstanding` — scene mesh, plane detection, room layout.
- `Haptics` — pulse, sine, ramp, amplitude envelope.

A backend reports which capabilities it supports at session init; engine code branches on `Option<&dyn HandTracking>` etc.

### Vendor escape hatch

Reserve a `vendor_extensions()` accessor returning an opaque handle, behind explicit feature gates (`feature = "meta-experimental"`, `feature = "pico-experimental"`). Used only by experimental modules; never by stable engine crates.

## Migration Plan

Phased so the engine keeps building at every step.

### Phase 0 — Design review (before any code)

- [ ] Review this doc with maintainers / tech-lead agent.
- [ ] Confirm trait surface granularity (especially `Swapchain` ↔ wgpu interop, `CompositionLayer` shape, `InputAction` binding suggestion API).
- [ ] Confirm error model (one HAL error enum vs per-trait errors).
- [ ] Confirm async vs sync surface (frame loop is naturally sync; capability queries can be async).

### Phase 1 — Create the contract crate

- [ ] New crate `aether-xr-hal` with core traits + capability traits + types (`Pose`, `Fov`, `ViewConfig`, `Extent2D`, `SwapchainFormat`, etc.).
- [ ] No backend implementations yet; `MockHal` for unit tests.
- [ ] Decide whether `aether-platform`'s capability/profile metadata moves into the HAL or stays separate (recommended: stays separate — `aether-platform` describes *device classes*, HAL describes *runtime contract*).

### Phase 2 — Reshape `aether-openxr` as a backend

- [ ] Implement HAL traits using existing `session.rs`, `frame_loop.rs`, `swapchain.rs`, `input_actions.rs`, `instance.rs`.
- [ ] Map vendor extensions to capability traits (`Passthrough`, `HandTracking`, etc.).
- [ ] Keep `aether-openxr` types `pub(crate)` where possible; expose only via HAL traits.

### Phase 3 — Reshape `aether-vr-emulator` as a backend

- [ ] Implement HAL traits over the existing desktop window + simulated tracking.
- [ ] Implement at least `Haptics` (no-op) and `HandTracking` (synthesized from mouse/keyboard) so dev workflows on desktop exercise the same code paths.

### Phase 4 — Migrate engine consumers

- [ ] `aether-input`: remove direct OpenXR coupling; depend on HAL only. The current `OpenXrAdapter` becomes a thin shim over `dyn Session`.
- [ ] `aether-renderer`: swapchain interop goes through HAL `Swapchain` trait.
- [ ] `aether-world-runtime`: frame loop driven by HAL `Frame`.
- [ ] `aether-avatar`: hand/body/face tracking reads via capability traits (graceful fallback when absent).
- [ ] `aether-vr-overlay`: composition layers via HAL `CompositionLayer`.
- [ ] `aether-multiplayer`: pose serialization uses HAL `Pose` (already mostly HAL-shaped).

### Phase 5 — Tests + CI

- [ ] All engine-crate unit tests run against `MockHal` — no OpenXR runtime required.
- [ ] Integration test suite using `aether-vr-emulator` backend on CI (headed-less compatible).
- [ ] OpenXR backend smoke tests gated to a Quest / Steam runtime job (manual or self-hosted).

### Phase 6 — Stretch / future backends

- [ ] `aether-webxr` skeleton (wasm-bindgen + WebXR Device API).
- [ ] `aether-visionos` skeleton (when relevant — RealityKit/CompositorServices).

## Affected Crates

| Crate                  | Change                                                |
| ---------------------- | ----------------------------------------------------- |
| `aether-xr-hal` (new)  | Define contract traits + types + `MockHal`.           |
| `aether-openxr`        | Implement HAL traits; hide internal types.            |
| `aether-vr-emulator`   | Implement HAL traits over desktop window backend.     |
| `aether-platform`      | Possibly slim down — keep device-class metadata only. |
| `aether-input`         | Drop direct OpenXR coupling; depend on HAL.           |
| `aether-renderer`      | Swapchain integration via HAL.                        |
| `aether-world-runtime` | Frame loop via HAL.                                   |
| `aether-avatar`        | Hand/body/face tracking via capability traits.        |
| `aether-vr-overlay`    | Composition layers via HAL.                           |
| `aether-multiplayer`   | Use HAL `Pose` type.                                  |

## Risks

- **Trait-surface bikeshedding.** Composition layer types and action binding suggestion semantics are the OpenXR concepts most resistant to clean abstraction. Allocate explicit design review time on these two.
- **Swapchain ↔ wgpu interop.** The HAL must expose enough for the renderer without leaking OpenXR `XrSwapchain` types. Likely API: HAL hands out `wgpu::Texture` views per-frame.
- **Async vs sync.** OpenXR's frame loop is synchronous and tight (xrWaitFrame blocks). Avoid async on the hot path; keep capability queries async-friendly.
- **Migration churn for in-flight features.** Sequence Phase 4 to land per-consumer, not all at once. Each consumer crate migrates in its own PR.
- **`aether-platform` overlap.** Risk of double-defining capability concepts. Decide early: HAL = runtime contract, `aether-platform` = device-class metadata. They reference each other but don't duplicate.

## Validation Plan

- Engine-crate unit tests pass against `MockHal` with zero OpenXR dependency.
- `aether-vr-emulator` backend runs the existing example scenes end-to-end on desktop CI.
- OpenXR backend runs against a real headset (Quest 3 or Steam VR) and reaches first-frame parity with current main.
- Smoke test on Quest APK build (`build-command-quest-apk.md` flow) — must still produce a working APK after Phase 2 lands.

## Out of Scope

- Implementing `aether-webxr` or `aether-visionos` (Phase 6 is a stretch placeholder, not part of this refactor).
- Replacing wgpu or changing the renderer architecture.
- Rewriting `aether-input`'s higher-level interaction model — only its boundary with the runtime changes.

## Open Questions for the Executing Session

1. Does `aether-platform` keep its capability/profile descriptors, or do they merge into the HAL? Recommendation: keep separate (different concerns).
2. Single HAL error enum or per-trait errors? Recommendation: single `HalError` with variants per subsystem; trait methods return `Result<_, HalError>`.
3. Frame-loop ownership: does the engine drive `wait_frame → begin_frame → end_frame`, or does the HAL expose a `run_frame(|ctx| { ... })` callback? Recommendation: explicit begin/end for parity with OpenXR semantics; engine drives.
4. How should `aether-vr-emulator` synthesize hand tracking — from mouse-projected ray, or from keyboard pose presets? Both are useful; ship presets first, ray later.

---

**Next step when picked up:** start with Phase 0 (design review of this doc), then Phase 1 (`aether-xr-hal` crate skeleton + `MockHal`). Do not start Phase 2 until Phase 1 compiles and `MockHal` has unit-test coverage of every core trait.
