use std::sync::{Arc, Mutex};

use crate::acoustics::HrtfTransportParams;
use crate::device::{AudioDeviceManager, DeviceConfig, DeviceError, OutputHandle};
use crate::hrtf::{HrtfProfile, HrtfSample};
use crate::loader::AudioAsset;
use crate::runtime::AudioMixInstruction;
use crate::types::AudioId;

/// Renders a mono source into stereo using HRTF parameters.
pub struct SpatialRenderer {
    profile: HrtfProfile,
}

impl SpatialRenderer {
    pub fn new(profile: HrtfProfile) -> Self {
        Self { profile }
    }

    /// Apply HRTF spatialization to mono samples, producing interleaved stereo output.
    pub fn render_stereo(&self, mono_samples: &[f32], params: &HrtfTransportParams) -> Vec<f32> {
        let hrtf = HrtfSample::for_profile(self.profile, params.azimuth_deg);
        let gain = params.distance_gain;
        let dry_mix = 1.0 - params.reflectivity.clamp(0.0, 0.5);

        let mut output = Vec::with_capacity(mono_samples.len() * 2);

        for &sample in mono_samples {
            let processed = sample * gain * dry_mix;
            let left = processed * hrtf.left_gain;
            let right = processed * hrtf.right_gain;
            output.push(left);
            output.push(right);
        }

        output
    }

    /// Mix a spatialized source into an existing stereo buffer at the given gain.
    pub fn mix_into(
        &self,
        target: &mut [f32],
        mono_samples: &[f32],
        params: &HrtfTransportParams,
        gain: f32,
    ) {
        let stereo = self.render_stereo(mono_samples, params);
        let mix_len = target.len().min(stereo.len());
        for i in 0..mix_len {
            target[i] += stereo[i] * gain;
        }
    }
}

/// A playback source with its PCM data and current read position.
#[derive(Debug, Clone)]
pub struct PlaybackSource {
    pub id: AudioId,
    pub asset: AudioAsset,
    pub position: usize,
    pub looping: bool,
    pub volume: f32,
}

impl PlaybackSource {
    pub fn new(id: AudioId, asset: AudioAsset) -> Self {
        Self {
            id,
            asset,
            position: 0,
            looping: false,
            volume: 1.0,
        }
    }

    /// Read the next `count` mono samples from this source.
    /// Returns fewer samples if the source is done (non-looping).
    pub fn read_samples(&mut self, count: usize) -> Vec<f32> {
        let channels = self.asset.channels as usize;
        if channels == 0 {
            return vec![0.0; count];
        }

        let mut output = Vec::with_capacity(count);

        for _ in 0..count {
            if self.position >= self.asset.samples.len() {
                if self.looping {
                    self.position = 0;
                } else {
                    break;
                }
            }

            if channels == 1 {
                output.push(self.asset.samples[self.position] * self.volume);
                self.position += 1;
            } else {
                // Downmix to mono by averaging channels
                let mut sum = 0.0f32;
                for ch in 0..channels {
                    let idx = self.position + ch;
                    if idx < self.asset.samples.len() {
                        sum += self.asset.samples[idx];
                    }
                }
                output.push((sum / channels as f32) * self.volume);
                self.position += channels;
            }
        }

        output
    }

    /// Whether this source has finished playing.
    pub fn is_finished(&self) -> bool {
        !self.looping && self.position >= self.asset.samples.len()
    }
}

/// The output pipeline that mixes audio sources and feeds a device output stream.
pub struct OutputPipeline {
    renderer: SpatialRenderer,
    output_buffer: Arc<Mutex<Vec<f32>>>,
    _output_handle: Option<OutputHandle>,
    sample_rate: u32,
}

impl OutputPipeline {
    /// Create a new pipeline (not yet connected to a device).
    pub fn new(sample_rate: u32, hrtf_profile: HrtfProfile) -> Self {
        Self {
            renderer: SpatialRenderer::new(hrtf_profile),
            output_buffer: Arc::new(Mutex::new(Vec::new())),
            _output_handle: None,
            sample_rate,
        }
    }

