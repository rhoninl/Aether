//! Multiplayer server: integrates QUIC transport with tick loop, sessions,
//! input collection, simulation, and state broadcast.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{broadcast, mpsc, Mutex, Notify};
use uuid::Uuid;

use aether_network::quic::config::QuicConfig;
use aether_network::quic::server::QuicServer;
use aether_world_runtime::session::{SessionEvent, SessionManager};
use aether_world_runtime::state_sync::StateSyncManager;
use aether_world_runtime::tick::TickScheduler;

use crate::avatar::AvatarState;
use crate::config::MultiplayerConfig;
use crate::protocol::{encode_server_message, PlayerId, ServerMessage};
use crate::simulation::WorldState;

/// Internal message from per-client receive tasks to the main tick loop.
#[derive(Debug, Clone)]
pub struct IncomingInput {
    pub player_id: PlayerId,
    pub tick: u64,
    pub avatar: AvatarState,
}

/// Maps QUIC connection IDs to player UUIDs.
type ConnectionMap = Arc<Mutex<HashMap<u64, PlayerId>>>;

/// Shared server state accessible by both accept and tick loops.
pub struct SharedState {
    pub session_manager: Arc<Mutex<SessionManager>>,
    pub state_sync: Arc<Mutex<StateSyncManager>>,
    pub world_state: Arc<Mutex<WorldState>>,
    pub connection_map: ConnectionMap,
    pub quic: Arc<QuicServer>,
    pub event_tx: broadcast::Sender<ServerMessage>,
}

/// The multiplayer server orchestrator.
pub struct MultiplayerServer {
    config: MultiplayerConfig,
    shutdown: Arc<Notify>,
}

impl MultiplayerServer {
    pub fn new(config: MultiplayerConfig) -> Self {
        Self {
            config,
            shutdown: Arc::new(Notify::new()),
        }
    }

    /// Get a handle to signal server shutdown.
    pub fn shutdown_handle(&self) -> Arc<Notify> {
        Arc::clone(&self.shutdown)
    }

    /// Run the server until shutdown is signaled.
    pub async fn run(&self) -> Result<(), ServerError> {
        let quic_config = QuicConfig {
            bind_addr: self.config.bind_addr,
            ..QuicConfig::default()
        };

        let quic_server = Arc::new(
            QuicServer::bind(&quic_config).map_err(|e| ServerError::Bind(e.to_string()))?,
        );

        let local_addr = quic_server
            .local_addr()
            .map_err(|e| ServerError::Bind(e.to_string()))?;

        tracing::info!(addr = %local_addr, "multiplayer server started");

        let (event_tx, _) = broadcast::channel::<ServerMessage>(256);

        let shared = Arc::new(SharedState {
            session_manager: Arc::new(Mutex::new(SessionManager::with_config(
                30_000,
                self.config.max_players,
            ))),
            state_sync: Arc::new(Mutex::new(StateSyncManager::new())),
            world_state: Arc::new(Mutex::new(WorldState::new())),
            connection_map: Arc::new(Mutex::new(HashMap::new())),
            quic: quic_server,
            event_tx,
        });

        let (input_tx, input_rx) = mpsc::channel::<IncomingInput>(1024);

        // Spawn accept loop
        let accept_handle = {
            let shared = Arc::clone(&shared);
            let input_tx = input_tx.clone();
            let shutdown = Arc::clone(&self.shutdown);

            tokio::spawn(async move {
                accept_loop(shared, input_tx, shutdown).await;
            })
        };

        // Spawn tick loop
        let tick_handle = {
            let shared = Arc::clone(&shared);
            let shutdown = Arc::clone(&self.shutdown);
            let tick_rate = self.config.tick_rate;

            tokio::spawn(async move {
                tick_loop(tick_rate, shared, input_rx, shutdown).await;
            })
        };

        tokio::select! {
            _ = accept_handle => {
                tracing::info!("accept loop ended");
            }
            _ = tick_handle => {
                tracing::info!("tick loop ended");
            }
        }

        shared.quic.shutdown().await;
        tracing::info!("multiplayer server shut down");
        Ok(())
    }
}

