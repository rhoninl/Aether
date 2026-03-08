use std::collections::VecDeque;

use crate::runtime::{RuntimeTransport, TransportError};
use crate::transport::{Reliability, TransportMessage};
use crate::types::NetEntity;

/// A QUIC-backed implementation of RuntimeTransport.
///
/// This adapter buffers outbound messages for later flushing over the QUIC
/// connection, and collects inbound messages from datagrams and streams.
///
/// It works in both client and server modes:
/// - In client mode: all messages go to the server (single connection).
/// - In server mode: messages are routed by `to_client_id`.
///
/// The actual QUIC I/O happens asynchronously; this struct provides the
/// synchronous RuntimeTransport interface by buffering messages.
pub struct QuicTransport {
    reliable_outbound: VecDeque<TransportMessage>,
    datagram_outbound: VecDeque<TransportMessage>,
    inbound: VecDeque<TransportMessage>,
    max_outbound: usize,
    max_datagram_size: usize,
}

impl std::fmt::Debug for QuicTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuicTransport")
            .field("reliable_outbound_len", &self.reliable_outbound.len())
            .field("datagram_outbound_len", &self.datagram_outbound.len())
            .field("inbound_len", &self.inbound.len())
            .field("max_outbound", &self.max_outbound)
            .finish()
    }
}

/// Default maximum number of outbound messages buffered.
const DEFAULT_MAX_OUTBOUND: usize = 4096;

impl QuicTransport {
    /// Create a new QuicTransport with the given maximum outbound buffer size.
    pub fn new(max_outbound: usize) -> Self {
        Self {
            reliable_outbound: VecDeque::new(),
            datagram_outbound: VecDeque::new(),
            inbound: VecDeque::new(),
            max_outbound,
            max_datagram_size: super::config::MAX_DATAGRAM_SIZE,
        }
    }

    /// Get the number of reliable messages waiting to be sent.
    pub fn reliable_outbound_len(&self) -> usize {
        self.reliable_outbound.len()
    }

    /// Get the number of datagram messages waiting to be sent.
    pub fn datagram_outbound_len(&self) -> usize {
        self.datagram_outbound.len()
    }

    /// Get the total number of outbound messages.
    pub fn outbound_len(&self) -> usize {
        self.reliable_outbound.len() + self.datagram_outbound.len()
    }

    /// Pop the next reliable outbound message (for async sender to consume).
    pub fn pop_reliable_outbound(&mut self) -> Option<TransportMessage> {
        self.reliable_outbound.pop_front()
    }

    /// Pop the next datagram outbound message (for async sender to consume).
    pub fn pop_datagram_outbound(&mut self) -> Option<TransportMessage> {
        self.datagram_outbound.pop_front()
    }

    /// Push a message into the inbound buffer (called by async receiver).
    pub fn push_inbound(&mut self, msg: TransportMessage) {
        self.inbound.push_back(msg);
    }

    /// Drain all reliable outbound messages into a vec.
    pub fn drain_reliable_outbound(&mut self) -> Vec<TransportMessage> {
        self.reliable_outbound.drain(..).collect()
    }

    /// Drain all datagram outbound messages into a vec.
    pub fn drain_datagram_outbound(&mut self) -> Vec<TransportMessage> {
        self.datagram_outbound.drain(..).collect()
    }
}

impl Default for QuicTransport {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_OUTBOUND)
    }
}

