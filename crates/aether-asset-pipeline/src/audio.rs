//! Audio encoding abstractions for WAV-to-Opus conversion.

/// Supported audio codecs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioCodec {
    Opus,
    Vorbis,
    Pcm,
}

/// Raw audio input data (PCM).
#[derive(Debug, Clone)]
pub struct AudioInput {
    /// Interleaved PCM samples as 16-bit signed integers.
    pub samples: Vec<i16>,
    pub sample_rate: u32,
    pub channels: u16,
    pub name: String,
}

impl AudioInput {
    pub fn new(name: String, samples: Vec<i16>, sample_rate: u32, channels: u16) -> Self {
        Self {
            samples,
            sample_rate,
            channels,
            name,
        }
    }

    /// Duration of the audio in seconds.
    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 || self.channels == 0 {
            return 0.0;
        }
        self.samples.len() as f64 / (self.sample_rate as f64 * self.channels as f64)
    }

    /// Size of raw PCM data in bytes.
    pub fn size_bytes(&self) -> u64 {
        (self.samples.len() * std::mem::size_of::<i16>()) as u64
    }
}

/// Encoded audio data with codec metadata.
#[derive(Debug, Clone)]
pub struct EncodedAudio {
    pub data: Vec<u8>,
    pub codec: AudioCodec,
    pub sample_rate: u32,
    pub channels: u16,
    pub name: String,
}

impl EncodedAudio {
    /// Size of the encoded audio data in bytes.
    pub fn size_bytes(&self) -> u64 {
        self.data.len() as u64
    }
}

/// Errors during audio encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioError {
    UnsupportedCodec,
    InvalidInput(String),
    EncodingFailed(String),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::UnsupportedCodec => write!(f, "unsupported audio codec"),
            AudioError::InvalidInput(msg) => write!(f, "invalid audio input: {}", msg),
            AudioError::EncodingFailed(msg) => write!(f, "audio encoding failed: {}", msg),
        }
    }
}

impl std::error::Error for AudioError {}

/// Trait for audio encoding backends.
pub trait AudioEncoder {
    /// Encode PCM audio to the specified codec.
    fn encode(&self, input: &AudioInput, codec: AudioCodec) -> Result<EncodedAudio, AudioError>;
}

/// Built-in passthrough audio encoder for testing.
///
/// Converts PCM samples to raw bytes with a codec tag.
/// Real Opus encoding should be provided via feature-gated backends.
pub struct PassthroughEncoder;

