use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BufferSize, Device, Host, SampleRate, Stream, StreamConfig, SupportedStreamConfigRange,
};

const DEFAULT_SAMPLE_RATE: u32 = 48_000;
const DEFAULT_BUFFER_SIZE: u32 = 1024;
const DEFAULT_OUTPUT_CHANNELS: u16 = 2;
const DEFAULT_INPUT_CHANNELS: u16 = 1;

const ENV_SAMPLE_RATE: &str = "AETHER_AUDIO_SAMPLE_RATE";
const ENV_BUFFER_SIZE: &str = "AETHER_AUDIO_BUFFER_SIZE";
const ENV_OUTPUT_CHANNELS: &str = "AETHER_AUDIO_OUTPUT_CHANNELS";
const ENV_INPUT_CHANNELS: &str = "AETHER_AUDIO_INPUT_CHANNELS";

/// Errors that can occur during device operations.
#[derive(Debug)]
pub enum DeviceError {
    NoOutputDevice,
    NoInputDevice,
    ConfigNotSupported(String),
    StreamError(String),
    HostError(String),
}

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceError::NoOutputDevice => write!(f, "no output audio device found"),
            DeviceError::NoInputDevice => write!(f, "no input audio device found"),
            DeviceError::ConfigNotSupported(msg) => {
                write!(f, "stream configuration not supported: {msg}")
            }
            DeviceError::StreamError(msg) => write!(f, "stream error: {msg}"),
            DeviceError::HostError(msg) => write!(f, "host error: {msg}"),
        }
    }
}

impl std::error::Error for DeviceError {}

/// Configuration for audio device streams.
#[derive(Debug, Clone)]
pub struct DeviceConfig {
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub output_channels: u16,
    pub input_channels: u16,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            sample_rate: DEFAULT_SAMPLE_RATE,
            buffer_size: DEFAULT_BUFFER_SIZE,
            output_channels: DEFAULT_OUTPUT_CHANNELS,
            input_channels: DEFAULT_INPUT_CHANNELS,
        }
    }
}

impl DeviceConfig {
    /// Build a DeviceConfig reading overrides from environment variables.
    pub fn from_env() -> Self {
        let mut cfg = Self::default();

        if let Ok(val) = std::env::var(ENV_SAMPLE_RATE) {
            if let Ok(rate) = val.parse::<u32>() {
                cfg.sample_rate = rate;
            }
        }
        if let Ok(val) = std::env::var(ENV_BUFFER_SIZE) {
            if let Ok(size) = val.parse::<u32>() {
                cfg.buffer_size = size;
            }
        }
        if let Ok(val) = std::env::var(ENV_OUTPUT_CHANNELS) {
            if let Ok(ch) = val.parse::<u16>() {
                cfg.output_channels = ch;
            }
        }
        if let Ok(val) = std::env::var(ENV_INPUT_CHANNELS) {
            if let Ok(ch) = val.parse::<u16>() {
                cfg.input_channels = ch;
            }
        }

        cfg
    }

    /// Build a cpal StreamConfig for output.
    pub fn output_stream_config(&self) -> StreamConfig {
        StreamConfig {
            channels: self.output_channels,
            sample_rate: SampleRate(self.sample_rate),
            buffer_size: BufferSize::Fixed(self.buffer_size),
        }
    }

    /// Build a cpal StreamConfig for input.
    pub fn input_stream_config(&self) -> StreamConfig {
        StreamConfig {
            channels: self.input_channels,
            sample_rate: SampleRate(self.sample_rate),
            buffer_size: BufferSize::Fixed(self.buffer_size),
        }
    }
}

/// Information about an available audio device.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub is_default: bool,
    pub supported_sample_rates: Vec<u32>,
}

/// Manages audio device enumeration and stream creation.
pub struct AudioDeviceManager {
    host: Host,
    config: DeviceConfig,
}