impl RuntimeTransport for QuicTransport {
    fn send(&mut self, msg: TransportMessage) -> Result<(), TransportError> {
        let total = self.reliable_outbound.len() + self.datagram_outbound.len();
        if total >= self.max_outbound {
            return Err(TransportError::CapacityExceeded);
        }

        if msg.payload.len() > super::config::MAX_STREAM_CHUNK_SIZE {
            return Err(TransportError::MessageTooLarge(msg.payload.len()));
        }

        match msg.reliability {
            Reliability::ReliableOrdered => {
                self.reliable_outbound.push_back(msg);
            }
            Reliability::UnreliableDatagram => {
                // If the datagram is too large, promote it to reliable
                if msg.payload.len() > self.max_datagram_size {
                    let promoted = TransportMessage {
                        to_client_id: msg.to_client_id,
                        entity: msg.entity,
                        reliability: Reliability::ReliableOrdered,
                        payload: msg.payload,
                        is_voice: msg.is_voice,
                    };
                    self.reliable_outbound.push_back(promoted);
                } else {
                    self.datagram_outbound.push_back(msg);
                }
            }
        }

        Ok(())
    }

    fn recv(&mut self, max: usize) -> Vec<TransportMessage> {
        let count = max.min(self.inbound.len());
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            if let Some(msg) = self.inbound.pop_front() {
                out.push(msg);
            }
        }
        out
    }

    fn flush(&mut self) {
        // Flushing is a no-op for the synchronous interface.
        // The async pump task handles actual network I/O.
    }
}

