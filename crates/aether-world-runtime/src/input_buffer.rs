//! Client input buffering, ordering, and validation.

use std::collections::{HashMap, VecDeque};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Default maximum number of buffered inputs per player.
const DEFAULT_MAX_BUFFER_SIZE: usize = 64;

/// Unique player identifier.
pub type PlayerId = Uuid;

/// An individual action within a player input frame.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InputAction {
    Jump,
    Interact,
    Attack,
    UseItem { item_id: u64 },
    Custom(String),
}

/// A single frame of player input tied to a specific tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInput {
    pub player_id: PlayerId,
    pub tick: u64,
    pub movement: [f32; 3],
    pub look_rotation: [f32; 4],
    pub actions: Vec<InputAction>,
}

/// Errors returned from input buffer operations.
#[derive(Debug, Clone, PartialEq)]
pub enum InputBufferError {
    /// The input tick was already received or is in the past.
    DuplicateOrStale { player_id: PlayerId, tick: u64 },
    /// The buffer is full and the oldest entry was evicted.
    BufferOverflow { player_id: PlayerId },
}

/// Per-player input buffer with ordering and validation.
#[derive(Debug)]
pub struct InputBuffer {
    buffers: HashMap<PlayerId, VecDeque<PlayerInput>>,
    max_buffer_size: usize,
}

impl InputBuffer {
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
            max_buffer_size: DEFAULT_MAX_BUFFER_SIZE,
        }
    }

    pub fn with_max_buffer_size(max_buffer_size: usize) -> Self {
        Self {
            buffers: HashMap::new(),
            max_buffer_size: max_buffer_size.max(1),
        }
    }

    /// Submit an input frame. Returns an error if the tick is a duplicate or stale.
    pub fn submit(&mut self, input: PlayerInput) -> Result<(), InputBufferError> {
        let buffer = self.buffers.entry(input.player_id).or_default();

        // Check for duplicate or stale tick
        if let Some(last) = buffer.back() {
            if input.tick <= last.tick {
                return Err(InputBufferError::DuplicateOrStale {
                    player_id: input.player_id,
                    tick: input.tick,
                });
            }
        }

        // Evict oldest if at capacity
        let mut overflow = false;
        if buffer.len() >= self.max_buffer_size {
            buffer.pop_front();
            overflow = true;
        }

        buffer.push_back(input.clone());

        if overflow {
            return Err(InputBufferError::BufferOverflow {
                player_id: input.player_id,
            });
        }

        Ok(())
    }

    /// Collect all player inputs for a specific tick.
    /// Removes the consumed inputs from the buffer.
    pub fn drain_for_tick(&mut self, tick: u64) -> Vec<PlayerInput> {
        let mut result = Vec::new();
        for buffer in self.buffers.values_mut() {
            // Find and remove the input matching this tick
            if let Some(pos) = buffer.iter().position(|i| i.tick == tick) {
                // Also drain any inputs older than this tick
                let drained: Vec<PlayerInput> = buffer.drain(..=pos).collect();
                if let Some(input) = drained.into_iter().last() {
                    result.push(input);
                }
            }
        }
        result
    }

    /// Peek at the latest input for a given player without removing it.
    pub fn latest_input(&self, player_id: &PlayerId) -> Option<&PlayerInput> {
        self.buffers.get(player_id).and_then(|b| b.back())
    }

    /// Get the number of buffered inputs for a player.
    pub fn buffered_count(&self, player_id: &PlayerId) -> usize {
        self.buffers.get(player_id).map_or(0, |b| b.len())
    }

    /// Remove all inputs for a player (e.g., on disconnect).
    pub fn remove_player(&mut self, player_id: &PlayerId) {
        self.buffers.remove(player_id);
    }

    /// Get all tracked player IDs.
    pub fn player_ids(&self) -> Vec<PlayerId> {
        self.buffers.keys().copied().collect()
    }
}

