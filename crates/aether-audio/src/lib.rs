pub mod acoustics;
pub mod attenuation;
pub mod channel;
pub mod ecs_bridge;
pub mod hrtf;
pub mod opus;
pub mod types;
pub mod runtime;

pub use acoustics::{AcousticsProfile, OcclusionState, RoomAcoustics};
pub use attenuation::{AttenuationCurve, AttenuationModel, DistanceBand};
pub use channel::{
    ChannelConfig, ChannelId, ChannelKind, RoutingPolicy, RoutingRequest, VoiceChannelManager, VoiceZone,
    ZoneEvent,
};
pub use ecs_bridge::{
    play_sound, play_sound_at_transform, play_sound_looped, play_sound_with_volume,
    SoundHandle, SoundRequest, Transform3, DEFAULT_VOLUME, MAX_VOLUME, MIN_VOLUME,
};
pub use acoustics::HrtfTransportParams;
pub use hrtf::{HrtfProfile, HrtfSample};
pub use opus::{BitRateKbps, CodecError, OpusConfig, OpusPacket};
pub use types::{AudioId, AudioLod, AudioSource, ListenerState, Vec3};
pub use runtime::{
    AudioMixInstruction, AudioRuntime, AudioRuntimeConfig, AudioRuntimeInput, AudioRuntimeOutput,
    RuntimeProfiler,
};