/// Helper to create a TransportMessage from raw bytes (useful for receive path).
pub fn bytes_to_transport_message(
    client_id: u64,
    entity_id: u64,
    reliability: Reliability,
    payload: Vec<u8>,
    is_voice: bool,
) -> TransportMessage {
    TransportMessage {
        to_client_id: client_id,
        entity: NetEntity(entity_id),
        reliability,
        payload,
        is_voice,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quic_transport_send_reliable() {
        let mut transport = QuicTransport::new(100);
        let msg = TransportMessage {
            to_client_id: 1,
            entity: NetEntity(10),
            reliability: Reliability::ReliableOrdered,
            payload: vec![1, 2, 3],
            is_voice: false,
        };

        let result = transport.send(msg);
        assert!(result.is_ok());
        assert_eq!(transport.reliable_outbound_len(), 1);
        assert_eq!(transport.datagram_outbound_len(), 0);
    }

    #[test]
    fn quic_transport_send_datagram() {
        let mut transport = QuicTransport::new(100);
        let msg = TransportMessage {
            to_client_id: 1,
            entity: NetEntity(10),
            reliability: Reliability::UnreliableDatagram,
            payload: vec![1, 2, 3],
            is_voice: false,
        };

        let result = transport.send(msg);
        assert!(result.is_ok());
        assert_eq!(transport.datagram_outbound_len(), 1);
        assert_eq!(transport.reliable_outbound_len(), 0);
    }

    #[test]
    fn quic_transport_large_datagram_promoted_to_reliable() {
        let mut transport = QuicTransport::new(100);
        let large_payload = vec![0u8; super::super::config::MAX_DATAGRAM_SIZE + 1];
        let msg = TransportMessage {
            to_client_id: 1,
            entity: NetEntity(10),
            reliability: Reliability::UnreliableDatagram,
            payload: large_payload,
            is_voice: false,
        };

        let result = transport.send(msg);
        assert!(result.is_ok());
        // Should be promoted to reliable
        assert_eq!(transport.reliable_outbound_len(), 1);
        assert_eq!(transport.datagram_outbound_len(), 0);
    }

    #[test]
    fn quic_transport_capacity_exceeded() {
        let mut transport = QuicTransport::new(1);
        let msg1 = TransportMessage {
            to_client_id: 1,
            entity: NetEntity(10),
            reliability: Reliability::ReliableOrdered,
            payload: vec![1],
            is_voice: false,
        };
        let msg2 = TransportMessage {
            to_client_id: 1,
            entity: NetEntity(11),
            reliability: Reliability::ReliableOrdered,
            payload: vec![2],
            is_voice: false,
        };

        assert!(transport.send(msg1).is_ok());
        let result = transport.send(msg2);
        assert!(matches!(result, Err(TransportError::CapacityExceeded)));
    }

    #[test]
    fn quic_transport_message_too_large() {
        let mut transport = QuicTransport::new(100);
        let huge_payload = vec![0u8; super::super::config::MAX_STREAM_CHUNK_SIZE + 1];
        let msg = TransportMessage {
            to_client_id: 1,
            entity: NetEntity(10),
            reliability: Reliability::ReliableOrdered,
            payload: huge_payload,
            is_voice: false,
        };

        let result = transport.send(msg);
        assert!(matches!(result, Err(TransportError::MessageTooLarge(_))));
    }

    #[test]
    fn quic_transport_recv_returns_inbound() {
        let mut transport = QuicTransport::new(100);
        transport.push_inbound(TransportMessage {
            to_client_id: 1,
            entity: NetEntity(10),
            reliability: Reliability::ReliableOrdered,
            payload: vec![1, 2, 3],
            is_voice: false,
        });
        transport.push_inbound(TransportMessage {
            to_client_id: 1,
            entity: NetEntity(11),
            reliability: Reliability::UnreliableDatagram,
            payload: vec![4, 5],
            is_voice: true,
        });

        let msgs = transport.recv(10);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].payload, vec![1, 2, 3]);
        assert_eq!(msgs[1].payload, vec![4, 5]);
    }

    #[test]
    fn quic_transport_recv_respects_max() {
        let mut transport = QuicTransport::new(100);
        for i in 0..5 {
            transport.push_inbound(TransportMessage {
                to_client_id: 1,
                entity: NetEntity(i),
                reliability: Reliability::ReliableOrdered,
                payload: vec![i as u8],
                is_voice: false,
            });
        }

        let msgs = transport.recv(3);
        assert_eq!(msgs.len(), 3);
        // Should still have 2 remaining
        assert_eq!(transport.recv(10).len(), 2);
    }

    #[test]
    fn quic_transport_pop_outbound() {
        let mut transport = QuicTransport::new(100);
        transport
            .send(TransportMessage {
                to_client_id: 1,
                entity: NetEntity(10),
                reliability: Reliability::ReliableOrdered,
                payload: vec![1],
                is_voice: false,
            })
            .unwrap();
        transport
            .send(TransportMessage {
                to_client_id: 1,
                entity: NetEntity(11),
                reliability: Reliability::UnreliableDatagram,
                payload: vec![2],
                is_voice: false,
            })
            .unwrap();

        let reliable = transport.pop_reliable_outbound();
        assert!(reliable.is_some());
        assert_eq!(reliable.unwrap().payload, vec![1]);

        let datagram = transport.pop_datagram_outbound();
        assert!(datagram.is_some());
        assert_eq!(datagram.unwrap().payload, vec![2]);
    }

    #[test]
    fn quic_transport_drain_outbound() {
        let mut transport = QuicTransport::new(100);
        for i in 0..3 {
            transport
                .send(TransportMessage {
                    to_client_id: 1,
                    entity: NetEntity(i),
                    reliability: Reliability::ReliableOrdered,
                    payload: vec![i as u8],
                    is_voice: false,
                })
                .unwrap();
        }

        let drained = transport.drain_reliable_outbound();
        assert_eq!(drained.len(), 3);
        assert_eq!(transport.reliable_outbound_len(), 0);
    }

    #[test]
    fn quic_transport_default() {
        let transport = QuicTransport::default();
        assert_eq!(transport.outbound_len(), 0);
        assert_eq!(transport.max_outbound, DEFAULT_MAX_OUTBOUND);
    }

    #[test]
    fn bytes_to_transport_message_creates_correct_message() {
        let msg = bytes_to_transport_message(42, 100, Reliability::ReliableOrdered, vec![1, 2, 3], false);
        assert_eq!(msg.to_client_id, 42);
        assert_eq!(msg.entity, NetEntity(100));
        assert_eq!(msg.payload, vec![1, 2, 3]);
        assert!(!msg.is_voice);
    }
}
