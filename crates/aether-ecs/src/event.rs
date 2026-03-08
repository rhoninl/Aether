use std::any::TypeId;

/// Trait for type-erased event buffer swapping, used by the tick runner.
pub trait EventQueue: Send + Sync + 'static {
    /// Swap the double buffers: the current write buffer becomes the read buffer,
    /// and the old read buffer is cleared to become the new write buffer.
    fn swap_buffers(&mut self);

    /// Returns the TypeId that this event queue was registered under.
    fn event_type_id(&self) -> TypeId;
}

/// A double-buffered event channel for inter-system communication.
///
/// Events sent during the current tick are written to the write buffer.
/// Events from the previous tick can be read from the read buffer.
/// The tick runner swaps buffers between ticks.
pub struct Events<T: Send + Sync + 'static> {
    buffers: [Vec<T>; 2],
    /// Index of the current write buffer (0 or 1).
    write_idx: usize,
}

impl<T: Send + Sync + 'static> Events<T> {
    pub fn new() -> Self {
        Self {
            buffers: [Vec::new(), Vec::new()],
            write_idx: 0,
        }
    }

    /// Send an event into the current write buffer.
    pub fn send(&mut self, event: T) {
        self.buffers[self.write_idx].push(event);
    }

    /// Read events from the previous tick (the read buffer).
    /// Returns an empty slice if no events were sent last tick.
    pub fn read(&self) -> &[T] {
        let read_idx = 1 - self.write_idx;
        &self.buffers[read_idx]
    }

    /// Number of events in the read buffer (previous tick).
    pub fn read_count(&self) -> usize {
        self.read().len()
    }

    /// Number of events in the write buffer (current tick, not yet readable).
    pub fn write_count(&self) -> usize {
        self.buffers[self.write_idx].len()
    }
}

impl<T: Send + Sync + 'static> Default for Events<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Send + Sync + 'static> EventQueue for Events<T> {
    fn swap_buffers(&mut self) {
        // The current write buffer becomes the read buffer.
        // The old read buffer (now becoming write buffer) is cleared.
        self.write_idx = 1 - self.write_idx;
        self.buffers[self.write_idx].clear();
    }

    fn event_type_id(&self) -> TypeId {
        TypeId::of::<Events<T>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Clone)]
    struct CollisionEvent {
        entity_a: u32,
        entity_b: u32,
    }

    #[derive(Debug, PartialEq, Clone)]
    struct DamageEvent {
        target: u32,
        amount: f32,
    }

    #[test]
    fn send_and_read_within_same_tick_yields_empty_read() {
        let mut events = Events::<CollisionEvent>::new();
        events.send(CollisionEvent {
            entity_a: 1,
            entity_b: 2,
        });
        // Read buffer is the *other* buffer, which is empty
        assert_eq!(events.read().len(), 0);
    }

    #[test]
    fn after_swap_previously_sent_events_are_readable() {
        let mut events = Events::<CollisionEvent>::new();
        events.send(CollisionEvent {
            entity_a: 1,
            entity_b: 2,
        });
        events.swap_buffers();
        let read = events.read();
        assert_eq!(read.len(), 1);
        assert_eq!(
            read[0],
            CollisionEvent {
                entity_a: 1,
                entity_b: 2
            }
        );
    }

    #[test]
    fn multiple_events_accumulate_in_buffer() {
        let mut events = Events::<CollisionEvent>::new();
        events.send(CollisionEvent {
            entity_a: 1,
            entity_b: 2,
        });
        events.send(CollisionEvent {
            entity_a: 3,
            entity_b: 4,
        });
        events.send(CollisionEvent {
            entity_a: 5,
            entity_b: 6,
        });
        events.swap_buffers();
        assert_eq!(events.read().len(), 3);
    }

    #[test]
    fn swap_clears_write_buffer_for_next_tick() {
        let mut events = Events::<CollisionEvent>::new();
        events.send(CollisionEvent {
            entity_a: 1,
            entity_b: 2,
        });
        events.swap_buffers();
        // After swap, the new write buffer should be empty
        assert_eq!(events.write_count(), 0);
        // New events go to the new write buffer
        events.send(CollisionEvent {
            entity_a: 10,
            entity_b: 20,
        });
        assert_eq!(events.write_count(), 1);
    }

    #[test]
    fn events_cleared_after_two_swaps() {
        let mut events = Events::<CollisionEvent>::new();
        events.send(CollisionEvent {
            entity_a: 1,
            entity_b: 2,
        });
        events.swap_buffers();
        assert_eq!(events.read().len(), 1);
        // Second swap: old read buffer (which had the event) becomes write buffer and is cleared
        events.swap_buffers();
        assert_eq!(events.read().len(), 0);
    }

    #[test]
    fn empty_event_reads_return_empty_slice() {
        let events = Events::<CollisionEvent>::new();
        assert!(events.read().is_empty());
    }

    #[test]
    fn multiple_event_types_are_independent() {
        let mut collisions = Events::<CollisionEvent>::new();
        let mut damages = Events::<DamageEvent>::new();

        collisions.send(CollisionEvent {
            entity_a: 1,
            entity_b: 2,
        });
        damages.send(DamageEvent {
            target: 1,
            amount: 50.0,
        });

        collisions.swap_buffers();
        damages.swap_buffers();

        assert_eq!(collisions.read().len(), 1);
        assert_eq!(damages.read().len(), 1);
        assert_eq!(
            damages.read()[0],
            DamageEvent {
                target: 1,
                amount: 50.0
            }
        );
    }

    #[test]
    fn write_count_tracks_current_buffer() {
        let mut events = Events::<CollisionEvent>::new();
        assert_eq!(events.write_count(), 0);
        events.send(CollisionEvent {
            entity_a: 1,
            entity_b: 2,
        });
        assert_eq!(events.write_count(), 1);
        events.send(CollisionEvent {
            entity_a: 3,
            entity_b: 4,
        });
        assert_eq!(events.write_count(), 2);
    }

    #[test]
    fn read_count_tracks_previous_buffer() {
        let mut events = Events::<CollisionEvent>::new();
        assert_eq!(events.read_count(), 0);
        events.send(CollisionEvent {
            entity_a: 1,
            entity_b: 2,
        });
        assert_eq!(events.read_count(), 0); // not yet swapped
        events.swap_buffers();
        assert_eq!(events.read_count(), 1);
    }

    #[test]
    fn event_queue_trait_swap_works() {
        let mut events = Events::<CollisionEvent>::new();
        events.send(CollisionEvent {
            entity_a: 1,
            entity_b: 2,
        });

        // Use the trait method
        let queue: &mut dyn EventQueue = &mut events;
        queue.swap_buffers();

        assert_eq!(events.read().len(), 1);
    }

    #[test]
    fn continuous_send_swap_cycle() {
        let mut events = Events::<CollisionEvent>::new();

        // Tick 1: send event A
        events.send(CollisionEvent {
            entity_a: 1,
            entity_b: 2,
        });
        events.swap_buffers();
        assert_eq!(events.read().len(), 1);

        // Tick 2: send event B, read event A
        events.send(CollisionEvent {
            entity_a: 3,
            entity_b: 4,
        });
        assert_eq!(events.read().len(), 1); // still event A
        assert_eq!(events.read()[0].entity_a, 1);
        events.swap_buffers();

        // Tick 3: read event B
        assert_eq!(events.read().len(), 1);
        assert_eq!(events.read()[0].entity_a, 3);
        events.swap_buffers();

        // Tick 4: nothing to read
        assert_eq!(events.read().len(), 0);
    }
}
