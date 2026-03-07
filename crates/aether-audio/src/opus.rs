#[derive(Debug, Clone, Copy)]
pub enum BitRateKbps {
    Kbps16,
    Kbps24,
    Kbps32,
    Kbps64,
}

impl BitRateKbps {
    pub fn as_value(&self) -> u32 {
        match self {
            BitRateKbps::Kbps16 => 16000,
            BitRateKbps::Kbps24 => 24000,
            BitRateKbps::Kbps32 => 32000,
            BitRateKbps::Kbps64 => 64000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpusConfig {
    pub sample_rate_hz: u32,
    pub frame_ms: u8,
    pub bitrate_kbps: BitRateKbps,
    pub inband_fec: bool,
    pub use_hardware_accel: bool,
}

impl OpusConfig {
    pub fn opus_voice_default() -> Self {
        Self {
            sample_rate_hz: 48_000,
            frame_ms: 20,
            bitrate_kbps: BitRateKbps::Kbps32,
            inband_fec: true,
            use_hardware_accel: true,
        }
    }

    pub fn packets_per_second(&self) -> f32 {
        1000.0 / self.frame_ms as f32
    }
}

#[derive(Debug)]
pub struct OpusPacket {
    pub sequence: u64,
    pub payload: Vec<u8>,
    pub codec_ms: u8,
}

#[derive(Debug)]
pub enum CodecError {
    InvalidSampleRate(u32),
    InvalidFrameMs(u8),
    EmptyPayload,
}

impl OpusPacket {
    pub fn packet_size_limit(cfg: &OpusConfig) -> usize {
        match cfg.bitrate_kbps {
            BitRateKbps::Kbps16 => 160,
            BitRateKbps::Kbps24 => 250,
            BitRateKbps::Kbps32 => 350,
            BitRateKbps::Kbps64 => 420,
        }
    }
}