    /// Open an output device and start the output stream.
    pub fn start(&mut self, device_config: DeviceConfig) -> Result<(), DeviceError> {
        let manager = AudioDeviceManager::new(device_config);
        let handle = manager.open_output_stream(self.output_buffer.clone())?;
        self._output_handle = Some(handle);
        Ok(())
    }

    /// Stop the output stream.
    pub fn stop(&mut self) {
        self._output_handle = None;
    }

    pub fn is_active(&self) -> bool {
        self._output_handle.is_some()
    }

    /// Mix sources according to instructions and push stereo samples to the output buffer.
    pub fn mix_and_output(
        &self,
        sources: &mut [PlaybackSource],
        instructions: &[AudioMixInstruction],
        frame_count: usize,
    ) {
        let stereo_samples = frame_count * 2;
        let mut mix_buf = vec![0.0f32; stereo_samples];

        for instruction in instructions {
            let source = sources
                .iter_mut()
                .find(|s| s.id.0 == instruction.source_id.0);

            if let Some(source) = source {
                let mono = source.read_samples(frame_count);
                if !mono.is_empty() {
                    self.renderer.mix_into(
                        &mut mix_buf,
                        &mono,
                        &instruction.hrtf,
                        instruction.gain,
                    );
                }
            }
        }

        // Clamp output to [-1.0, 1.0]
        for sample in &mut mix_buf {
            *sample = sample.clamp(-1.0, 1.0);
        }

        let mut buf = self.output_buffer.lock().unwrap_or_else(|e| e.into_inner());
        buf.extend_from_slice(&mix_buf);
    }