impl AudioDeviceManager {
    pub fn new(config: DeviceConfig) -> Self {
        Self {
            host: cpal::default_host(),
            config,
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(DeviceConfig::from_env())
    }

    pub fn config(&self) -> &DeviceConfig {
        &self.config
    }

    /// List available output devices.
    pub fn list_output_devices(&self) -> Result<Vec<DeviceInfo>, DeviceError> {
        let default_name = self
            .host
            .default_output_device()
            .and_then(|d| d.name().ok());

        let devices = self
            .host
            .output_devices()
            .map_err(|e| DeviceError::HostError(e.to_string()))?;

        let mut result = Vec::new();
        for device in devices {
            let name = device.name().unwrap_or_else(|_| "unknown".to_string());
            let is_default = default_name.as_deref() == Some(&name);
            let supported_sample_rates = collect_sample_rates(&device, false);
            result.push(DeviceInfo {
                name,
                is_default,
                supported_sample_rates,
            });
        }
        Ok(result)
    }

    /// List available input devices.
    pub fn list_input_devices(&self) -> Result<Vec<DeviceInfo>, DeviceError> {
        let default_name = self.host.default_input_device().and_then(|d| d.name().ok());

        let devices = self
            .host
            .input_devices()
            .map_err(|e| DeviceError::HostError(e.to_string()))?;

        let mut result = Vec::new();
        for device in devices {
            let name = device.name().unwrap_or_else(|_| "unknown".to_string());
            let is_default = default_name.as_deref() == Some(&name);
            let supported_sample_rates = collect_sample_rates(&device, true);
            result.push(DeviceInfo {
                name,
                is_default,
                supported_sample_rates,
            });
        }
        Ok(result)
    }

    /// Open an output stream that pulls samples from the provided buffer.
    pub fn open_output_stream(
        &self,
        sample_buffer: Arc<Mutex<Vec<f32>>>,
    ) -> Result<OutputHandle, DeviceError> {
        let device = self
            .host
            .default_output_device()
            .ok_or(DeviceError::NoOutputDevice)?;

        let stream_config = self.config.output_stream_config();
        let channels = self.config.output_channels as usize;

        let stream = device
            .build_output_stream(
                &stream_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut buf = sample_buffer.lock().unwrap_or_else(|e| e.into_inner());
                    for sample in data.iter_mut() {
                        if let Some(s) = buf.first().copied() {
                            buf.remove(0);
                            *sample = s;
                        } else {
                            *sample = 0.0;
                        }
                    }
                },
                move |err| {
                    eprintln!("audio output stream error: {err}");
                },
                None,
            )
            .map_err(|e| DeviceError::StreamError(e.to_string()))?;

        stream
            .play()
            .map_err(|e| DeviceError::StreamError(e.to_string()))?;

        Ok(OutputHandle {
            _stream: stream,
            channels,
            sample_rate: self.config.sample_rate,
        })
    }

    /// Open an input stream that pushes captured samples into the provided buffer.
    pub fn open_input_stream(
        &self,
        sample_buffer: Arc<Mutex<Vec<f32>>>,
    ) -> Result<InputHandle, DeviceError> {
        let device = self
            .host
            .default_input_device()
            .ok_or(DeviceError::NoInputDevice)?;

        let stream_config = self.config.input_stream_config();

        let stream = device
            .build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let mut buf = sample_buffer.lock().unwrap_or_else(|e| e.into_inner());
                    buf.extend_from_slice(data);
                },
                move |err| {
                    eprintln!("audio input stream error: {err}");
                },
                None,
            )
            .map_err(|e| DeviceError::StreamError(e.to_string()))?;

        stream
            .play()
            .map_err(|e| DeviceError::StreamError(e.to_string()))?;

        Ok(InputHandle {
            _stream: stream,
            sample_rate: self.config.sample_rate,
        })
    }
}

/// Handle to an active output audio stream. The stream stops when dropped.
pub struct OutputHandle {
    _stream: Stream,
    pub channels: usize,
    pub sample_rate: u32,
}

/// Handle to an active input audio stream. The stream stops when dropped.
pub struct InputHandle {
    _stream: Stream,
    pub sample_rate: u32,
}