impl AudioEncoder for PassthroughEncoder {
    fn encode(&self, input: &AudioInput, codec: AudioCodec) -> Result<EncodedAudio, AudioError> {
        if input.samples.is_empty() {
            return Err(AudioError::InvalidInput("empty samples".to_string()));
        }

        // Convert i16 samples to raw bytes
        let data: Vec<u8> = input.samples.iter().flat_map(|s| s.to_le_bytes()).collect();

        Ok(EncodedAudio {
            data,
            codec,
            sample_rate: input.sample_rate,
            channels: input.channels,
            name: input.name.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sine_samples(count: usize) -> Vec<i16> {
        (0..count)
            .map(|i| {
                let t = i as f64 / 44100.0;
                (f64::sin(t * 440.0 * 2.0 * std::f64::consts::PI) * 16000.0) as i16
            })
            .collect()
    }

    #[test]
    fn audio_input_creation() {
        let samples = vec![0i16; 44100];
        let input = AudioInput::new("test".to_string(), samples, 44100, 1);
        assert_eq!(input.sample_rate, 44100);
        assert_eq!(input.channels, 1);
        assert_eq!(input.name, "test");
    }

    #[test]
    fn audio_input_duration_mono() {
        let samples = vec![0i16; 44100]; // 1 second at 44.1kHz mono
        let input = AudioInput::new("test".to_string(), samples, 44100, 1);
        let duration = input.duration_secs();
        assert!((duration - 1.0).abs() < 0.001);
    }

    #[test]
    fn audio_input_duration_stereo() {
        let samples = vec![0i16; 88200]; // 1 second at 44.1kHz stereo
        let input = AudioInput::new("test".to_string(), samples, 44100, 2);
        let duration = input.duration_secs();
        assert!((duration - 1.0).abs() < 0.001);
    }

    #[test]
    fn audio_input_duration_zero_rate() {
        let input = AudioInput::new("test".to_string(), vec![0; 100], 0, 1);
        assert_eq!(input.duration_secs(), 0.0);
    }

    #[test]
    fn audio_input_duration_zero_channels() {
        let input = AudioInput::new("test".to_string(), vec![0; 100], 44100, 0);
        assert_eq!(input.duration_secs(), 0.0);
    }

    #[test]
    fn audio_input_size_bytes() {
        let samples = vec![0i16; 1000];
        let input = AudioInput::new("test".to_string(), samples, 44100, 1);
        assert_eq!(input.size_bytes(), 2000); // 1000 samples * 2 bytes
    }

    #[test]
    fn passthrough_encoder_opus() {
        let samples = make_sine_samples(4410);
        let input = AudioInput::new("sine".to_string(), samples, 44100, 1);
        let encoder = PassthroughEncoder;
        let result = encoder.encode(&input, AudioCodec::Opus);
        assert!(result.is_ok());
        let encoded = result.unwrap();
        assert_eq!(encoded.codec, AudioCodec::Opus);
        assert_eq!(encoded.sample_rate, 44100);
        assert_eq!(encoded.channels, 1);
        assert_eq!(encoded.name, "sine");
    }

    #[test]
    fn passthrough_encoder_vorbis() {
        let samples = make_sine_samples(1000);
        let input = AudioInput::new("test".to_string(), samples, 48000, 2);
        let encoder = PassthroughEncoder;
        let result = encoder.encode(&input, AudioCodec::Vorbis);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().codec, AudioCodec::Vorbis);
    }

    #[test]
    fn passthrough_encoder_pcm() {
        let samples = make_sine_samples(100);
        let input = AudioInput::new("test".to_string(), samples, 44100, 1);
        let encoder = PassthroughEncoder;
        let result = encoder.encode(&input, AudioCodec::Pcm);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().codec, AudioCodec::Pcm);
    }

    #[test]
    fn passthrough_encoder_empty_input_fails() {
        let input = AudioInput::new("empty".to_string(), vec![], 44100, 1);
        let encoder = PassthroughEncoder;
        let result = encoder.encode(&input, AudioCodec::Opus);
        assert!(result.is_err());
        match result {
            Err(AudioError::InvalidInput(msg)) => assert!(msg.contains("empty")),
            _ => panic!("expected InvalidInput error"),
        }
    }

    #[test]
    fn passthrough_encoder_data_size() {
        let samples = vec![1000i16; 500];
        let input = AudioInput::new("test".to_string(), samples, 44100, 1);
        let encoder = PassthroughEncoder;
        let encoded = encoder.encode(&input, AudioCodec::Opus).unwrap();
        // Passthrough converts i16 -> bytes, so 500 samples -> 1000 bytes
        assert_eq!(encoded.size_bytes(), 1000);
    }

    #[test]
    fn encoded_audio_size_bytes() {
        let encoded = EncodedAudio {
            data: vec![0u8; 256],
            codec: AudioCodec::Opus,
            sample_rate: 48000,
            channels: 2,
            name: "test".to_string(),
        };
        assert_eq!(encoded.size_bytes(), 256);
    }

    #[test]
    fn audio_error_display() {
        let err = AudioError::UnsupportedCodec;
        assert!(format!("{}", err).contains("unsupported"));

        let err = AudioError::InvalidInput("bad data".to_string());
        assert!(format!("{}", err).contains("bad data"));

        let err = AudioError::EncodingFailed("timeout".to_string());
        assert!(format!("{}", err).contains("timeout"));
    }

    #[test]
    fn audio_codec_equality() {
        assert_eq!(AudioCodec::Opus, AudioCodec::Opus);
        assert_ne!(AudioCodec::Opus, AudioCodec::Vorbis);
        assert_ne!(AudioCodec::Vorbis, AudioCodec::Pcm);
    }

    #[test]
    fn passthrough_preserves_sample_rate_and_channels() {
        let samples = make_sine_samples(100);
        let input = AudioInput::new("test".to_string(), samples, 48000, 2);
        let encoder = PassthroughEncoder;
        let encoded = encoder.encode(&input, AudioCodec::Opus).unwrap();
        assert_eq!(encoded.sample_rate, 48000);
        assert_eq!(encoded.channels, 2);
    }
}
