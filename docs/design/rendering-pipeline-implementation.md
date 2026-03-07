# Rendering Pipeline Implementation Notes (task-002)

This checkpoint focuses on rendering policy and scheduling primitives for the pipeline task.

## Implemented in code

- New workspace crate: `crates/aether-renderer`.
- Added policy/config layer for:
  - Stereo/multiview settings (`StereoConfig`)
  - Foveated rendering knobs with VRS tier support (`FoveationConfig`)
  - Clustered lighting limits (`ClusterLightingConfig`)
  - Cascade shadow controls and downscale budget clamp (`ShadowCascadeConfig`)
  - LOD levels with hysteresis transition helper (`LODPolicy`, `LodCurve`)
  - Streaming priorities and requests (`StreamRequest`, `StreamPriority`)
- Added scheduling helpers:
  - `FrameScheduler::estimate_workload`
  - `decide_frame_mode`/`decide_mode` and workload buckets
  - Cascade resolution budget clamps for constrained memory budgets
- Added GPU batching helper model:
  - `MaterialBatchKey`
  - `batch_instances_by_key` deterministic grouped batches
- Added progressive mesh streaming controller:
  - `ProgressiveMeshStreaming::choose_next_level`

## Remaining work

- Hook these abstractions to the concrete `wgpu` render backend, including:
  - actual `VK_KHR_multiview` pass setup
  - eye-gaze driven VRS profile updates
  - clustered shading implementation
  - shadow map dispatch and resource creation
  - GPU-driven render backend that consumes batch hints
