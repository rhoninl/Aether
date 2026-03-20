use crate::opus::OpusConfig;

/// Errors from codec encode/decode operations.
#[derive(Debug)]
pub enum CodecEncodeError {
    InsufficientSamples { expected: usize, got: usize },
    EncodeFailed(String),
    DecodeFailed(String),
    InvalidConfig(String),
}

impl std::fmt::Display for CodecEncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecEncodeError::InsufficientSamples { expected, got } => {
                write!(f, "need {expected} samples, got {got}")
            }
            CodecEncodeError::EncodeFailed(msg) => write!(f, "encode failed: {msg}"),
            CodecEncodeError::DecodeFailed(msg) => write!(f, "decode failed: {msg}"),
            CodecEncodeError::InvalidConfig(msg) => write!(f, "invalid config: {msg}"),
        }
    }
}

impl std::error::Error for CodecEncodeError {}

/// Trait for audio codecs that can encode and decode PCM samples.
pub trait AudioCodec: Send {
    /// Encode PCM f32 samples into compressed bytes.
    fn encode(&mut self, pcm: &[f32]) -> Result<Vec<u8>, CodecEncodeError>;

    /// Decode compressed bytes back into PCM f32 samples.
    fn decode(&mut self, data: &[u8]) -> Result<Vec<f32>, CodecEncodeError>;

    /// Number of samples expected per encode call (frame size).
    fn frame_size(&self) -> usize;

    /// Sample rate of the codec.
    fn sample_rate(&self) -> u32;
}

/// A stub codec that converts f32 PCM to/from raw bytes without compression.
/// Used for testing and as a fallback when real Opus is not available.
pub struct StubCodec {
    config: OpusConfig,
    frame_samples: usize,
}

impl StubCodec {
    pub fn new(config: OpusConfig) -> Self {
        let frame_samples = (config.sample_rate_hz as usize * config.frame_ms as usize) / 1000;
        Self {
            config,
            frame_samples,
        }
    }

    pub fn from_default() -> Self {
        Self::new(OpusConfig::opus_voice_default())
    }
}

impl AudioCodec for StubCodec {
    fn encode(&mut self, pcm: &[f32]) -> Result<Vec<u8>, CodecEncodeError> {
        if pcm.len() < self.frame_samples {
            return Err(CodecEncodeError::InsufficientSamples {
                expected: self.frame_samples,
                got: pcm.len(),
            });
        }

        let samples = &pcm[..self.frame_samples];
        let mut bytes = Vec::with_capacity(samples.len() * 4);
        for &sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        Ok(bytes)
    }

    fn decode(&mut self, data: &[u8]) -> Result<Vec<f32>, CodecEncodeError> {
        if !data.len().is_multiple_of(4) {
            return Err(CodecEncodeError::DecodeFailed(
                "byte length not aligned to f32".to_string(),
            ));
        }

        let mut samples = Vec::with_capacity(data.len() / 4);
        for chunk in data.chunks_exact(4) {
            let bytes: [u8; 4] = chunk.try_into().unwrap();
            samples.push(f32::from_le_bytes(bytes));
        }
        Ok(samples)
    }

    fn frame_size(&self) -> usize {
        self.frame_samples
    }

    fn sample_rate(&self) -> u32 {
        self.config.sample_rate_hz
    }
}

/// Create a codec instance from an OpusConfig. Currently returns the stub codec.
/// When a real Opus implementation is available, this can be swapped.
pub fn create_codec(config: OpusConfig) -> Box<dyn AudioCodec> {
    Box::new(StubCodec::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_codec_roundtrip_preserves_samples() {
        let mut codec = StubCodec::from_default();
        let frame_size = codec.frame_size();

        let input: Vec<f32> = (0..frame_size)
            .map(|i| (i as f32) / frame_size as f32)
            .collect();
        let encoded = codec.encode(&input).unwrap();
        let decoded = codec.decode(&encoded).unwrap();

        assert_eq!(decoded.len(), frame_size);
        for (original, decoded_val) in input.iter().zip(decoded.iter()) {
            assert!(
                (original - decoded_val).abs() < f32::EPSILON,
                "sample mismatch: {original} vs {decoded_val}"
            );
        }
    }

    #[test]
    fn stub_codec_encode_rejects_short_input() {
        let mut codec = StubCodec::from_default();
        let too_short = vec![0.0f32; 10];
        let result = codec.encode(&too_short);
        assert!(result.is_err());

        if let Err(CodecEncodeError::InsufficientSamples { expected, got }) = result {
            assert_eq!(got, 10);
            assert!(expected > 10);
        } else {
            panic!("expected InsufficientSamples error");
        }
    }

    #[test]
    fn stub_codec_decode_rejects_misaligned_bytes() {
        let mut codec = StubCodec::from_default();
        let bad_data = vec![0u8; 7]; // not a multiple of 4
        let result = codec.decode(&bad_data);
        assert!(result.is_err());
    }

    #[test]
    fn stub_codec_frame_size_matches_config() {
        let config = OpusConfig {
            sample_rate_hz: 48_000,
            frame_ms: 20,
            bitrate_kbps: crate::opus::BitRateKbps::Kbps32,
            inband_fec: true,
            use_hardware_accel: false,
        };
        let codec = StubCodec::new(config);
        // 48000 * 20 / 1000 = 960 samples per frame
        assert_eq!(codec.frame_size(), 960);
    }

    #[test]
    fn stub_codec_sample_rate_matches_config() {
        let config = OpusConfig::opus_voice_default();
        let codec = StubCodec::new(config);
        assert_eq!(codec.sample_rate(), 48_000);
    }

    #[test]
    fn create_codec_returns_working_codec() {
        let config = OpusConfig::opus_voice_default();
        let mut codec = create_codec(config);
        let frame_size = codec.frame_size();

        let input: Vec<f32> = vec![0.5; frame_size];
        let encoded = codec.encode(&input).unwrap();
        let decoded = codec.decode(&encoded).unwrap();

        assert_eq!(decoded.len(), frame_size);
        assert!((decoded[0] - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn stub_codec_encode_uses_only_frame_size_samples() {
        let mut codec = StubCodec::from_default();
        let frame_size = codec.frame_size();

        // Provide more samples than a frame
        let input: Vec<f32> = vec![1.0; frame_size + 100];
        let encoded = codec.encode(&input).unwrap();
        let decoded = codec.decode(&encoded).unwrap();

        // Only frame_size samples should be encoded
        assert_eq!(decoded.len(), frame_size);
    }

    #[test]
    fn stub_codec_handles_silence() {
        let mut codec = StubCodec::from_default();
        let frame_size = codec.frame_size();
        let silence: Vec<f32> = vec![0.0; frame_size];

        let encoded = codec.encode(&silence).unwrap();
        let decoded = codec.decode(&encoded).unwrap();

        assert_eq!(decoded.len(), frame_size);
        for sample in &decoded {
            assert_eq!(*sample, 0.0);
        }
    }

    #[test]
    fn stub_codec_handles_extreme_values() {
        let mut codec = StubCodec::from_default();
        let frame_size = codec.frame_size();
        let mut input = vec![0.0f32; frame_size];
        input[0] = f32::MAX;
        input[1] = f32::MIN;
        input[2] = -1.0;
        input[3] = 1.0;

        let encoded = codec.encode(&input).unwrap();
        let decoded = codec.decode(&encoded).unwrap();

        assert_eq!(decoded[0], f32::MAX);
        assert_eq!(decoded[1], f32::MIN);
        assert_eq!(decoded[2], -1.0);
        assert_eq!(decoded[3], 1.0);
    }
}
