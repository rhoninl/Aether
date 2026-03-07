use crate::types::NetEntity;

#[derive(Debug, Clone)]
pub struct InputSample {
    pub client_tick: u64,
    pub seq: u64,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct EntitySnapshot {
    pub entity_id: NetEntity,
    pub transform: [f32; 7],
    pub tick: u64,
}

#[derive(Debug, Clone)]
pub struct ClientPrediction {
    pub pending_inputs: Vec<InputSample>,
    pub max_reconcile_ms: u32,
    pub last_authoritative_tick: u64,
}

impl ClientPrediction {
    pub fn new() -> Self {
        Self {
            pending_inputs: Vec::new(),
            max_reconcile_ms: 150,
            last_authoritative_tick: 0,
        }
    }

    pub fn queue_input(&mut self, input: InputSample) {
        self.pending_inputs.push(input);
        self.pending_inputs.sort_by_key(|s| s.client_tick);
    }

    pub fn reconcile(&mut self, snapshot: &EntitySnapshot) -> Reconciliation {
        if snapshot.tick < self.last_authoritative_tick {
            return Reconciliation::Rejected;
        }
        self.last_authoritative_tick = snapshot.tick;
        self.pending_inputs.retain(|s| s.client_tick > snapshot.tick);
        Reconciliation::Accepted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconcile_discards_stale_inputs() {
        let mut pred = ClientPrediction::new();
        pred.queue_input(InputSample {
            client_tick: 1,
            seq: 1,
            payload: vec![1],
        });
        pred.queue_input(InputSample {
            client_tick: 2,
            seq: 2,
            payload: vec![2],
        });

        let snapshot = EntitySnapshot {
            entity_id: NetEntity(1),
            transform: [0.0; 7],
            tick: 1,
        };
        let r = pred.reconcile(&snapshot);
        assert!(matches!(r, Reconciliation::Accepted));
        assert_eq!(pred.pending_inputs.len(), 1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reconciliation {
    Accepted,
    Rejected,
    NotNeeded,
}

#[derive(Debug, Clone)]
pub struct InterpolationConfig {
    pub buffer_time_ms: f32,
    pub max_speed: f32,
}

impl Default for InterpolationConfig {
    fn default() -> Self {
        Self {
            buffer_time_ms: 100.0,
            max_speed: 6.0,
        }
    }
}
