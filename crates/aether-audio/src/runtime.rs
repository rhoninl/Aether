use std::collections::HashMap;

use crate::acoustics::{AcousticsProfile, HrtfTransportParams};
use crate::attenuation::{AttenuationCurve, AttenuationModel};
use crate::channel::{
    ChannelConfig, ChannelKind, RoutingPolicy, RoutingRequest, VoiceChannelManager,
};
use crate::opus::{OpusConfig, OpusPacket};
use crate::types::{AudioId, AudioLod, AudioSource, ListenerState};

#[derive(Debug, Default)]
pub struct RuntimeProfiler {
    pub processed_sources: usize,
    pub voice_packets: usize,
    pub dropped_packets: usize,
    pub routed_packets: usize,
    pub max_observed_gain: f32,
}

#[derive(Debug, Clone)]
pub struct AudioRuntimeConfig {
    pub attenuation: AttenuationModel,
    pub acoustics: AcousticsProfile,
    pub opus: OpusConfig,
    pub max_concurrent_sources: u16,
    pub quality_cap_high_concurrency: u16,
}

impl Default for AudioRuntimeConfig {
    fn default() -> Self {
        Self {
            attenuation: AttenuationModel::from_preset_inverse(),
            acoustics: AcousticsProfile::offline(),
            opus: OpusConfig::opus_voice_default(),
            max_concurrent_sources: 64,
            quality_cap_high_concurrency: 24,
        }
    }
}

#[derive(Debug, Default)]
pub struct AudioRuntimeState {
    active_source_sequence: HashMap<AudioId, u64>,
    last_encoded_seq: HashMap<(u64, u64), u64>,
}

#[derive(Debug, Clone)]
pub struct AudioRuntimeInput {
    pub now_ms: u64,
    pub listener: ListenerState,
    pub sources: Vec<AudioSource>,
    pub routing_requests: Vec<RoutingRequest>,
    pub source_channels: Vec<ChannelConfig>,
    pub max_channels: usize,
}

#[derive(Debug, Clone)]
pub struct AudioMixInstruction {
    pub source_id: AudioId,
    pub gain: f32,
    pub hrtf: HrtfTransportParams,
    pub lod: AudioLod,
    pub bandwidth_profile: String,
    pub route: RoutingPolicy,
}

#[derive(Debug)]
pub struct AudioRuntimeOutput {
    pub now_ms: u64,
    pub instructions: Vec<AudioMixInstruction>,
    pub packets: Vec<OpusPacket>,
    pub profiler: RuntimeProfiler,
}

#[derive(Debug)]
pub struct AudioRuntime {
    cfg: AudioRuntimeConfig,
    state: AudioRuntimeState,
    _channel_mgr: VoiceChannelManager,
    _profiler: RuntimeProfiler,
}

impl Default for AudioRuntime {
    fn default() -> Self {
        Self::new(AudioRuntimeConfig::default())
    }
}

impl AudioRuntime {
    pub fn new(cfg: AudioRuntimeConfig) -> Self {
        Self {
            cfg,
            state: AudioRuntimeState::default(),
            _channel_mgr: VoiceChannelManager::new(),
            _profiler: RuntimeProfiler::default(),
        }
    }

    pub fn step(&mut self, input: AudioRuntimeInput) -> AudioRuntimeOutput {
        let capacity = if input.sources.len() as u16 > self.cfg.quality_cap_high_concurrency {
            self.cfg.quality_cap_high_concurrency
        } else {
            self.cfg.max_concurrent_sources
        };
        let max_channels = input.max_channels.max(1) as u32;
        let mut profiler = RuntimeProfiler::default();
        let mut instructions = Vec::new();
        let mut packets = Vec::new();
        for source in input.sources.iter().take(usize::from(capacity)) {
            let distance = distance_between(
                &self.listener_pos(&input.listener),
                (source.position.x, source.position.y, source.position.z),
            );
            let band = self.cfg.attenuation.band(distance);
            let gain = self.cfg.attenuation.gain(distance) * source.volume * band.gain;
            let lod = AcousticsProfile::lod_for_distance(distance);
            let (mut hrtf, bandwidth_profile) =
                self.pick_voice_quality(distance, max_channels as usize);
            if let Some(source_channel) = input
                .source_channels
                .iter()
                .find(|channel| channel.world_id == source.world_id)
                .map(|channel| channel.kind)
            {
                match source_channel {
                    ChannelKind::Proximity => hrtf.distance_gain *= 0.95,
                    ChannelKind::Private => hrtf.distance_gain *= 0.90,
                    ChannelKind::World => hrtf.distance_gain *= 1.05,
                }
            }
            let route = self.route_source(source.id, &input.routing_requests);

            let packet_size = self.estimate_packet_size(source.id, &bandwidth_profile);
            let seq_val = {
                let seq = self
                    .state
                    .active_source_sequence
                    .entry(source.id)
                    .and_modify(|value| *value = value.saturating_add(1))
                    .or_insert(0);
                *seq
            };
            let packet = OpusPacket {
                sequence: seq_val,
                payload: vec![0u8; packet_size],
                codec_ms: self.cfg.opus.frame_ms,
            };
            let target_gain = gain.max(0.0);
            if target_gain > 0.02 {
                if let Some(replay) = self.state.last_encoded_seq.get(&(source.id.0, seq_val)) {
                    if *replay == seq_val {
                        profiler.dropped_packets = profiler.dropped_packets.saturating_add(1);
                    }
                }
                packets.push(packet);
                profiler.voice_packets = profiler.voice_packets.saturating_add(1);
                profiler.routed_packets = profiler.routed_packets.saturating_add(1);
                let _ = self
                    .state
                    .last_encoded_seq
                    .insert((source.id.0, seq_val), seq_val);
            } else {
                profiler.dropped_packets = profiler.dropped_packets.saturating_add(1);
            }
            profiler.max_observed_gain = profiler.max_observed_gain.max(target_gain);

            instructions.push(AudioMixInstruction {
                source_id: source.id,
                gain: target_gain,
                hrtf,
                lod,
                bandwidth_profile,
                route,
            });
        }

        profiler.processed_sources = input.sources.len();
        profiler.dropped_packets = profiler.dropped_packets.min(input.sources.len());

        AudioRuntimeOutput {
            now_ms: input.now_ms,
            instructions,
            packets,
            profiler,
        }
    }

