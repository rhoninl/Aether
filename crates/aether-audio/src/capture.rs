use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::codec::AudioCodec;
use crate::device::{AudioDeviceManager, DeviceConfig, DeviceError, InputHandle};

const DEFAULT_RING_BUFFER_CAPACITY: usize = 48_000; // 1 second at 48kHz

/// Configuration for microphone capture.
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: u32,
    pub ring_buffer_capacity: usize,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            channels: 1,
            buffer_size: 1024,
            ring_buffer_capacity: DEFAULT_RING_BUFFER_CAPACITY,
        }
    }
}

impl CaptureConfig {
    pub fn to_device_config(&self) -> DeviceConfig {
        DeviceConfig {
            sample_rate: self.sample_rate,
            buffer_size: self.buffer_size,
            output_channels: 2,
            input_channels: self.channels,
        }
    }
}

/// Thread-safe ring buffer for captured audio samples.
pub struct CaptureRingBuffer {
    buffer: VecDeque<f32>,
    capacity: usize,
}

impl CaptureRingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push samples into the ring buffer, discarding oldest if at capacity.
    pub fn push_samples(&mut self, samples: &[f32]) {
        for &sample in samples {
            if self.buffer.len() >= self.capacity {
                self.buffer.pop_front();
            }
            self.buffer.push_back(sample);
        }
    }

    /// Drain up to `count` samples from the front of the buffer.
    pub fn drain_samples(&mut self, count: usize) -> Vec<f32> {
        let take = count.min(self.buffer.len());
        self.buffer.drain(..take).collect()
    }

    /// Number of samples currently available.
    pub fn available(&self) -> usize {
        self.buffer.len()
    }

    /// Clear all samples.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Manages microphone capture with a ring buffer and optional codec encoding.
pub struct CaptureStream {
    config: CaptureConfig,
    raw_buffer: Arc<Mutex<Vec<f32>>>,
    ring_buffer: Arc<Mutex<CaptureRingBuffer>>,
    _input_handle: Option<InputHandle>,
}

impl CaptureStream {
    /// Create a capture stream. Call `start()` to begin capturing.
    pub fn new(config: CaptureConfig) -> Self {
        let ring_capacity = config.ring_buffer_capacity;
        Self {
            config,
            raw_buffer: Arc::new(Mutex::new(Vec::new())),
            ring_buffer: Arc::new(Mutex::new(CaptureRingBuffer::new(ring_capacity))),
            _input_handle: None,
        }
    }

    /// Start capturing from the default input device.
    pub fn start(&mut self) -> Result<(), DeviceError> {
        let device_config = self.config.to_device_config();
        let manager = AudioDeviceManager::new(device_config);
        let handle = manager.open_input_stream(self.raw_buffer.clone())?;
        self._input_handle = Some(handle);
        Ok(())
    }

    /// Transfer samples from the raw cpal buffer into the ring buffer.
    /// This should be called periodically from the application's update loop.
    pub fn poll(&self) {
        let mut raw = self.raw_buffer.lock().unwrap_or_else(|e| e.into_inner());
        if raw.is_empty() {
            return;
        }
        let samples: Vec<f32> = raw.drain(..).collect();
        drop(raw);

        let mut ring = self.ring_buffer.lock().unwrap_or_else(|e| e.into_inner());
        ring.push_samples(&samples);
    }

    /// Read available samples from the ring buffer (drains them).
    pub fn read_samples(&self, max_count: usize) -> Vec<f32> {
        let mut ring = self.ring_buffer.lock().unwrap_or_else(|e| e.into_inner());
        ring.drain_samples(max_count)
    }

    /// Read exactly one codec frame worth of samples, encoding them.
    pub fn read_encoded_frame(
        &self,
        codec: &mut dyn AudioCodec,
    ) -> Option<Result<Vec<u8>, crate::codec::CodecEncodeError>> {
        let frame_size = codec.frame_size();
        let mut ring = self.ring_buffer.lock().unwrap_or_else(|e| e.into_inner());
        if ring.available() < frame_size {
            return None;
        }
        let samples = ring.drain_samples(frame_size);
        drop(ring);
        Some(codec.encode(&samples))
    }

    /// Number of samples available in the ring buffer.
    pub fn available_samples(&self) -> usize {
        let ring = self.ring_buffer.lock().unwrap_or_else(|e| e.into_inner());
        ring.available()
    }

    /// Stop capturing. The stream handle is dropped.
    pub fn stop(&mut self) {
        self._input_handle = None;
    }

