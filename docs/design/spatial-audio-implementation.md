# Spatial Audio Implementation Notes (task-004)

## Implemented foundations

- New workspace crate: `crates/aether-audio`.
- Added execution-agnostic domain modules:
  - `types` for vectors, listeners, sources, and audio LOD buckets
  - `attenuation` for distance falloff presets and continuous gain functions
  - `acoustics` for room reverb/occlusion model and LOD-by-distance policy
  - `hrtf` for HRTF sample/profile stubs
  - `opus` for Opus configuration and packet envelope metadata
  - `channel` for routing and membership policies for proximity/private/world channels

## Notes

- This crate models acceptance criteria in configuration and scheduler form:
  - #1: HRTF profile shape and sample selection
  - #2: Configurable attenuation curves and gain bands
  - #3: Reverb/occlusion/reflection fields and room profile presets
  - #4: Voice chat channels/zones with routing policies
  - #5: Opus codec fields including bitrate and in-band FEC flag
  - #6: Audio LOD by distance is available for backend-specific processing downgrades
- Actual DSP/HRTF convolution, Opus runtime encode/decode, and packet transport remain as future integration.
