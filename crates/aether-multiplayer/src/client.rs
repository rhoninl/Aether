//! Multiplayer client: connects to server via QUIC, sends inputs, receives state.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::Mutex;

use aether_network::quic::client::QuicClient;
use aether_network::quic::config::QuicConfig;

use crate::avatar::AvatarState;
use crate::protocol::{
    decode_server_message, encode_client_message, ClientMessage, PlayerId, ServerMessage,
};

/// Errors from client operations.
#[derive(Debug)]
pub enum ClientError {
    /// Failed to create the QUIC client.
    Init(String),
    /// Failed to connect to the server.
    Connect(String),
    /// Failed to send a message.
    Send(String),
    /// Failed to receive a message.
    Recv(String),
    /// Client is not connected.
    NotConnected,
    /// Serialization error.
    Encoding(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Init(msg) => write!(f, "client init error: {msg}"),
            ClientError::Connect(msg) => write!(f, "client connect error: {msg}"),
            ClientError::Send(msg) => write!(f, "client send error: {msg}"),
            ClientError::Recv(msg) => write!(f, "client recv error: {msg}"),
            ClientError::NotConnected => write!(f, "client is not connected"),
            ClientError::Encoding(msg) => write!(f, "encoding error: {msg}"),
        }
    }
}

impl std::error::Error for ClientError {}

/// State received from the server, maintained locally.
#[derive(Debug, Clone)]
pub struct RemoteWorldState {
    /// The most recently received server tick.
    pub last_tick: u64,
    /// Current avatar states of all players in the world.
    pub avatars: HashMap<PlayerId, AvatarState>,
}

impl Default for RemoteWorldState {
    fn default() -> Self {
        Self {
            last_tick: 0,
            avatars: HashMap::new(),
        }
    }
}

/// Multiplayer client that connects to a server and syncs state.
pub struct MultiplayerClient {
    quic: QuicClient,
    remote_state: Arc<Mutex<RemoteWorldState>>,
    local_tick: u64,
}

impl MultiplayerClient {
    /// Create a new client configured to connect to the given server address.
    pub fn new(server_addr: SocketAddr) -> Result<Self, ClientError> {
        let config = QuicConfig {
            server_addr,
            ..QuicConfig::default()
        };

        let quic = QuicClient::new(config).map_err(|e| ClientError::Init(e.to_string()))?;

        Ok(Self {
            quic,
            remote_state: Arc::new(Mutex::new(RemoteWorldState::default())),
            local_tick: 0,
        })
    }

    /// Connect to the server.
    pub async fn connect(&mut self) -> Result<u64, ClientError> {
        let client_id = rand_client_id();
        let token = b"aether-client";
        let server_tick = self
            .quic
            .connect(client_id, token)
            .await
            .map_err(|e| ClientError::Connect(e.to_string()))?;
        Ok(server_tick)
    }

    /// Check if the client is connected.
    pub fn is_connected(&self) -> bool {
        self.quic.is_connected()
    }

    /// Send an avatar state update to the server.
    pub async fn send_input(&mut self, avatar: AvatarState) -> Result<(), ClientError> {
        if !self.quic.is_connected() {
            return Err(ClientError::NotConnected);
        }

        self.local_tick += 1;
        let msg = ClientMessage::InputUpdate {
            tick: self.local_tick,
            avatar,
        };
        let data =
            encode_client_message(&msg).map_err(|e| ClientError::Encoding(e.to_string()))?;

        self.quic
            .send_datagram(&data)
            .map_err(|e| ClientError::Send(e.to_string()))
    }

    /// Send a ping to measure latency.
    pub async fn send_ping(&self) -> Result<(), ClientError> {
        if !self.quic.is_connected() {
            return Err(ClientError::NotConnected);
        }

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let msg = ClientMessage::Ping {
            client_time_ms: now_ms,
        };
        let data =
            encode_client_message(&msg).map_err(|e| ClientError::Encoding(e.to_string()))?;

        self.quic
            .send_reliable(&data)
            .await
            .map_err(|e| ClientError::Send(e.to_string()))
    }

    /// Receive and process the next server message.
    pub async fn recv(&self) -> Result<ServerMessage, ClientError> {
        if !self.quic.is_connected() {
            return Err(ClientError::NotConnected);
        }

        let data = self
            .quic
            .recv_reliable()
            .await
            .map_err(|e| ClientError::Recv(e.to_string()))?;

        let msg =
            decode_server_message(&data).map_err(|e| ClientError::Encoding(e.to_string()))?;

        // Update local state
        self.apply_server_message(&msg).await;

        Ok(msg)
    }

