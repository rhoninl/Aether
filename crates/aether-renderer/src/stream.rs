use crate::config::LODLevel;
use crate::config::{FrameBudget, StreamPriority, StreamRequest};

#[derive(Debug)]
pub enum StreamError {
    TooManyRequests,
    NoBandwidth,
    InvalidLevel,
}

#[derive(Debug, Clone, Copy)]
pub struct StreamingProgress {
    pub world_id: u64,
    pub script_id: u64,
    pub bytes_loaded: u32,
    pub current_level: u8,
}

#[derive(Debug)]
pub struct ProgressiveMeshStreaming {
    pub max_bytes_per_tick: u32,
    pub min_lod: u8,
    pub max_lod: u8,
    pub current_budget: FrameBudget,
}

impl ProgressiveMeshStreaming {
    pub fn new(max_bytes_per_tick: u32) -> Self {
        Self {
            max_bytes_per_tick,
            min_lod: 0,
            max_lod: 3,
            current_budget: FrameBudget {
                target_ms: 16.6,
                gpu_headroom: 0.35,
                cpu_headroom: 0.3,
            },
        }
    }

    pub fn choose_next_level(
        &self,
        request: &StreamRequest,
        current_level: u8,
        bandwidth_available: u32,
    ) -> Result<u8, StreamError> {
        if request.requested_level < self.min_lod || request.requested_level > self.max_lod {
            return Err(StreamError::InvalidLevel);
        }

        let mut target = current_level;
        let can_increase = bandwidth_available >= request.bytes;
        let load_priority_penalty = match request.priority {
            StreamPriority::High => 1.1,
            StreamPriority::Medium => 1.0,
            StreamPriority::Low => 0.8,
        };
        let effective_budget = (self.max_bytes_per_tick as f32 * load_priority_penalty) as u32;

        if bandwidth_available == 0 || effective_budget == 0 {
            return Err(StreamError::NoBandwidth);
        }

        if can_increase
            && bandwidth_available >= request.bytes
            && request.requested_level > current_level
        {
            if target < self.max_lod {
                target += 1;
            }
        } else if !can_increase && current_level > self.min_lod {
            target = current_level.saturating_sub(1);
        } else if self.current_budget.cpu_headroom < 0.1 {
            target = current_level;
        }

        Ok(target)
    }

    pub fn lod_for_distance(distance_m: f32, base_level: LODLevel) -> LODLevel {
        let _ = base_level;
        if distance_m < 2.0 {
            LODLevel::L0Near
        } else if distance_m < 8.0 {
            LODLevel::L1Mid
        } else if distance_m < 20.0 {
            LODLevel::L2Far
        } else {
            LODLevel::L3VeryFar
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StreamRequest;

    #[test]
    fn streaming_requests_adapt_to_bandwidth() {
        let streamer = ProgressiveMeshStreaming::new(4096);
        let req = StreamRequest {
            world_id: 1,
            script_id: 9,
            requested_level: 3,
            bytes: 1024,
            priority: StreamPriority::High,
        };
        let up = streamer.choose_next_level(&req, 1, 5000).unwrap();
        assert!(up >= 1);
        let down = streamer.choose_next_level(&req, 2, 64).unwrap();
        assert!(down < 3);
    }
}