/// Accept loop: handles new QUIC client connections.
async fn accept_loop(
    shared: Arc<SharedState>,
    _input_tx: mpsc::Sender<IncomingInput>,
    shutdown: Arc<Notify>,
) {
    loop {
        tokio::select! {
            _ = shutdown.notified() => {
                tracing::info!("accept loop shutting down");
                return;
            }
            result = shared.quic.accept() => {
                match result {
                    Ok((client_id, _token)) => {
                        handle_new_connection(&shared, client_id).await;
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, "accept failed");
                        return;
                    }
                }
            }
        }
    }
}

/// Handle a single new client connection.
async fn handle_new_connection(shared: &SharedState, client_id: u64) {
    let player_id = Uuid::new_v4();
    let now_ms = current_time_ms();

    // Register session
    {
        let mut sessions = shared.session_manager.lock().await;
        match sessions.join(player_id, now_ms) {
            Ok(SessionEvent::PlayerJoined { .. }) | Ok(SessionEvent::PlayerReconnecting { .. }) => {
                if let Err(e) = sessions.activate(&player_id) {
                    tracing::error!(
                        player_id = %player_id,
                        error = ?e,
                        "failed to activate session"
                    );
                    shared.quic.disconnect(client_id, "activation failed").await;
                    return;
                }
            }
            Err(e) => {
                tracing::warn!(player_id = %player_id, error = ?e, "session join rejected");
                shared.quic.disconnect(client_id, "session rejected").await;
                return;
            }
            _ => {}
        }
    }

    // Register in state sync and world
    shared.state_sync.lock().await.add_client(player_id);
    shared.world_state.lock().await.add_player(player_id);
    shared.connection_map.lock().await.insert(client_id, player_id);

    tracing::info!(client_id = client_id, player_id = %player_id, "player connected");

    // Send full sync to new player
    let avatars = shared.world_state.lock().await.all_avatars();
    let full_sync = ServerMessage::FullSync { tick: 0, avatars };
    if let Ok(data) = encode_server_message(&full_sync) {
        if let Err(e) = shared.quic.send_reliable(client_id, &data).await {
            tracing::warn!(client_id = client_id, error = %e, "failed to send full sync");
        }
    }

    // Broadcast join event
    let _ = shared
        .event_tx
        .send(ServerMessage::PlayerJoined { player_id });
}

/// Main server tick loop.
async fn tick_loop(
    tick_rate: u32,
    shared: Arc<SharedState>,
    mut input_rx: mpsc::Receiver<IncomingInput>,
    shutdown: Arc<Notify>,
) {
    let mut scheduler = TickScheduler::new(tick_rate);
    let mut last_instant = Instant::now();

    loop {
        let tick_interval = std::time::Duration::from_micros(scheduler.tick_interval_us());

        tokio::select! {
            _ = shutdown.notified() => {
                tracing::info!("tick loop shutting down");
                return;
            }
            _ = tokio::time::sleep(tick_interval) => {
                let now = Instant::now();
                let elapsed_us = now.duration_since(last_instant).as_micros() as u64;
                last_instant = now;

                let ticks = scheduler.update(elapsed_us);

                // Drain incoming inputs
                let mut pending_inputs = Vec::new();
                while let Ok(input) = input_rx.try_recv() {
                    pending_inputs.push(input);
                }

                for tick in &ticks {
                    process_tick(tick.tick_number, &shared, &pending_inputs).await;
                }

                // Update server tick on QUIC server
                if let Some(last_tick) = ticks.last() {
                    shared.quic.set_server_tick(last_tick.tick_number).await;
                }
            }
        }
    }
}

