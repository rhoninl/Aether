pub mod acoustics;
pub mod attenuation;
pub mod capture;
pub mod channel;
pub mod codec;
pub mod device;
pub mod hrtf;
pub mod loader;
pub mod opus;
pub mod output;
pub mod runtime;
pub mod types;

pub use acoustics::{AcousticsProfile, OcclusionState, RoomAcoustics};
pub use attenuation::{AttenuationCurve, AttenuationModel, DistanceBand};
pub use capture::{CaptureConfig, CaptureRingBuffer, CaptureStream};
pub use channel::{
    ChannelConfig, ChannelId, ChannelKind, RoutingPolicy, RoutingRequest, VoiceChannelManager,
    VoiceZone, ZoneEvent,
};
pub use codec::{AudioCodec, CodecEncodeError, StubCodec};
pub use device::{AudioDeviceManager, DeviceConfig, DeviceError, DeviceInfo};
pub use hrtf::{HrtfProfile, HrtfSample};
pub use acoustics::HrtfTransportParams;
pub use loader::{AudioAsset, LoadError};
pub use opus::{BitRateKbps, CodecError, OpusConfig, OpusPacket};
pub use output::{OutputPipeline, PlaybackSource, SpatialRenderer};
pub use runtime::{
    AudioMixInstruction, AudioRuntime, AudioRuntimeConfig, AudioRuntimeInput, AudioRuntimeOutput,
    RuntimeProfiler,
};
pub use types::{AudioId, AudioLod, AudioSource, ListenerState, Vec3};