    pub fn is_active(&self) -> bool {
        self._input_handle.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_config_defaults() {
        let cfg = CaptureConfig::default();
        assert_eq!(cfg.sample_rate, 48_000);
        assert_eq!(cfg.channels, 1);
        assert_eq!(cfg.buffer_size, 1024);
        assert_eq!(cfg.ring_buffer_capacity, DEFAULT_RING_BUFFER_CAPACITY);
    }

    #[test]
    fn capture_config_to_device_config() {
        let cfg = CaptureConfig {
            sample_rate: 16000,
            channels: 1,
            buffer_size: 512,
            ring_buffer_capacity: 16000,
        };
        let dc = cfg.to_device_config();
        assert_eq!(dc.sample_rate, 16000);
        assert_eq!(dc.input_channels, 1);
        assert_eq!(dc.buffer_size, 512);
    }

    #[test]
    fn ring_buffer_push_and_drain() {
        let mut buf = CaptureRingBuffer::new(10);
        buf.push_samples(&[1.0, 2.0, 3.0]);
        assert_eq!(buf.available(), 3);

        let drained = buf.drain_samples(2);
        assert_eq!(drained, vec![1.0, 2.0]);
        assert_eq!(buf.available(), 1);
    }

    #[test]
    fn ring_buffer_respects_capacity() {
        let mut buf = CaptureRingBuffer::new(4);
        buf.push_samples(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        // capacity is 4, so oldest samples should be discarded
        assert_eq!(buf.available(), 4);

        let drained = buf.drain_samples(4);
        assert_eq!(drained, vec![3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn ring_buffer_drain_more_than_available() {
        let mut buf = CaptureRingBuffer::new(100);
        buf.push_samples(&[1.0, 2.0]);
        let drained = buf.drain_samples(10);
        assert_eq!(drained, vec![1.0, 2.0]);
        assert_eq!(buf.available(), 0);
    }

    #[test]
    fn ring_buffer_clear() {
        let mut buf = CaptureRingBuffer::new(100);
        buf.push_samples(&[1.0, 2.0, 3.0]);
        buf.clear();
        assert_eq!(buf.available(), 0);
    }

    #[test]
    fn ring_buffer_capacity() {
        let buf = CaptureRingBuffer::new(42);
        assert_eq!(buf.capacity(), 42);
    }

    #[test]
    fn capture_stream_not_active_before_start() {
        let stream = CaptureStream::new(CaptureConfig::default());
        assert!(!stream.is_active());
        assert_eq!(stream.available_samples(), 0);
    }

    #[test]
    fn capture_stream_read_samples_returns_empty_when_no_data() {
        let stream = CaptureStream::new(CaptureConfig::default());
        let samples = stream.read_samples(1024);
        assert!(samples.is_empty());
    }

    #[test]
    fn capture_stream_poll_transfers_raw_to_ring() {
        let stream = CaptureStream::new(CaptureConfig::default());

        // Simulate cpal pushing data into the raw buffer
        {
            let mut raw = stream.raw_buffer.lock().unwrap();
            raw.extend_from_slice(&[0.1, 0.2, 0.3, 0.4]);
        }

        stream.poll();

        assert_eq!(stream.available_samples(), 4);
        let samples = stream.read_samples(4);
        assert_eq!(samples, vec![0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn capture_stream_read_encoded_frame() {
        let stream = CaptureStream::new(CaptureConfig::default());
        let mut codec = crate::codec::StubCodec::from_default();
        let frame_size = codec.frame_size();

        // Not enough samples for a frame
        assert!(stream.read_encoded_frame(&mut codec).is_none());

        // Push exactly one frame worth of samples
        {
            let mut raw = stream.raw_buffer.lock().unwrap();
            raw.extend(vec![0.5f32; frame_size]);
        }
        stream.poll();

        let result = stream.read_encoded_frame(&mut codec);
        assert!(result.is_some());
        let encoded = result.unwrap().unwrap();
        assert!(!encoded.is_empty());

        // Decode and verify
        let decoded = codec.decode(&encoded).unwrap();
        assert_eq!(decoded.len(), frame_size);
        assert!((decoded[0] - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    #[ignore]
    fn capture_stream_start_stop() {
        let mut stream = CaptureStream::new(CaptureConfig::default());
        stream.start().expect("should start capture");
        assert!(stream.is_active());
        stream.stop();
        assert!(!stream.is_active());
    }
}