fn collect_sample_rates(device: &Device, is_input: bool) -> Vec<u32> {
    let configs: Vec<SupportedStreamConfigRange> = if is_input {
        device
            .supported_input_configs()
            .map(|c| c.collect())
            .unwrap_or_default()
    } else {
        device
            .supported_output_configs()
            .map(|c| c.collect())
            .unwrap_or_default()
    };

    let mut rates: Vec<u32> = Vec::new();
    for cfg in configs {
        let min = cfg.min_sample_rate().0;
        let max = cfg.max_sample_rate().0;
        for &standard_rate in &[8000, 16000, 22050, 44100, 48000, 96000] {
            if standard_rate >= min && standard_rate <= max && !rates.contains(&standard_rate) {
                rates.push(standard_rate);
            }
        }
    }
    rates.sort();
    rates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let cfg = DeviceConfig::default();
        assert_eq!(cfg.sample_rate, 48_000);
        assert_eq!(cfg.buffer_size, 1024);
        assert_eq!(cfg.output_channels, 2);
        assert_eq!(cfg.input_channels, 1);
    }

    #[test]
    fn output_stream_config_matches_device_config() {
        let cfg = DeviceConfig {
            sample_rate: 44100,
            buffer_size: 512,
            output_channels: 2,
            input_channels: 1,
        };
        let stream_cfg = cfg.output_stream_config();
        assert_eq!(stream_cfg.channels, 2);
        assert_eq!(stream_cfg.sample_rate.0, 44100);
    }

    #[test]
    fn input_stream_config_matches_device_config() {
        let cfg = DeviceConfig {
            sample_rate: 16000,
            buffer_size: 256,
            output_channels: 2,
            input_channels: 1,
        };
        let stream_cfg = cfg.input_stream_config();
        assert_eq!(stream_cfg.channels, 1);
        assert_eq!(stream_cfg.sample_rate.0, 16000);
    }

    #[test]
    fn from_env_uses_defaults_when_no_env() {
        // Clear any env vars that might be set
        std::env::remove_var(ENV_SAMPLE_RATE);
        std::env::remove_var(ENV_BUFFER_SIZE);
        std::env::remove_var(ENV_OUTPUT_CHANNELS);
        std::env::remove_var(ENV_INPUT_CHANNELS);

        let cfg = DeviceConfig::from_env();
        assert_eq!(cfg.sample_rate, DEFAULT_SAMPLE_RATE);
        assert_eq!(cfg.buffer_size, DEFAULT_BUFFER_SIZE);
        assert_eq!(cfg.output_channels, DEFAULT_OUTPUT_CHANNELS);
        assert_eq!(cfg.input_channels, DEFAULT_INPUT_CHANNELS);
    }

    #[test]
    fn device_info_struct_accessible() {
        let info = DeviceInfo {
            name: "Test Device".to_string(),
            is_default: true,
            supported_sample_rates: vec![44100, 48000],
        };
        assert_eq!(info.name, "Test Device");
        assert!(info.is_default);
        assert_eq!(info.supported_sample_rates.len(), 2);
    }

    #[test]
    #[ignore]
    fn can_enumerate_output_devices() {
        let manager = AudioDeviceManager::with_default_config();
        let devices = manager.list_output_devices().unwrap();
        assert!(!devices.is_empty(), "expected at least one output device");
    }

    #[test]
    #[ignore]
    fn can_open_and_close_output_stream() {
        let manager = AudioDeviceManager::with_default_config();
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let handle = manager.open_output_stream(buffer);
        assert!(handle.is_ok(), "should open output stream without error");
        // Stream is closed when handle is dropped
    }

    #[test]
    #[ignore]
    fn can_enumerate_input_devices() {
        let manager = AudioDeviceManager::with_default_config();
        let devices = manager.list_input_devices().unwrap();
        assert!(!devices.is_empty(), "expected at least one input device");
    }

    #[test]
    #[ignore]
    fn can_open_and_close_input_stream() {
        let manager = AudioDeviceManager::with_default_config();
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let handle = manager.open_input_stream(buffer);
        assert!(handle.is_ok(), "should open input stream without error");
    }
}