/// Process a single server tick.
async fn process_tick(
    tick_number: u64,
    shared: &SharedState,
    pending_inputs: &[IncomingInput],
) {
    // Apply pending inputs
    {
        let mut world = shared.world_state.lock().await;
        for input in pending_inputs {
            world.apply_input(&input.player_id, input.avatar.clone());
        }
    }

    // Update state sync with current avatar positions
    {
        let world = shared.world_state.lock().await;
        let mut sync = shared.state_sync.lock().await;
        for (player_id, avatar) in world.all_avatars() {
            let entity_id = player_id_to_entity_id(&player_id);
            let entity_state = avatar.to_entity_state(entity_id, tick_number);
            sync.update_entity(entity_state);
        }
    }

    // Broadcast world state
    let avatars = shared.world_state.lock().await.all_avatars();
    let world_state_msg = ServerMessage::WorldState {
        tick: tick_number,
        avatars,
    };

    if let Ok(data) = encode_server_message(&world_state_msg) {
        let conn_map = shared.connection_map.lock().await;
        for (&client_id, _) in conn_map.iter() {
            if let Err(e) = shared.quic.send_datagram(client_id, &data).await {
                tracing::debug!(client_id = client_id, error = %e, "failed to send world state");
            }
        }
    }

    // Sweep session timeouts
    let now_ms = current_time_ms();
    let timeouts = shared.session_manager.lock().await.sweep_timeouts(now_ms);
    for event in timeouts {
        if let SessionEvent::PlayerTimedOut { player_id } = event {
            tracing::info!(player_id = %player_id, "player timed out");
            let _ = shared
                .event_tx
                .send(ServerMessage::PlayerLeft { player_id });
            shared.state_sync.lock().await.remove_client(&player_id);
            shared.world_state.lock().await.remove_player(&player_id);

            // Remove from connection map
            let mut conn_map = shared.connection_map.lock().await;
            conn_map.retain(|_, pid| *pid != player_id);
        }
    }
}

/// Convert a PlayerId UUID to a deterministic u64 entity ID.
pub fn player_id_to_entity_id(player_id: &PlayerId) -> u64 {
    let bytes = player_id.as_bytes();
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Errors from server operations.
#[derive(Debug)]
pub enum ServerError {
    Bind(String),
    Runtime(String),
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerError::Bind(msg) => write!(f, "server bind error: {msg}"),
            ServerError::Runtime(msg) => write!(f, "server runtime error: {msg}"),
        }
    }
}

impl std::error::Error for ServerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_id_to_entity_id_is_deterministic() {
        let pid = Uuid::new_v4();
        let eid1 = player_id_to_entity_id(&pid);
        let eid2 = player_id_to_entity_id(&pid);
        assert_eq!(eid1, eid2);
    }

    #[test]
    fn different_players_get_different_entity_ids() {
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        let e1 = player_id_to_entity_id(&p1);
        let e2 = player_id_to_entity_id(&p2);
        assert_ne!(e1, e2);
    }

    #[test]
    fn server_error_displays() {
        let err = ServerError::Bind("port in use".to_string());
        assert!(format!("{err}").contains("port in use"));
        let err = ServerError::Runtime("tick failed".to_string());
        assert!(format!("{err}").contains("tick failed"));
    }

    #[test]
    fn current_time_ms_is_nonzero() {
        assert!(current_time_ms() > 0);
    }

    #[test]
    fn multiplayer_server_creates() {
        let config = MultiplayerConfig::default();
        let server = MultiplayerServer::new(config);
        let _handle = server.shutdown_handle();
    }

    #[test]
    fn incoming_input_clone() {
        let input = IncomingInput {
            player_id: Uuid::new_v4(),
            tick: 42,
            avatar: AvatarState::default(),
        };
        let cloned = input.clone();
        assert_eq!(input.player_id, cloned.player_id);
        assert_eq!(input.tick, cloned.tick);
    }
}
