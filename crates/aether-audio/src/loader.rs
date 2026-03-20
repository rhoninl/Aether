use std::path::Path;

const MAX_AUDIO_DURATION_SECONDS: f32 = 600.0; // 10 minutes max

/// Errors that can occur when loading audio assets.
#[derive(Debug)]
pub enum LoadError {
    IoError(std::io::Error),
    UnsupportedFormat(String),
    InvalidData(String),
    TooLong { duration_seconds: f32 },
    DecodeFailed(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::IoError(e) => write!(f, "I/O error: {e}"),
            LoadError::UnsupportedFormat(ext) => write!(f, "unsupported format: {ext}"),
            LoadError::InvalidData(msg) => write!(f, "invalid audio data: {msg}"),
            LoadError::TooLong { duration_seconds } => {
                write!(
                    f,
                    "audio too long: {duration_seconds}s exceeds max {MAX_AUDIO_DURATION_SECONDS}s"
                )
            }
            LoadError::DecodeFailed(msg) => write!(f, "decode failed: {msg}"),
        }
    }
}

impl std::error::Error for LoadError {}

impl From<std::io::Error> for LoadError {
    fn from(e: std::io::Error) -> Self {
        LoadError::IoError(e)
    }
}

/// A loaded audio asset with PCM sample data.
#[derive(Debug, Clone)]
pub struct AudioAsset {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_seconds: f32,
}

impl AudioAsset {
    /// Number of sample frames (samples / channels).
    pub fn frame_count(&self) -> usize {
        if self.channels == 0 {
            return 0;
        }
        self.samples.len() / self.channels as usize
    }

    /// Resample to a target sample rate using linear interpolation.
    /// Returns a new AudioAsset at the target rate.
    pub fn resample(&self, target_rate: u32) -> Self {
        if self.sample_rate == target_rate || self.sample_rate == 0 {
            return self.clone();
        }

        let ratio = target_rate as f64 / self.sample_rate as f64;
        let channels = self.channels as usize;
        let input_frames = self.frame_count();
        let output_frames = (input_frames as f64 * ratio).ceil() as usize;

        let mut output = Vec::with_capacity(output_frames * channels);

        for frame_idx in 0..output_frames {
            let src_pos = frame_idx as f64 / ratio;
            let src_frame = src_pos.floor() as usize;
            let frac = (src_pos - src_frame as f64) as f32;

            for ch in 0..channels {
                let idx0 = src_frame * channels + ch;
                let idx1 = ((src_frame + 1).min(input_frames.saturating_sub(1))) * channels + ch;

                let s0 = self.samples.get(idx0).copied().unwrap_or(0.0);
                let s1 = self.samples.get(idx1).copied().unwrap_or(0.0);

                output.push(s0 + (s1 - s0) * frac);
            }
        }

        let duration_seconds = if target_rate > 0 {
            output_frames as f32 / target_rate as f32
        } else {
            0.0
        };

        AudioAsset {
            samples: output,
            sample_rate: target_rate,
            channels: self.channels,
            duration_seconds,
        }
    }
}

/// Load a WAV file from disk.
pub fn load_wav<P: AsRef<Path>>(path: P) -> Result<AudioAsset, LoadError> {
    let reader = hound::WavReader::open(path.as_ref()).map_err(|e| {
        if matches!(e, hound::Error::IoError(_)) {
            LoadError::IoError(std::io::Error::other(e.to_string()))
        } else {
            LoadError::InvalidData(e.to_string())
        }
    })?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels;
    let total_samples = reader.len() as usize;
    let frame_count = total_samples / channels as usize;
    let duration_seconds = frame_count as f32 / sample_rate as f32;

    if duration_seconds > MAX_AUDIO_DURATION_SECONDS {
        return Err(LoadError::TooLong { duration_seconds });
    }

    let samples = read_wav_samples(reader, spec)?;

    Ok(AudioAsset {
        samples,
        sample_rate,
        channels,
        duration_seconds,
    })
}

fn read_wav_samples(
    mut reader: hound::WavReader<std::io::BufReader<std::fs::File>>,
    spec: hound::WavSpec,
) -> Result<Vec<f32>, LoadError> {
    match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1u64 << (spec.bits_per_sample - 1)) as f32;
            let samples: Result<Vec<f32>, _> = reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_val))
                .collect();
            samples.map_err(|e| LoadError::InvalidData(e.to_string()))
        }
        hound::SampleFormat::Float => {
            let samples: Result<Vec<f32>, _> = reader.samples::<f32>().collect();
            samples.map_err(|e| LoadError::InvalidData(e.to_string()))
        }
    }
}