impl Default for InputBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(player_id: PlayerId, tick: u64) -> PlayerInput {
        PlayerInput {
            player_id,
            tick,
            movement: [0.0, 0.0, 1.0],
            look_rotation: [0.0, 0.0, 0.0, 1.0],
            actions: vec![],
        }
    }

    #[test]
    fn test_submit_and_drain_single_player() {
        let mut buf = InputBuffer::new();
        let pid = Uuid::new_v4();

        buf.submit(make_input(pid, 1)).unwrap();
        buf.submit(make_input(pid, 2)).unwrap();
        buf.submit(make_input(pid, 3)).unwrap();

        let inputs = buf.drain_for_tick(2);
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].tick, 2);
        assert_eq!(inputs[0].player_id, pid);

        // Tick 1 should have been drained too, only tick 3 remains
        assert_eq!(buf.buffered_count(&pid), 1);
    }

    #[test]
    fn test_submit_duplicate_tick_rejected() {
        let mut buf = InputBuffer::new();
        let pid = Uuid::new_v4();

        buf.submit(make_input(pid, 5)).unwrap();
        let result = buf.submit(make_input(pid, 5));
        assert_eq!(
            result,
            Err(InputBufferError::DuplicateOrStale {
                player_id: pid,
                tick: 5
            })
        );
    }

    #[test]
    fn test_submit_stale_tick_rejected() {
        let mut buf = InputBuffer::new();
        let pid = Uuid::new_v4();

        buf.submit(make_input(pid, 10)).unwrap();
        let result = buf.submit(make_input(pid, 5));
        assert_eq!(
            result,
            Err(InputBufferError::DuplicateOrStale {
                player_id: pid,
                tick: 5
            })
        );
    }

    #[test]
    fn test_multi_player_drain() {
        let mut buf = InputBuffer::new();
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();

        buf.submit(make_input(p1, 1)).unwrap();
        buf.submit(make_input(p2, 1)).unwrap();
        buf.submit(make_input(p1, 2)).unwrap();
        buf.submit(make_input(p2, 2)).unwrap();

        let inputs = buf.drain_for_tick(1);
        assert_eq!(inputs.len(), 2);
        assert!(inputs.iter().all(|i| i.tick == 1));
    }

    #[test]
    fn test_drain_missing_tick_returns_empty() {
        let mut buf = InputBuffer::new();
        let pid = Uuid::new_v4();

        buf.submit(make_input(pid, 5)).unwrap();
        let inputs = buf.drain_for_tick(3);
        assert!(inputs.is_empty());
    }

    #[test]
    fn test_buffer_overflow_evicts_oldest() {
        let mut buf = InputBuffer::with_max_buffer_size(3);
        let pid = Uuid::new_v4();

        buf.submit(make_input(pid, 1)).unwrap();
        buf.submit(make_input(pid, 2)).unwrap();
        buf.submit(make_input(pid, 3)).unwrap();

        // This should evict tick 1 and return overflow error
        let result = buf.submit(make_input(pid, 4));
        assert_eq!(
            result,
            Err(InputBufferError::BufferOverflow { player_id: pid })
        );

        // Buffer should have ticks 2, 3, 4
        assert_eq!(buf.buffered_count(&pid), 3);
        assert_eq!(buf.latest_input(&pid).unwrap().tick, 4);
    }

    #[test]
    fn test_latest_input() {
        let mut buf = InputBuffer::new();
        let pid = Uuid::new_v4();

        assert!(buf.latest_input(&pid).is_none());

        buf.submit(make_input(pid, 1)).unwrap();
        assert_eq!(buf.latest_input(&pid).unwrap().tick, 1);

        buf.submit(make_input(pid, 5)).unwrap();
        assert_eq!(buf.latest_input(&pid).unwrap().tick, 5);
    }

    #[test]
    fn test_remove_player() {
        let mut buf = InputBuffer::new();
        let pid = Uuid::new_v4();

        buf.submit(make_input(pid, 1)).unwrap();
        buf.submit(make_input(pid, 2)).unwrap();
        assert_eq!(buf.buffered_count(&pid), 2);

        buf.remove_player(&pid);
        assert_eq!(buf.buffered_count(&pid), 0);
        assert!(buf.latest_input(&pid).is_none());
    }

    #[test]
    fn test_player_ids() {
        let mut buf = InputBuffer::new();
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();

        buf.submit(make_input(p1, 1)).unwrap();
        buf.submit(make_input(p2, 1)).unwrap();

        let ids = buf.player_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&p1));
        assert!(ids.contains(&p2));
    }

    #[test]
    fn test_input_with_actions() {
        let mut buf = InputBuffer::new();
        let pid = Uuid::new_v4();

        let mut input = make_input(pid, 1);
        input.actions = vec![InputAction::Jump, InputAction::UseItem { item_id: 42 }];
        buf.submit(input).unwrap();

        let drained = buf.drain_for_tick(1);
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].actions.len(), 2);
        assert_eq!(drained[0].actions[0], InputAction::Jump);
        assert_eq!(drained[0].actions[1], InputAction::UseItem { item_id: 42 });
    }

    #[test]
    fn test_multiple_drains_are_independent() {
        let mut buf = InputBuffer::new();
        let pid = Uuid::new_v4();

        buf.submit(make_input(pid, 1)).unwrap();
        buf.submit(make_input(pid, 2)).unwrap();
        buf.submit(make_input(pid, 3)).unwrap();

        let d1 = buf.drain_for_tick(1);
        assert_eq!(d1.len(), 1);

        let d2 = buf.drain_for_tick(2);
        assert_eq!(d2.len(), 1);

        // After draining 1 and 2, only 3 should remain
        assert_eq!(buf.buffered_count(&pid), 1);
    }
}