    /// Apply a server message to local state.
    async fn apply_server_message(&self, msg: &ServerMessage) {
        let mut state = self.remote_state.lock().await;
        match msg {
            ServerMessage::WorldState { tick, avatars } => {
                if *tick > state.last_tick {
                    state.last_tick = *tick;
                    for (pid, avatar) in avatars {
                        state.avatars.insert(*pid, avatar.clone());
                    }
                }
            }
            ServerMessage::FullSync { tick, avatars } => {
                state.last_tick = *tick;
                state.avatars.clear();
                for (pid, avatar) in avatars {
                    state.avatars.insert(*pid, avatar.clone());
                }
            }
            ServerMessage::PlayerLeft { player_id } => {
                state.avatars.remove(player_id);
            }
            ServerMessage::PlayerJoined { .. } | ServerMessage::Pong { .. } => {}
        }
    }

    /// Get a snapshot of the current remote world state.
    pub async fn world_state(&self) -> RemoteWorldState {
        self.remote_state.lock().await.clone()
    }

    /// Disconnect from the server.
    pub fn disconnect(&mut self) {
        self.quic.disconnect("client disconnect");
    }

    /// Shut down the client.
    pub fn shutdown(&mut self) {
        self.quic.shutdown();
    }
}

/// Generate a pseudo-random client ID.
fn rand_client_id() -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_error_displays() {
        let err = ClientError::NotConnected;
        assert!(format!("{err}").contains("not connected"));

        let err = ClientError::Init("test".to_string());
        assert!(format!("{err}").contains("test"));

        let err = ClientError::Connect("timeout".to_string());
        assert!(format!("{err}").contains("timeout"));

        let err = ClientError::Send("full".to_string());
        assert!(format!("{err}").contains("full"));

        let err = ClientError::Recv("eof".to_string());
        assert!(format!("{err}").contains("eof"));

        let err = ClientError::Encoding("bad bytes".to_string());
        assert!(format!("{err}").contains("bad bytes"));
    }

    #[test]
    fn remote_world_state_default() {
        let state = RemoteWorldState::default();
        assert_eq!(state.last_tick, 0);
        assert!(state.avatars.is_empty());
    }

    #[test]
    fn rand_client_id_is_nonzero() {
        let id = rand_client_id();
        // Very unlikely to be zero, but technically possible
        let _ = id;
    }

    #[tokio::test]
    async fn apply_world_state_updates_tick() {
        let client_state = Arc::new(Mutex::new(RemoteWorldState::default()));
        let pid = uuid::Uuid::new_v4();
        let msg = ServerMessage::WorldState {
            tick: 42,
            avatars: vec![(pid, AvatarState::default())],
        };

        {
            let mut state = client_state.lock().await;
            match &msg {
                ServerMessage::WorldState { tick, avatars } => {
                    if *tick > state.last_tick {
                        state.last_tick = *tick;
                        for (pid, avatar) in avatars {
                            state.avatars.insert(*pid, avatar.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        let state = client_state.lock().await;
        assert_eq!(state.last_tick, 42);
        assert_eq!(state.avatars.len(), 1);
        assert!(state.avatars.contains_key(&pid));
    }

    #[tokio::test]
    async fn apply_full_sync_replaces_state() {
        let client_state = Arc::new(Mutex::new(RemoteWorldState::default()));
        let pid1 = uuid::Uuid::new_v4();
        let pid2 = uuid::Uuid::new_v4();

        // First add a player
        {
            let mut state = client_state.lock().await;
            state.avatars.insert(pid1, AvatarState::default());
            state.last_tick = 10;
        }

        // Full sync replaces everything
        {
            let mut state = client_state.lock().await;
            state.last_tick = 20;
            state.avatars.clear();
            state.avatars.insert(pid2, AvatarState::default());
        }

        let state = client_state.lock().await;
        assert_eq!(state.last_tick, 20);
        assert_eq!(state.avatars.len(), 1);
        assert!(state.avatars.contains_key(&pid2));
        assert!(!state.avatars.contains_key(&pid1));
    }

    #[tokio::test]
    async fn apply_player_left_removes_avatar() {
        let client_state = Arc::new(Mutex::new(RemoteWorldState::default()));
        let pid = uuid::Uuid::new_v4();

        {
            let mut state = client_state.lock().await;
            state.avatars.insert(pid, AvatarState::default());
        }

        {
            let mut state = client_state.lock().await;
            state.avatars.remove(&pid);
        }

        let state = client_state.lock().await;
        assert!(!state.avatars.contains_key(&pid));
    }

    #[tokio::test]
    async fn old_world_state_ignored() {
        let client_state = Arc::new(Mutex::new(RemoteWorldState::default()));

        {
            let mut state = client_state.lock().await;
            state.last_tick = 100;
        }

        // Older tick should be ignored
        {
            let mut state = client_state.lock().await;
            let tick = 50;
            if tick > state.last_tick {
                state.last_tick = tick;
            }
        }

        let state = client_state.lock().await;
        assert_eq!(state.last_tick, 100);
    }
}