/// Load a WAV file from an in-memory byte slice.
pub fn load_wav_from_bytes(data: &[u8]) -> Result<AudioAsset, LoadError> {
    let cursor = std::io::Cursor::new(data);
    let reader =
        hound::WavReader::new(cursor).map_err(|e| LoadError::InvalidData(e.to_string()))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels;
    let total_samples = reader.len() as usize;
    let frame_count = total_samples / channels as usize;
    let duration_seconds = frame_count as f32 / sample_rate as f32;

    if duration_seconds > MAX_AUDIO_DURATION_SECONDS {
        return Err(LoadError::TooLong { duration_seconds });
    }

    let samples = read_wav_samples_cursor(reader, spec)?;

    Ok(AudioAsset {
        samples,
        sample_rate,
        channels,
        duration_seconds,
    })
}

fn read_wav_samples_cursor(
    mut reader: hound::WavReader<std::io::Cursor<&[u8]>>,
    spec: hound::WavSpec,
) -> Result<Vec<f32>, LoadError> {
    match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1u64 << (spec.bits_per_sample - 1)) as f32;
            let samples: Result<Vec<f32>, _> = reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_val))
                .collect();
            samples.map_err(|e| LoadError::InvalidData(e.to_string()))
        }
        hound::SampleFormat::Float => {
            let samples: Result<Vec<f32>, _> = reader.samples::<f32>().collect();
            samples.map_err(|e| LoadError::InvalidData(e.to_string()))
        }
    }
}

/// Load an OGG/Vorbis file from disk.
pub fn load_ogg<P: AsRef<Path>>(path: P) -> Result<AudioAsset, LoadError> {
    let file = std::fs::File::open(path.as_ref()).map_err(LoadError::IoError)?;
    let reader = std::io::BufReader::new(file);

    decode_ogg_from_reader(reader)
}

/// Load an OGG/Vorbis file from an in-memory byte slice.
pub fn load_ogg_from_bytes(data: &[u8]) -> Result<AudioAsset, LoadError> {
    let reader = std::io::Cursor::new(data);
    decode_ogg_from_reader(reader)
}

fn decode_ogg_from_reader<R: std::io::Read + std::io::Seek>(
    reader: R,
) -> Result<AudioAsset, LoadError> {
    let mut ogg_reader = lewton::inside_ogg::OggStreamReader::new(reader)
        .map_err(|e| LoadError::DecodeFailed(format!("{e:?}")))?;

    let sample_rate = ogg_reader.ident_hdr.audio_sample_rate;
    let channels = ogg_reader.ident_hdr.audio_channels as u16;

    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        match ogg_reader.read_dec_packet_itl() {
            Ok(Some(packet)) => {
                for sample in packet {
                    // lewton returns i16-range samples as i16 values
                    all_samples.push(sample as f32 / 32768.0);
                }
            }
            Ok(None) => break,
            Err(e) => return Err(LoadError::DecodeFailed(format!("{e:?}"))),
        }
    }

    let frame_count = if channels > 0 {
        all_samples.len() / channels as usize
    } else {
        0
    };
    let duration_seconds = frame_count as f32 / sample_rate as f32;

    if duration_seconds > MAX_AUDIO_DURATION_SECONDS {
        return Err(LoadError::TooLong { duration_seconds });
    }

    Ok(AudioAsset {
        samples: all_samples,
        sample_rate,
        channels,
        duration_seconds,
    })
}