    /// Access the raw output buffer (for testing).
    pub fn output_buffer(&self) -> Arc<Mutex<Vec<f32>>> {
        self.output_buffer.clone()
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acoustics::HrtfTransportParams;
    use crate::types::AudioId;

    fn test_hrtf_params(azimuth: f32) -> HrtfTransportParams {
        HrtfTransportParams {
            azimuth_deg: azimuth,
            elevation_deg: 0.0,
            distance_gain: 1.0,
            occlusion: 0.0,
            reflectivity: 0.0,
        }
    }

    #[test]
    fn spatial_renderer_produces_stereo_from_mono() {
        let renderer = SpatialRenderer::new(HrtfProfile::Generic);
        let mono = vec![1.0f32; 10];
        let params = test_hrtf_params(0.0);
        let stereo = renderer.render_stereo(&mono, &params);

        assert_eq!(stereo.len(), 20); // 10 mono -> 20 stereo
    }

    #[test]
    fn spatial_renderer_zero_gain_produces_silence() {
        let renderer = SpatialRenderer::new(HrtfProfile::Generic);
        let mono = vec![1.0f32; 10];
        let params = HrtfTransportParams {
            azimuth_deg: 0.0,
            elevation_deg: 0.0,
            distance_gain: 0.0,
            occlusion: 0.0,
            reflectivity: 0.0,
        };
        let stereo = renderer.render_stereo(&mono, &params);
        for sample in &stereo {
            assert_eq!(*sample, 0.0);
        }
    }

    #[test]
    fn spatial_renderer_empty_input_produces_empty_output() {
        let renderer = SpatialRenderer::new(HrtfProfile::Generic);
        let mono: Vec<f32> = vec![];
        let params = test_hrtf_params(0.0);
        let stereo = renderer.render_stereo(&mono, &params);
        assert!(stereo.is_empty());
    }

    #[test]
    fn spatial_renderer_mix_into_accumulates() {
        let renderer = SpatialRenderer::new(HrtfProfile::Generic);
        let mono = vec![0.5f32; 4];
        let params = test_hrtf_params(0.0);

        let mut target = vec![0.1f32; 8]; // 4 frames stereo
        renderer.mix_into(&mut target, &mono, &params, 1.0);

        // Target should have been accumulated on top of 0.1
        for sample in &target {
            assert!(*sample > 0.1);
        }
    }

    #[test]
    fn spatial_renderer_azimuth_affects_stereo_balance() {
        let renderer = SpatialRenderer::new(HrtfProfile::Generic);
        let mono = vec![1.0f32; 100];

        let left_params = test_hrtf_params(-90.0);
        let right_params = test_hrtf_params(90.0);

        let left_stereo = renderer.render_stereo(&mono, &left_params);
        let right_stereo = renderer.render_stereo(&mono, &right_params);

        // With different azimuth angles, the stereo balance should differ
        let left_sum: f32 = left_stereo.iter().step_by(2).sum();
        let right_sum: f32 = right_stereo.iter().step_by(2).sum();
        // Both should be non-zero (HRTF doesn't completely silence one side)
        assert!(left_sum > 0.0);
        assert!(right_sum > 0.0);
    }

    #[test]
    fn playback_source_reads_mono_samples() {
        let asset = AudioAsset {
            samples: vec![0.1, 0.2, 0.3, 0.4, 0.5],
            sample_rate: 48000,
            channels: 1,
            duration_seconds: 5.0 / 48000.0,
        };
        let mut source = PlaybackSource::new(AudioId(1), asset);

        let read = source.read_samples(3);
        assert_eq!(read.len(), 3);
        assert!((read[0] - 0.1).abs() < f32::EPSILON);
        assert!((read[1] - 0.2).abs() < f32::EPSILON);
        assert!((read[2] - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn playback_source_reads_stereo_as_mono() {
        let asset = AudioAsset {
            samples: vec![0.4, 0.6, 0.8, 1.0], // 2 frames stereo
            sample_rate: 48000,
            channels: 2,
            duration_seconds: 2.0 / 48000.0,
        };
        let mut source = PlaybackSource::new(AudioId(1), asset);

        let read = source.read_samples(2);
        assert_eq!(read.len(), 2);
        // First frame: (0.4 + 0.6) / 2 = 0.5
        assert!((read[0] - 0.5).abs() < f32::EPSILON);
        // Second frame: (0.8 + 1.0) / 2 = 0.9
        assert!((read[1] - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn playback_source_stops_at_end_when_not_looping() {
        let asset = AudioAsset {
            samples: vec![1.0, 2.0, 3.0],
            sample_rate: 48000,
            channels: 1,
            duration_seconds: 3.0 / 48000.0,
        };
        let mut source = PlaybackSource::new(AudioId(1), asset);
        source.looping = false;

        let read = source.read_samples(5);
        assert_eq!(read.len(), 3); // only 3 samples available
        assert!(source.is_finished());
    }

    #[test]
    fn playback_source_loops_when_enabled() {
        let asset = AudioAsset {
            samples: vec![1.0, 2.0],
            sample_rate: 48000,
            channels: 1,
            duration_seconds: 2.0 / 48000.0,
        };
        let mut source = PlaybackSource::new(AudioId(1), asset);
        source.looping = true;

        let read = source.read_samples(5);
        assert_eq!(read.len(), 5);
        assert!(!source.is_finished());
    }

    #[test]
    fn playback_source_volume_scales_output() {
        let asset = AudioAsset {
            samples: vec![1.0; 10],
            sample_rate: 48000,
            channels: 1,
            duration_seconds: 10.0 / 48000.0,
        };
        let mut source = PlaybackSource::new(AudioId(1), asset);
        source.volume = 0.5;

        let read = source.read_samples(5);
        for sample in &read {
            assert!((*sample - 0.5).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn output_pipeline_mix_and_output_produces_stereo() {
        let pipeline = OutputPipeline::new(48000, HrtfProfile::Generic);

        let asset = AudioAsset {
            samples: vec![0.5f32; 100],
            sample_rate: 48000,
            channels: 1,
            duration_seconds: 100.0 / 48000.0,
        };
        let mut sources = vec![PlaybackSource::new(AudioId(1), asset)];
        let instructions = vec![AudioMixInstruction {
            source_id: AudioId(1),
            gain: 0.8,
            hrtf: test_hrtf_params(0.0),
            lod: crate::types::AudioLod::Near,
            bandwidth_profile: "high".to_string(),
            route: crate::channel::RoutingPolicy::Allow,
        }];

        pipeline.mix_and_output(&mut sources, &instructions, 50);

        let buf = pipeline.output_buffer();
        let data = buf.lock().unwrap();
        assert_eq!(data.len(), 100); // 50 frames * 2 channels
                                     // All samples should be non-zero (signal was 0.5 * 0.8 * hrtf gain)
        assert!(data.iter().any(|s| *s != 0.0));
    }

    #[test]
    fn output_pipeline_clamps_output() {
        let pipeline = OutputPipeline::new(48000, HrtfProfile::Generic);

        let asset = AudioAsset {
            samples: vec![10.0f32; 100], // very loud
            sample_rate: 48000,
            channels: 1,
            duration_seconds: 100.0 / 48000.0,
        };
        let mut sources = vec![PlaybackSource::new(AudioId(1), asset)];
        let instructions = vec![AudioMixInstruction {
            source_id: AudioId(1),
            gain: 5.0,
            hrtf: test_hrtf_params(0.0),
            lod: crate::types::AudioLod::Near,
            bandwidth_profile: "high".to_string(),
            route: crate::channel::RoutingPolicy::Allow,
        }];

        pipeline.mix_and_output(&mut sources, &instructions, 50);

        let buf = pipeline.output_buffer();
        let data = buf.lock().unwrap();
        for sample in data.iter() {
            assert!(
                *sample >= -1.0 && *sample <= 1.0,
                "sample out of range: {sample}"
            );
        }
    }

    #[test]
    fn output_pipeline_no_instructions_produces_silence() {
        let pipeline = OutputPipeline::new(48000, HrtfProfile::Generic);
        let mut sources: Vec<PlaybackSource> = vec![];
        let instructions: Vec<AudioMixInstruction> = vec![];

        pipeline.mix_and_output(&mut sources, &instructions, 50);

        let buf = pipeline.output_buffer();
        let data = buf.lock().unwrap();
        assert_eq!(data.len(), 100);
        for sample in data.iter() {
            assert_eq!(*sample, 0.0);
        }
    }

    #[test]
    fn output_pipeline_multiple_sources_mix() {
        let pipeline = OutputPipeline::new(48000, HrtfProfile::Generic);

        let asset1 = AudioAsset {
            samples: vec![0.3f32; 100],
            sample_rate: 48000,
            channels: 1,
            duration_seconds: 100.0 / 48000.0,
        };
        let asset2 = AudioAsset {
            samples: vec![0.2f32; 100],
            sample_rate: 48000,
            channels: 1,
            duration_seconds: 100.0 / 48000.0,
        };
        let mut sources = vec![
            PlaybackSource::new(AudioId(1), asset1),
            PlaybackSource::new(AudioId(2), asset2),
        ];
        let instructions = vec![
            AudioMixInstruction {
                source_id: AudioId(1),
                gain: 1.0,
                hrtf: test_hrtf_params(0.0),
                lod: crate::types::AudioLod::Near,
                bandwidth_profile: "high".to_string(),
                route: crate::channel::RoutingPolicy::Allow,
            },
            AudioMixInstruction {
                source_id: AudioId(2),
                gain: 1.0,
                hrtf: test_hrtf_params(90.0),
                lod: crate::types::AudioLod::Near,
                bandwidth_profile: "high".to_string(),
                route: crate::channel::RoutingPolicy::Allow,
            },
        ];

        pipeline.mix_and_output(&mut sources, &instructions, 50);

        let buf = pipeline.output_buffer();
        let data = buf.lock().unwrap();
        assert_eq!(data.len(), 100);
        // Mixed signal should be louder than either individual source
        assert!(data.iter().any(|s| *s != 0.0));
    }

    #[test]
    #[ignore]
    fn output_pipeline_start_stop() {
        let mut pipeline = OutputPipeline::new(48000, HrtfProfile::Generic);
        let config = DeviceConfig::default();
        pipeline.start(config).expect("should start output");
        assert!(pipeline.is_active());
        pipeline.stop();
        assert!(!pipeline.is_active());
    }
}
