# VR Input & Interaction (task-009)

Implemented the initial input abstraction surface and locomotion/comfort data model.

## Implemented API surface

- Added crate `aether-input` with modules:
  - `capabilities`: headset/runtime capability typing.
  - `actions`: controller/interaction events and grab/use targets.
  - `haptics`: haptic wave/effect/request models.
  - `locomotion`: locomotion modes and comfort controls (vignette/snap turn/seated).
  - `adapter`: runtime input adapter trait with poll/update contract.
  - `openxr`: OpenXR adapter facade with session polling and profile injection.
- Updated workspace membership to include `aether-input`.

## Mapping to acceptance criteria

- `#1` OpenXR abstraction represented by `aether-input::openxr::OpenXrAdapter` and backend trait.
- `#2` hand+controller abstraction via `InputActionPath` and action event channel.
- `#3` interaction verbs (`grab`, `use`, `point`, `throw`) can be expressed as `InteractionEvent` and `InteractionTarget` payloads.
- `#4` `HapticRequest` and `HapticEffect` provide API contracts for controller feedback.
- `#5` `LocomotionMode` and `LocomotionProfile` include teleport/smooth/climb/fly variants.
- `#6` `ComfortProfile` carries vignette/snap turn/seated mode parameters.

## Remaining implementation work

- Add concrete OpenXR event conversion layer and device polling implementation.
- Add physics integration for throw trajectories and world-aware action validation.
- Add session/runtime mapping per world settings and dynamic mode switching.