    fn pick_voice_quality(
        &mut self,
        distance_m: f32,
        max_channels: usize,
    ) -> (HrtfTransportParams, String) {
        let profile = if max_channels > 32 {
            ("low".to_string(), 0.35)
        } else {
            ("high".to_string(), 1.0)
        };
        let mut profile_name = if self.cfg.attenuation.curve == AttenuationCurve::Exponential {
            "expo"
        } else {
            "linear"
        }
        .to_string();
        if distance_m > self.cfg.acoustics.room.room_size_m2 {
            profile_name.push_str("-far");
        }
        let params = HrtfTransportParams {
            azimuth_deg: (distance_m * 4.5) % 360.0,
            elevation_deg: (distance_m * 1.6) % 45.0,
            distance_gain: self.cfg.acoustics.room.occlusion * 0.8,
            occlusion: self.cfg.acoustics.room.occlusion,
            reflectivity: self.cfg.acoustics.room.early_reflection_gain,
        };
        (
            params,
            if profile.0 == "low" {
                "low".into()
            } else {
                profile_name
            },
        )
    }

    fn route_source(&self, source_id: AudioId, requests: &[RoutingRequest]) -> RoutingPolicy {
        let default = RoutingPolicy::Defer;
        for req in requests {
            if req.source_channel.0 % 10 + req.target_zone.unwrap_or(0) == source_id.0 % 10 {
                return RoutingPolicy::Allow;
            }
        }
        if requests
            .iter()
            .any(|request| request.source_channel.0 > 0 && request.target_player_id == source_id.0)
        {
            return RoutingPolicy::Allow;
        }
        default
    }

    fn listener_pos(&self, listener: &ListenerState) -> (f32, f32, f32) {
        (
            listener.position.x,
            listener.position.y,
            listener.position.z,
        )
    }

    fn estimate_packet_size(&self, source_id: AudioId, profile: &str) -> usize {
        let base: usize = if profile == "low" { 90 } else { 180 };
        base.saturating_add(usize::try_from(source_id.0 % 3).unwrap_or_default() * 4)
            .min(OpusPacket::packet_size_limit(&self.cfg.opus))
    }
}

fn distance_between(a: &(f32, f32, f32), b: (f32, f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    let dz = a.2 - b.2;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AudioSource, ListenerState, Vec3};

    #[test]
    fn attenuation_drives_gain_down_with_range() {
        let runtime = AudioRuntime::default();
        let profile = runtime.cfg.attenuation;
        assert!(profile.gain(0.5) > profile.gain(90.0));
    }

    #[test]
    fn routing_produces_instructions_for_active_sources() {
        let mut runtime = AudioRuntime::new(AudioRuntimeConfig {
            attenuation: AttenuationModel::from_preset_linear(),
            acoustics: AcousticsProfile::voice_mode(),
            opus: OpusConfig::opus_voice_default(),
            max_concurrent_sources: 8,
            quality_cap_high_concurrency: 4,
        });
        let out = runtime.step(AudioRuntimeInput {
            now_ms: 100,
            listener: ListenerState {
                position: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                forward: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                up: Vec3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                },
            },
            sources: vec![AudioSource {
                id: AudioId(1),
                position: Vec3 {
                    x: 3.0,
                    y: 0.0,
                    z: 0.0,
                },
                volume: 1.0,
                world_id: 1,
            }],
            routing_requests: vec![],
            source_channels: vec![],
            max_channels: 8,
        });
        assert_eq!(out.instructions.len(), 1);
        assert_eq!(out.packets.len(), 1);
    }
}
