# Multi-Platform Client Support (task-022)

Added client/platform capability and quality adaptation profiles for multi-platform support.

## Implemented API surface

- Added crate `aether-platform` with platform kinds, compliance metadata, quality and build mode descriptors.
- Added inputs and input backend enums.
- Added script execution profile (`ClientJit`, `ServerAot`, etc.) mapping.

## Remaining implementation work

- Implement platform build matrices and certification gating.
- Implement runtime adaptation logic tied to measured device capacity.
