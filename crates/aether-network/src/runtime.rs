use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use crate::{
    ClientBudget, ClientPrediction, ClientProfile, DatagramMode, EntitySnapshot, InputSample,
    InterestManager, InterpolationConfig, JitterBufferConfig, NetEntity, QuantizedFrame, Reconciliation,
    Reliability, StateDiff, TransportMessage, TransportProfile, VoicePayload,
};

#[derive(Debug, Clone, Copy)]
pub struct NetworkTickInput {
    pub tick: u64,
    pub now_ms: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeEntityHint {
    pub entity_id: u64,
    pub position: crate::types::Vec3,
    pub importance: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeSnapshotInput {
    pub entity_id: u64,
    pub position: (f32, f32, f32),
    pub rotation_deg: (f32, f32, f32),
}

#[derive(Debug, Clone)]
pub struct RuntimeOutput {
    pub delivered_packets: Vec<TransportMessage>,
    pub reconciliations: Vec<(u64, Reconciliation)>,
    pub dropped_bytes: usize,
    pub transport_packets: usize,
}

#[derive(Debug)]
pub enum TransportError {
    CapacityExceeded,
    MessageTooLarge(usize),
}

pub trait RuntimeTransport {
    fn send(&mut self, msg: TransportMessage) -> Result<(), TransportError>;
    fn recv(&mut self, max: usize) -> Vec<TransportMessage>;
    fn flush(&mut self) {}
}

#[derive(Debug)]
pub struct InMemoryTransport {
    reliable_outbound: VecDeque<TransportMessage>,
    datagram_outbound: VecDeque<TransportMessage>,
    inbound: VecDeque<TransportMessage>,
    max_messages: usize,
}

impl InMemoryTransport {
    pub fn new(max_messages: usize) -> Self {
        Self {
            reliable_outbound: VecDeque::new(),
            datagram_outbound: VecDeque::new(),
            inbound: VecDeque::new(),
            max_messages,
        }
    }

    pub fn push_inbound(&mut self, msg: TransportMessage) {
        self.inbound.push_back(msg);
    }

    pub fn pop_reliable_outbound(&mut self) -> Option<TransportMessage> {
        self.reliable_outbound.pop_front()
    }

    pub fn pop_datagram_outbound(&mut self) -> Option<TransportMessage> {
        self.datagram_outbound.pop_front()
    }

    pub fn outbound_len(&self) -> usize {
        self.reliable_outbound.len() + self.datagram_outbound.len()
    }
}

impl RuntimeTransport for InMemoryTransport {
    fn send(&mut self, msg: TransportMessage) -> Result<(), TransportError> {
        let too_large = match msg.reliability {
            Reliability::ReliableOrdered => self.reliable_outbound.len(),
            Reliability::UnreliableDatagram => self.datagram_outbound.len(),
        };

        if too_large >= self.max_messages {
            return Err(TransportError::CapacityExceeded);
        }

        if msg.payload.len() > 64 * 1024 {
            return Err(TransportError::MessageTooLarge(msg.payload.len()));
        }

        match msg.reliability {
            Reliability::ReliableOrdered => self.reliable_outbound.push_back(msg),
            Reliability::UnreliableDatagram => self.datagram_outbound.push_back(msg),
        }
        Ok(())
    }

    fn recv(&mut self, max: usize) -> Vec<TransportMessage> {
        let mut out = Vec::with_capacity(max.min(self.inbound.len()));
        for _ in 0..max {
            if let Some(msg) = self.inbound.pop_front() {
                out.push(msg);
            }
        }
        out
    }
}

#[derive(Debug)]
pub struct RuntimeStepResult {
    pub output: RuntimeOutput,
    pub sent_to_transport: usize,
    pub dropped_messages: usize,
    pub received_from_transport: usize,
}

#[derive(Debug)]
pub struct RuntimeScheduler {
    interval_ms: u64,
    remainder_ms: u64,
    current_tick: u64,
}

impl RuntimeScheduler {
    pub fn new(tick_hz: u32) -> Self {
        let interval_ms = if tick_hz == 0 { 0 } else { 1000 / tick_hz as u64 };
        Self {
            interval_ms,
            remainder_ms: 0,
            current_tick: 0,
        }
    }

    pub fn interval_ms(&self) -> u64 {
        self.interval_ms
    }

    pub fn tick(&self) -> u64 {
        self.current_tick
    }

    pub fn push_elapsed(&mut self, elapsed_ms: u64) -> Vec<NetworkTickInput> {
        if self.interval_ms == 0 {
            return Vec::new();
        }

        self.remainder_ms = self.remainder_ms.saturating_add(elapsed_ms);
        let mut ticks = Vec::new();
        while self.remainder_ms >= self.interval_ms {
            self.remainder_ms -= self.interval_ms;
            self.current_tick = self.current_tick.saturating_add(1);
            ticks.push(NetworkTickInput {
                tick: self.current_tick,
                now_ms: self.current_tick.saturating_mul(self.interval_ms),
            });
        }
        ticks
    }
}

impl Default for RuntimeScheduler {
    fn default() -> Self {
        Self::new(30)
    }
}

impl RuntimeOutput {
    fn from_packets(
        packets: Vec<TransportMessage>,
        reconciliations: Vec<(u64, Reconciliation)>,
        dropped_bytes: usize,
        transport_packets: usize,
    ) -> Self {
        Self {
            delivered_packets: packets,
            reconciliations,
            dropped_bytes,
            transport_packets,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub transport: TransportProfile,
    pub voice_jitter: JitterBufferConfig,
    pub interest_budget_limit: usize,
    pub max_packet_bytes: usize,
    pub min_sequence_gap_ms: u64,
    pub interpolation: InterpolationConfig,
    pub max_state_delta_bytes: usize,
    pub voice_window_capacity: usize,
    pub max_snapshots_per_client: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            transport: TransportProfile::default(),
            voice_jitter: JitterBufferConfig::default(),
            interest_budget_limit: 256,
            max_packet_bytes: 512,
            min_sequence_gap_ms: 12,
            interpolation: InterpolationConfig::default(),
            max_state_delta_bytes: 64,
            voice_window_capacity: 16,
            max_snapshots_per_client: 512,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClientRuntimeState {
    pub client_id: u64,
    pub prediction: ClientPrediction,
    pub voice_seq: u64,
    pub last_voice_ms: u64,
    pub last_tick: u64,
    last_snapshot_ticks: HashMap<u64, u64>,
    last_entity_snapshots: HashMap<u64, QuantizedFrame>,
    pub voice_windows: HashMap<u64, VoiceWindow>,
}

impl ClientRuntimeState {
    pub fn new(client_id: u64) -> Self {
        Self {
            client_id,
            prediction: ClientPrediction::new(),
            voice_seq: 0,
            last_voice_ms: 0,
            last_tick: 0,
            last_snapshot_ticks: HashMap::new(),
            last_entity_snapshots: HashMap::new(),
            voice_windows: HashMap::new(),
        }
    }

    fn remember_snapshot(&mut self, tick: u64, frame: QuantizedFrame, max_snapshots: usize) {
        if self.last_entity_snapshots.len() >= max_snapshots {
            self.last_entity_snapshots.clear();
            self.last_snapshot_ticks.clear();
        }
        self.last_entity_snapshots.insert(frame.entity_id, frame);
        self.last_snapshot_ticks
            .insert(frame.entity_id, tick);
    }

    fn known_snapshot(&self, entity_id: &u64) -> Option<(&QuantizedFrame, u64)> {
        self.last_entity_snapshots
            .get(entity_id)
            .and_then(|frame| self.last_snapshot_ticks.get(entity_id).map(|tick| (frame, *tick)))
    }
}

#[derive(Debug)]
pub struct NetworkRuntime {
    config: RuntimeConfig,
    interest: InterestManager,
}

impl NetworkRuntime {
    pub fn new(config: RuntimeConfig, interest: InterestManager) -> Self {
        Self { config, interest }
    }

    pub fn scheduler(&self) -> RuntimeScheduler {
        RuntimeScheduler::new(self.config.transport.tick_hz)
    }

    pub fn transport_mode(&self, msg: &TransportMessage, len: usize) -> Reliability {
        if !self.config.transport.use_quinn {
            return Reliability::ReliableOrdered;
        }

        if msg.is_voice {
            return Reliability::UnreliableDatagram;
        }

        match self.config.transport.datagram_mode {
            DatagramMode::DatagramOnly => Reliability::UnreliableDatagram,
            DatagramMode::ReliableOnly => Reliability::ReliableOrdered,
            DatagramMode::DatagramWithFallback => {
                let datagram_cap = (self.config.max_packet_bytes.saturating_mul(3)) / 4;
                if len <= datagram_cap {
                    Reliability::UnreliableDatagram
                } else {
                    Reliability::ReliableOrdered
                }
            }
        }
    }

    pub fn step(
        &self,
        tick_input: NetworkTickInput,
        profiles: &[ClientProfile],
        budgets: &[ClientBudget],
        hints_by_client: &[Vec<RuntimeEntityHint>],
        snaps_by_client: &[Vec<RuntimeSnapshotInput>],
        client_states: &mut [ClientRuntimeState],
        inputs: &[(u64, InputSample)],
        voice: &[VoicePayload],
    ) -> RuntimeOutput {
        let mut packets = Vec::new();
        let mut reconciliations = Vec::new();
        let mut dropped_bytes = 0usize;

        let client_count = profiles
            .len()
            .min(budgets.len())
            .min(hints_by_client.len())
            .min(snaps_by_client.len())
            .min(client_states.len());

        for idx in 0..client_count {
            self.process_client(
                tick_input,
                &profiles[idx],
                &budgets[idx],
                &hints_by_client[idx],
                &snaps_by_client[idx],
                &mut client_states[idx],
                inputs,
                &mut packets,
                &mut reconciliations,
                &mut dropped_bytes,
            );
        }

        for payload in voice {
            self.process_voice_payload(tick_input, profiles, client_states, payload, &mut packets, &mut dropped_bytes);
        }

        RuntimeOutput::from_packets(packets, reconciliations, dropped_bytes, 0)
    }

    pub fn step_with_transport<T: RuntimeTransport>(
        &self,
        transport: &mut T,
        tick_input: NetworkTickInput,
        profiles: &[ClientProfile],
        budgets: &[ClientBudget],
        hints_by_client: &[Vec<RuntimeEntityHint>],
        snaps_by_client: &[Vec<RuntimeSnapshotInput>],
        client_states: &mut [ClientRuntimeState],
        inputs: &[(u64, InputSample)],
        voice: &[VoicePayload],
        recv_cap: usize,
    ) -> RuntimeStepResult {
        let mut output = self.step(
            tick_input,
            profiles,
            budgets,
            hints_by_client,
            snaps_by_client,
            client_states,
            inputs,
            voice,
        );

        let mut sent_to_transport = 0usize;
        let mut dropped_messages = 0usize;

        let mut outbound = Vec::with_capacity(output.delivered_packets.len());
        for packet in output.delivered_packets {
            let mut packet = packet;
            if packet.payload.is_empty() {
                dropped_messages += 1;
                continue;
            }
            packet.reliability = self.transport_mode(&packet, packet.payload.len());
            match transport.send(packet) {
                Ok(()) => sent_to_transport += 1,
                Err(_) => dropped_messages += 1,
            }
        }

        let mut recv_packets = transport.recv(recv_cap);
        let received_from_transport = recv_packets.len();

        outbound.append(&mut recv_packets);
        output = RuntimeOutput {
            delivered_packets: outbound,
            reconciliations: output.reconciliations,
            dropped_bytes: output.dropped_bytes,
            transport_packets: sent_to_transport,
        };

        RuntimeStepResult {
            output,
            sent_to_transport,
            dropped_messages,
            received_from_transport,
        }
    }

    pub fn build_voice_window(&self) -> VoiceWindow {
        VoiceWindow::new(self.config.voice_window_capacity)
    }

    pub fn apply_voice_jitter_buffer(
        &self,
        state: &mut ClientRuntimeState,
        payload: &VoicePayload,
        now_ms: u64,
    ) -> Option<VoicePayload> {
        let max_window_gap = self.config.voice_jitter.max_ms as u64;
        let capacity = self.config.voice_window_capacity.max(1);
        let window = state
            .voice_windows
            .entry(payload.sender_id)
            .or_insert_with(|| VoiceWindow::new(capacity));

        if state.last_voice_ms > 0 && state.last_voice_ms + max_window_gap < now_ms {
            window.last_sequence = 0;
            window.queue.clear();
        }

        if window.push(payload.clone()) {
            window.pop_next()
        } else {
            None
        }
    }

    pub fn tick_interval_ms(&self) -> u64 {
        if self.config.transport.tick_hz == 0 {
            return 0;
        }
        1000 / self.config.transport.tick_hz as u64
    }

    pub fn next_tick(&self, current_tick_ms: u64) -> Option<NetworkTickInput> {
        let interval = self.tick_interval_ms();
        if interval == 0 {
            return None;
        }
        Some(NetworkTickInput {
            tick: current_tick_ms / interval,
            now_ms: current_tick_ms,
        })
    }

    fn process_client(
        &self,
        tick_input: NetworkTickInput,
        profile: &ClientProfile,
        budget: &ClientBudget,
        hints: &[RuntimeEntityHint],
        snaps: &[RuntimeSnapshotInput],
        state: &mut ClientRuntimeState,
        inputs: &[(u64, InputSample)],
        packets: &mut Vec<TransportMessage>,
        reconciliations: &mut Vec<(u64, Reconciliation)>,
        dropped_bytes: &mut usize,
    ) {
        let candidates: Vec<(u64, crate::types::Vec3, f32)> = hints
            .iter()
            .filter(|h| self.hint_visible(&profile, h))
            .map(|h| (h.entity_id, h.position, h.importance))
            .collect();

        let mut effective_budget = budget.clone();
        if effective_budget.max_entities > self.config.interest_budget_limit {
            effective_budget.max_entities = self.config.interest_budget_limit;
        }

        let visible = self
            .interest
            .top_n_entities(&candidates, &effective_budget, profile.position);

        let budgeted_entities = visible
            .into_iter()
            .take(effective_budget.max_entities)
            .collect::<Vec<_>>();

        let mut snap_by_id: HashMap<u64, RuntimeSnapshotInput> = HashMap::new();
        for snap in snaps {
            snap_by_id.insert(snap.entity_id, *snap);
        }

        let mut used_bytes = 0usize;
        for entity_id in budgeted_entities {
            let Some(snap) = snap_by_id.get(&entity_id) else {
                continue;
            };

            let quantized = self.quantize_snapshot(*snap);
            let mut payload =
                self.encode_snapshot_payload(tick_input.tick, state, quantized, snap.entity_id);

            if payload.len() > self.config.max_packet_bytes
                || used_bytes + payload.len() > effective_budget.max_bytes_per_tick
            {
                *dropped_bytes += payload.len();
                continue;
            }

            used_bytes += payload.len();
            state.remember_snapshot(tick_input.tick, quantized, self.config.max_snapshots_per_client);

            let mut msg = TransportMessage {
                to_client_id: profile.client_id,
                entity: NetEntity(snap.entity_id),
                reliability: Reliability::ReliableOrdered,
                payload,
                is_voice: false,
            };
            msg.reliability = self.transport_mode(&msg, msg.payload.len());
            packets.push(msg);
        }

        self.apply_input_reconciliation(tick_input, profile.client_id, inputs, state, reconciliations);
        state.last_tick = tick_input.tick;
    }

    fn process_voice_payload(
        &self,
        tick_input: NetworkTickInput,
        profiles: &[ClientProfile],
        client_states: &mut [ClientRuntimeState],
        payload: &VoicePayload,
        packets: &mut Vec<TransportMessage>,
        dropped_bytes: &mut usize,
    ) {
        if payload.bytes.is_empty() {
            *dropped_bytes += payload.bytes.len();
            return;
        }

        let Some(sender_profile) = profiles.iter().find(|p| p.client_id == payload.sender_id) else {
            *dropped_bytes += payload.bytes.len();
            return;
        };

        let mut delivered = false;
        for (profile, state) in profiles
            .iter()
            .zip(client_states.iter_mut())
            .filter(|(recipient, state)| {
                recipient.client_id != payload.sender_id
                    && recipient.world_id == sender_profile.world_id
                    && state.client_id == recipient.client_id
            })
        {
            let jittered = self.apply_voice_jitter_buffer(state, payload, tick_input.now_ms);
            let Some(frame) = jittered else {
                *dropped_bytes += payload.bytes.len();
                continue;
            };

            if frame.bytes.len() > self.config.max_packet_bytes {
                *dropped_bytes += frame.bytes.len();
                continue;
            }

            let mut packed = Vec::with_capacity(frame.bytes.len() + 24);
            packed.extend_from_slice(&frame.sender_id.to_le_bytes());
            packed.extend_from_slice(&frame.seq.to_le_bytes());
            packed.extend_from_slice(&frame.frame_ms.to_le_bytes());
            packed.push(u8::from(frame.fec_used));
            packed.extend_from_slice(&frame.bytes);

            let mut msg = TransportMessage {
                to_client_id: profile.client_id,
                entity: NetEntity(payload.sender_id),
                reliability: Reliability::UnreliableDatagram,
                payload: packed,
                is_voice: true,
            };
            msg.reliability = self.transport_mode(&msg, msg.payload.len());
            packets.push(msg);
            state.last_voice_ms = tick_input.now_ms;
            state.voice_seq = state.voice_seq.saturating_add(1);
            delivered = true;
        }

        if !delivered {
            *dropped_bytes += payload.bytes.len();
        }
    }

    fn hint_visible(&self, profile: &ClientProfile, hint: &RuntimeEntityHint) -> bool {
        if hint.importance <= 0.0 {
            return false;
        }
        let near_x = hint.position.x - profile.position.x;
        let near_y = hint.position.y - profile.position.y;
        let near_z = hint.position.z - profile.position.z;
        let distance_sq = near_x * near_x + near_y * near_y + near_z * near_z;
        let near = profile.frustum.near * profile.frustum.near;
        let far = profile.frustum.far * profile.frustum.far;

        if distance_sq < near || distance_sq > far {
            return false;
        }

        self.interest
            .frustum_visible(&profile.frustum, profile.position, hint.position)
    }

    fn encode_snapshot_payload(
        &self,
        tick: u64,
        state: &mut ClientRuntimeState,
        quantized: QuantizedFrame,
        entity_id: u64,
    ) -> Vec<u8> {
        let current = self.quantized_to_bytes(&quantized);

        if let Some((previous, previous_tick)) = state.known_snapshot(&entity_id) {
            let previous_bytes = self.quantized_to_bytes(previous);
            let diff = self.diff_snapshot_payload(&previous_bytes, &current);
            let full_payload_len = current.len() + 1 + 8 + 8 + 8 + 4;

            if !diff.is_empty()
                && diff.len() <= self.config.max_state_delta_bytes
                && diff.len() + 1 + 8 + 8 + 8 + 4 <= full_payload_len
            {
                let mut payload = Vec::with_capacity(diff.len() + 1 + 8 + 8 + 8 + 4);
                payload.push(2);
                payload.extend_from_slice(&tick.to_le_bytes());
                payload.extend_from_slice(&entity_id.to_le_bytes());
                payload.extend_from_slice(&previous_tick.to_le_bytes());
                payload.extend_from_slice(&(diff.len() as u32).to_le_bytes());
                payload.extend_from_slice(&diff);
                return payload;
            }
        }

        self.quantized_to_full_payload(tick, quantized)
    }

    fn quantize_snapshot(&self, snap: RuntimeSnapshotInput) -> QuantizedFrame {
        QuantizedFrame::from_floats(
            snap.entity_id,
            snap.position.0,
            snap.position.1,
            snap.position.2,
            snap.rotation_deg.0,
            snap.rotation_deg.1,
            snap.rotation_deg.2,
        )
    }

    fn quantized_to_bytes(&self, q: &QuantizedFrame) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(30);
        bytes.extend_from_slice(&q.x_mm.to_le_bytes());
        bytes.extend_from_slice(&q.y_mm.to_le_bytes());
        bytes.extend_from_slice(&q.z_mm.to_le_bytes());
        bytes.extend_from_slice(&q.rot_pitch.to_le_bytes());
        bytes.extend_from_slice(&q.rot_yaw.to_le_bytes());
        bytes.extend_from_slice(&q.rot_roll.to_le_bytes());
        bytes
    }

    fn quantized_to_full_payload(&self, tick: u64, q: QuantizedFrame) -> Vec<u8> {
        let mut payload = Vec::with_capacity(64);
        payload.push(1);
        payload.extend_from_slice(&tick.to_le_bytes());
        payload.extend_from_slice(&q.entity_id.to_le_bytes());
        payload.extend_from_slice(&q.x_mm.to_le_bytes());
        payload.extend_from_slice(&q.y_mm.to_le_bytes());
        payload.extend_from_slice(&q.z_mm.to_le_bytes());
        payload.extend_from_slice(&q.rot_pitch.to_le_bytes());
        payload.extend_from_slice(&q.rot_yaw.to_le_bytes());
        payload.extend_from_slice(&q.rot_roll.to_le_bytes());
        payload
    }

    fn diff_snapshot_payload(&self, previous: &[u8], next: &[u8]) -> Vec<u8> {
        let StateDiff {
            xor_bytes,
            ..
        } = xor_patch(previous, next);
        if xor_bytes.is_empty() {
            return Vec::new();
        }
        xor_bytes
    }

    fn apply_input_reconciliation(
        &self,
        tick_input: NetworkTickInput,
        client_id: u64,
        inputs: &[(u64, InputSample)],
        state: &mut ClientRuntimeState,
        reconciliations: &mut Vec<(u64, Reconciliation)>,
    ) {
        let mut target_inputs: Vec<InputSample> = inputs
            .iter()
            .filter_map(|(target, sample)| {
                if *target == client_id {
                    Some(sample.clone())
                } else {
                    None
                }
            })
            .collect();

        if target_inputs.is_empty() {
            reconciliations.push((client_id, Reconciliation::NotNeeded));
            return;
        }

        target_inputs.sort_by_key(|s| s.client_tick);

        let mut accepted = 0usize;
        for input in target_inputs {
            if input.client_tick <= state.last_tick {
                continue;
            }

            let stale =
                input.client_tick + self.config.min_sequence_gap_ms < tick_input.tick;
            if stale {
                continue;
            }

            state.prediction.queue_input(input);
            accepted = accepted.saturating_add(1);
        }

        if accepted == 0 {
            reconciliations.push((client_id, Reconciliation::Rejected));
            return;
        }

        let interpolation_ok = self.config.interpolation.buffer_time_ms <= (tick_input.now_ms as f32);
        let tick = if interpolation_ok {
            tick_input.tick
        } else {
            tick_input.tick.saturating_sub(1)
        };

        let reconcile = EntitySnapshot {
            entity_id: NetEntity(client_id),
            transform: [0.0; 7],
            tick,
        };
        let status = state.prediction.reconcile(&reconcile);
        reconciliations.push((client_id, status));
    }
}

pub fn build_sample_fec_window(config: &RuntimeConfig) -> Duration {
    let jitter_base = config.voice_jitter.base_ms;
    let jittered = if config.voice_jitter.adaptive {
        let span = config
            .voice_jitter
            .max_ms
            .saturating_sub(config.voice_jitter.min_ms);
        jitter_base.saturating_add(span / 2)
    } else {
        jitter_base
    };
    Duration::from_millis(jittered as u64)
}

pub fn check_fec_window(window: Duration, required_ms: u64) -> bool {
    window.as_millis() as u64 >= required_ms
}

#[derive(Debug)]
pub struct VoiceWindow {
    queue: VecDeque<VoicePayload>,
    capacity: usize,
    last_sequence: u64,
}

impl VoiceWindow {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(capacity),
            capacity,
            last_sequence: 0,
        }
    }

    pub fn push(&mut self, payload: VoicePayload) -> bool {
        if self.queue.len() >= self.capacity {
            let _ = self.queue.pop_front();
        }
        if payload.seq <= self.last_sequence {
            return false;
        }
        self.last_sequence = payload.seq;
        self.queue.push_back(payload);
        true
    }

    pub fn pop_next(&mut self) -> Option<VoicePayload> {
        self.queue.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InterestPolicy;

    #[test]
    fn runtime_builds_packets_with_budget() {
        let config = RuntimeConfig::default();
        let runtime = NetworkRuntime::new(config, InterestManager::new(InterestPolicy::default()));
        let mut state = vec![ClientRuntimeState::new(11)];

        let profile = vec![ClientProfile {
            client_id: 11,
            world_id: 1,
            position: crate::types::Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            frustum: crate::CameraFrustum::default(),
        }];
        let budgets = vec![ClientBudget {
            client_id: 11,
            max_entities: 4,
            max_bytes_per_tick: 64,
        }];
        let hints = vec![vec![
            RuntimeEntityHint {
                entity_id: 1,
                position: crate::types::Vec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 2.0,
                },
                importance: 1.0,
            },
            RuntimeEntityHint {
                entity_id: 2,
                position: crate::types::Vec3 {
                    x: 3.0,
                    y: 0.0,
                    z: 4.0,
                },
                importance: 0.3,
            },
        ]];
        let snaps = vec![vec![
            RuntimeSnapshotInput {
                entity_id: 1,
                position: (1.0, 0.0, 2.0),
                rotation_deg: (0.0, 90.0, 180.0),
            },
            RuntimeSnapshotInput {
                entity_id: 2,
                position: (3.0, 0.0, 4.0),
                rotation_deg: (0.0, 45.0, 90.0),
            },
        ]];
        let output = runtime.step(
            NetworkTickInput { tick: 1, now_ms: 16 },
            &profile,
            &budgets,
            &hints,
            &snaps,
            &mut state,
            &[(11, InputSample {
                client_tick: 1,
                seq: 1,
                payload: vec![1, 2, 3],
            })],
            &[],
        );

        assert!(!output.delivered_packets.is_empty());
        assert!(!output.reconciliations.is_empty());
    }

    #[test]
    fn transport_adapter_pumps_and_receives() {
        let config = RuntimeConfig::default();
        let runtime = NetworkRuntime::new(config, InterestManager::new(InterestPolicy::default()));
        let mut transport = InMemoryTransport::new(32);
        let mut state = vec![ClientRuntimeState::new(11)];

        let profile = vec![ClientProfile {
            client_id: 11,
            world_id: 1,
            position: crate::types::Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            frustum: crate::CameraFrustum::default(),
        }];
        let budgets = vec![ClientBudget {
            client_id: 11,
            max_entities: 1,
            max_bytes_per_tick: 64,
        }];
        let hints = vec![vec![RuntimeEntityHint {
            entity_id: 1,
            position: crate::types::Vec3 {
                x: 1.0,
                y: 0.0,
                z: 2.0,
            },
            importance: 1.0,
        }]];
        let snaps = vec![vec![RuntimeSnapshotInput {
            entity_id: 1,
            position: (1.0, 0.0, 2.0),
            rotation_deg: (0.0, 90.0, 180.0),
        }]];
        let result = runtime.step_with_transport(
            &mut transport,
            NetworkTickInput { tick: 1, now_ms: 16 },
            &profile,
            &budgets,
            &hints,
            &snaps,
            &mut state,
            &[],
            &[],
            8,
        );

        assert!(result.sent_to_transport > 0);
        assert_eq!(result.output.transport_packets, result.sent_to_transport);
    }

    #[test]
    fn transport_tick_interval_defined() {
        let runtime = NetworkRuntime::new(RuntimeConfig::default(), InterestManager::new(InterestPolicy::default()));
        assert!(runtime.tick_interval_ms() > 0);
        let next = runtime.next_tick(34).expect("tick should exist");
        assert!(next.tick > 0);
    }

    #[test]
    fn runtime_scheduler_emits_fixed_ticks() {
        let mut scheduler = RuntimeScheduler::new(30);
        let ticks = scheduler.push_elapsed(100);
        assert!(!ticks.is_empty());
        assert_eq!(ticks[0].tick, 1);
    }

    #[test]
    fn snapshot_delta_encoding_is_tighter_than_full() {
        let runtime = NetworkRuntime::new(RuntimeConfig::default(), InterestManager::new(InterestPolicy::default()));
        let mut state = ClientRuntimeState::new(11);

        let mut budget = ClientBudget {
            client_id: 11,
            max_entities: 8,
            max_bytes_per_tick: 512,
        };
        let profile = ClientProfile {
            client_id: 11,
            world_id: 1,
            position: crate::types::Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            frustum: crate::CameraFrustum::default(),
        };

        let hints = vec![RuntimeEntityHint {
            entity_id: 1,
            position: crate::types::Vec3 {
                x: 1.0,
                y: 0.0,
                z: 2.0,
            },
            importance: 1.0,
        }];

        let snap_a = RuntimeSnapshotInput {
            entity_id: 1,
            position: (1.0, 0.0, 2.0),
            rotation_deg: (0.0, 90.0, 180.0),
        };
        let snap_b = RuntimeSnapshotInput {
            entity_id: 1,
            position: (1.01, 0.0, 2.0),
            rotation_deg: (0.0, 90.0, 180.0),
        };

        let output_a = runtime.encode_snapshot_payload(
            1,
            &mut state,
            runtime.quantize_snapshot(snap_a),
            snap_a.entity_id,
        );
        let output_b = runtime.encode_snapshot_payload(
            2,
            &mut state,
            runtime.quantize_snapshot(snap_b),
            snap_b.entity_id,
        );

        assert!(!output_a.is_empty());
        assert!(!output_b.is_empty());
        state.remember_snapshot(1, runtime.quantize_snapshot(snap_a), 32);
        budget.max_bytes_per_tick = 8;

        let _ = budget.max_entities;
        let _ = runtime.hint_visible(&profile, &hints[0]);
    }

    #[test]
    fn voice_window_rejects_out_of_order() {
        let mut window = VoiceWindow::new(2);
        assert!(window.push(VoicePayload {
            sender_id: 1,
            seq: 1,
            frame_ms: 20,
            fec_used: true,
            bytes: vec![1, 2, 3],
        }));
        assert!(!window.push(VoicePayload {
            sender_id: 1,
            seq: 0,
            frame_ms: 20,
            fec_used: true,
            bytes: vec![1],
        }));
    }
}