/// Load an audio file, detecting format by file extension.
pub fn load_auto<P: AsRef<Path>>(path: P) -> Result<AudioAsset, LoadError> {
    let path = path.as_ref();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "wav" => load_wav(path),
        "ogg" => load_ogg(path),
        other => Err(LoadError::UnsupportedFormat(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal valid WAV file in memory (16-bit PCM, mono, 48kHz).
    fn create_test_wav(samples: &[i16], sample_rate: u32, channels: u16) -> Vec<u8> {
        let mut buf = Vec::new();
        let cursor = std::io::Cursor::new(&mut buf);
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(cursor, spec).unwrap();
        for &s in samples {
            writer.write_sample(s).unwrap();
        }
        writer.finalize().unwrap();
        buf
    }

    #[test]
    fn load_wav_from_bytes_mono() {
        let samples: Vec<i16> = (0..480)
            .map(|i| ((i as f32 / 480.0) * 32767.0) as i16)
            .collect();
        let wav_data = create_test_wav(&samples, 48000, 1);

        let asset = load_wav_from_bytes(&wav_data).unwrap();
        assert_eq!(asset.sample_rate, 48000);
        assert_eq!(asset.channels, 1);
        assert_eq!(asset.samples.len(), 480);
        assert!(asset.duration_seconds > 0.0);
    }

    #[test]
    fn load_wav_from_bytes_stereo() {
        // 240 frames x 2 channels = 480 samples
        let samples: Vec<i16> = (0..480).map(|i| (i as i16) % 100).collect();
        let wav_data = create_test_wav(&samples, 44100, 2);

        let asset = load_wav_from_bytes(&wav_data).unwrap();
        assert_eq!(asset.sample_rate, 44100);
        assert_eq!(asset.channels, 2);
        assert_eq!(asset.samples.len(), 480);
        assert_eq!(asset.frame_count(), 240);
    }

    #[test]
    fn load_wav_from_bytes_rejects_invalid_data() {
        let bad_data = vec![0u8; 100];
        let result = load_wav_from_bytes(&bad_data);
        assert!(result.is_err());
    }

    #[test]
    fn audio_asset_frame_count() {
        let asset = AudioAsset {
            samples: vec![0.0; 960],
            sample_rate: 48000,
            channels: 2,
            duration_seconds: 0.01,
        };
        assert_eq!(asset.frame_count(), 480);
    }

    #[test]
    fn audio_asset_frame_count_zero_channels() {
        let asset = AudioAsset {
            samples: vec![0.0; 100],
            sample_rate: 48000,
            channels: 0,
            duration_seconds: 0.0,
        };
        assert_eq!(asset.frame_count(), 0);
    }

    #[test]
    fn resample_same_rate_is_identity() {
        let asset = AudioAsset {
            samples: vec![1.0, 2.0, 3.0, 4.0],
            sample_rate: 48000,
            channels: 1,
            duration_seconds: 4.0 / 48000.0,
        };
        let resampled = asset.resample(48000);
        assert_eq!(resampled.sample_rate, 48000);
        assert_eq!(resampled.samples.len(), asset.samples.len());
    }

    #[test]
    fn resample_upsample_doubles_approximately() {
        let asset = AudioAsset {
            samples: vec![0.0, 1.0, 0.0, -1.0],
            sample_rate: 24000,
            channels: 1,
            duration_seconds: 4.0 / 24000.0,
        };
        let resampled = asset.resample(48000);
        assert_eq!(resampled.sample_rate, 48000);
        // Should have approximately 2x the frames
        assert!(resampled.samples.len() >= 7);
    }

    #[test]
    fn resample_downsample_halves_approximately() {
        let asset = AudioAsset {
            samples: (0..100).map(|i| (i as f32) / 100.0).collect(),
            sample_rate: 48000,
            channels: 1,
            duration_seconds: 100.0 / 48000.0,
        };
        let resampled = asset.resample(24000);
        assert_eq!(resampled.sample_rate, 24000);
        assert!(resampled.samples.len() <= 51);
    }

    #[test]
    fn resample_stereo() {
        // 4 frames of stereo (8 samples total)
        let asset = AudioAsset {
            samples: vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, -1.0, -1.0],
            sample_rate: 24000,
            channels: 2,
            duration_seconds: 4.0 / 24000.0,
        };
        let resampled = asset.resample(48000);
        assert_eq!(resampled.channels, 2);
        assert_eq!(resampled.sample_rate, 48000);
        // Output samples should be even (stereo pairs)
        assert_eq!(resampled.samples.len() % 2, 0);
    }

    #[test]
    fn load_auto_rejects_unknown_extension() {
        let result = load_auto("/fake/path/file.mp3");
        assert!(matches!(result, Err(LoadError::UnsupportedFormat(_))));
    }

    #[test]
    fn load_auto_dispatches_wav() {
        // This will fail because the file doesn't exist, but it should be an IO error
        // not an unsupported format error
        let result = load_auto("/nonexistent/file.wav");
        assert!(matches!(
            result,
            Err(LoadError::IoError(_)) | Err(LoadError::InvalidData(_))
        ));
    }

    #[test]
    fn load_auto_dispatches_ogg() {
        let result = load_auto("/nonexistent/file.ogg");
        assert!(matches!(result, Err(LoadError::IoError(_))));
    }

    #[test]
    fn load_wav_from_disk_not_found() {
        let result = load_wav("/tmp/nonexistent_aether_test_audio.wav");
        assert!(result.is_err());
    }

    #[test]
    fn load_ogg_from_disk_not_found() {
        let result = load_ogg("/tmp/nonexistent_aether_test_audio.ogg");
        assert!(result.is_err());
    }

    #[test]
    fn load_wav_roundtrip_through_file() {
        let samples: Vec<i16> = (0..960)
            .map(|i| ((i as f32 / 960.0) * 16000.0) as i16)
            .collect();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.wav");

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for &s in &samples {
            writer.write_sample(s).unwrap();
        }
        writer.finalize().unwrap();

        let asset = load_wav(&path).unwrap();
        assert_eq!(asset.sample_rate, 48000);
        assert_eq!(asset.channels, 1);
        assert_eq!(asset.samples.len(), 960);
    }
}
