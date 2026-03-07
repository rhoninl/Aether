use crate::types::NetEntity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reliability {
    ReliableOrdered,
    UnreliableDatagram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatagramMode {
    DatagramOnly,
    DatagramWithFallback,
    ReliableOnly,
}

#[derive(Debug, Clone)]
pub struct TransportProfile {
    pub tick_hz: u32,
    pub use_quinn: bool,
    pub max_retransmissions: u8,
    pub datagram_mode: DatagramMode,
}

impl Default for TransportProfile {
    fn default() -> Self {
        Self {
            tick_hz: 30,
            use_quinn: true,
            max_retransmissions: 2,
            datagram_mode: DatagramMode::DatagramWithFallback,
        }
    }
}

#[derive(Debug)]
pub struct TransportMessage {
    pub to_client_id: u64,
    pub entity: NetEntity,
    pub reliability: Reliability,
    pub payload: Vec<u8>,
    pub is_voice: bool,
}
