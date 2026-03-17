//! Network protocol messages for multiplayer communication.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::avatar::AvatarState;

/// Player identifier type (re-export for convenience).
pub type PlayerId = Uuid;

/// Messages sent from client to server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClientMessage {
    /// Player input update with avatar state.
    InputUpdate {
        tick: u64,
        avatar: AvatarState,
    },
    /// Ping for latency measurement.
    Ping {
        client_time_ms: u64,
    },
}

/// Messages sent from server to client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServerMessage {
    /// Incremental world state update.
    WorldState {
        tick: u64,
        avatars: Vec<(PlayerId, AvatarState)>,
    },
    /// Notification that a player joined.
    PlayerJoined {
        player_id: PlayerId,
    },
    /// Notification that a player left.
    PlayerLeft {
        player_id: PlayerId,
    },
    /// Pong response for latency measurement.
    Pong {
        client_time_ms: u64,
        server_time_ms: u64,
    },
    /// Full state sync (sent on initial connection).
    FullSync {
        tick: u64,
        avatars: Vec<(PlayerId, AvatarState)>,
    },
}

/// Serialize a client message to bytes.
pub fn encode_client_message(msg: &ClientMessage) -> Result<Vec<u8>, bincode::Error> {
    bincode::serialize(msg)
}

/// Deserialize a client message from bytes.
pub fn decode_client_message(data: &[u8]) -> Result<ClientMessage, bincode::Error> {
    bincode::deserialize(data)
}

/// Serialize a server message to bytes.
pub fn encode_server_message(msg: &ServerMessage) -> Result<Vec<u8>, bincode::Error> {
    bincode::serialize(msg)
}

/// Deserialize a server message from bytes.
pub fn decode_server_message(data: &[u8]) -> Result<ServerMessage, bincode::Error> {
    bincode::deserialize(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_input_update_roundtrip() {
        let msg = ClientMessage::InputUpdate {
            tick: 42,
            avatar: AvatarState::default(),
        };
        let bytes = encode_client_message(&msg).unwrap();
        let decoded = decode_client_message(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn client_ping_roundtrip() {
        let msg = ClientMessage::Ping {
            client_time_ms: 123456,
        };
        let bytes = encode_client_message(&msg).unwrap();
        let decoded = decode_client_message(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn server_world_state_roundtrip() {
        let pid = Uuid::new_v4();
        let msg = ServerMessage::WorldState {
            tick: 100,
            avatars: vec![(pid, AvatarState::default())],
        };
        let bytes = encode_server_message(&msg).unwrap();
        let decoded = decode_server_message(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn server_player_joined_roundtrip() {
        let pid = Uuid::new_v4();
        let msg = ServerMessage::PlayerJoined { player_id: pid };
        let bytes = encode_server_message(&msg).unwrap();
        let decoded = decode_server_message(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn server_player_left_roundtrip() {
        let pid = Uuid::new_v4();
        let msg = ServerMessage::PlayerLeft { player_id: pid };
        let bytes = encode_server_message(&msg).unwrap();
        let decoded = decode_server_message(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn server_pong_roundtrip() {
        let msg = ServerMessage::Pong {
            client_time_ms: 100,
            server_time_ms: 200,
        };
        let bytes = encode_server_message(&msg).unwrap();
        let decoded = decode_server_message(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn server_full_sync_roundtrip() {
        let pid1 = Uuid::new_v4();
        let pid2 = Uuid::new_v4();
        let msg = ServerMessage::FullSync {
            tick: 50,
            avatars: vec![
                (pid1, AvatarState::default()),
                (pid2, AvatarState::default()),
            ],
        };
        let bytes = encode_server_message(&msg).unwrap();
        let decoded = decode_server_message(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn server_world_state_empty_avatars() {
        let msg = ServerMessage::WorldState {
            tick: 1,
            avatars: vec![],
        };
        let bytes = encode_server_message(&msg).unwrap();
        let decoded = decode_server_message(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn decode_invalid_bytes_returns_error() {
        let result = decode_client_message(&[0xFF, 0xFF, 0xFF]);
        assert!(result.is_err());
    }

    #[test]
    fn decode_empty_bytes_returns_error() {
        let result = decode_client_message(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn server_message_decode_invalid_returns_error() {
        let result = decode_server_message(&[0xFF, 0xFF]);
        assert!(result.is_err());
    }

    #[test]
    fn multiple_avatars_in_world_state() {
        let avatars: Vec<(PlayerId, AvatarState)> = (0..20)
            .map(|_| (Uuid::new_v4(), AvatarState::default()))
            .collect();
        let msg = ServerMessage::WorldState {
            tick: 999,
            avatars: avatars.clone(),
        };
        let bytes = encode_server_message(&msg).unwrap();
        let decoded = decode_server_message(&bytes).unwrap();
        if let ServerMessage::WorldState { tick, avatars: decoded_avatars } = decoded {
            assert_eq!(tick, 999);
            assert_eq!(decoded_avatars.len(), 20);
        } else {
            panic!("wrong variant");
        }
    }
}
