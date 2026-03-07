#[derive(Debug, Clone)]
pub struct JitterBufferConfig {
    pub base_ms: u32,
    pub min_ms: u32,
    pub max_ms: u32,
    pub adaptive: bool,
}

impl Default for JitterBufferConfig {
    fn default() -> Self {
        Self {
            base_ms: 40,
            min_ms: 20,
            max_ms: 80,
            adaptive: true,
        }
    }
}

#[derive(Debug)]
pub struct VoicePayload {
    pub sender_id: u64,
    pub seq: u64,
    pub frame_ms: u8,
    pub fec_used: bool,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct VoiceTransport {
    pub channel_id: u64,
    pub zone_id: u64,
    pub use_datagram: bool,
}

impl Default for VoiceTransport {
    fn default() -> Self {
        Self {
            channel_id: 0,
            zone_id: 0,
            use_datagram: true,
        }
    }
}
