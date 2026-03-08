//! Player session lifecycle management.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::input_buffer::PlayerId;

/// Default reconnect window in milliseconds (30 seconds).
const DEFAULT_RECONNECT_WINDOW_MS: u64 = 30_000;

/// Default maximum concurrent sessions.
const DEFAULT_MAX_SESSIONS: usize = 256;

/// The lifecycle state of a player session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// Player is establishing connection.
    Connecting,
    /// Player is fully connected and active.
    Active,
    /// Player disconnected; may reconnect within the window.
    Disconnected { since_ms: u64 },
    /// Player is attempting to reconnect.
    Reconnecting,
}

/// A player's session data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSession {
    pub player_id: PlayerId,
    pub connection_id: u64,
    pub joined_at_ms: u64,
    pub last_input_tick: u64,
    pub state: SessionState,
}

/// Events emitted by the session manager.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionEvent {
    PlayerJoined { player_id: PlayerId },
    PlayerActive { player_id: PlayerId },
    PlayerDisconnected { player_id: PlayerId },
    PlayerReconnecting { player_id: PlayerId },
    PlayerReconnected { player_id: PlayerId },
    PlayerTimedOut { player_id: PlayerId },
    SessionFull { player_id: PlayerId },
}

/// Errors from session operations.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionError {
    /// Session already exists for this player.
    AlreadyExists { player_id: PlayerId },
    /// No session found for this player.
    NotFound { player_id: PlayerId },
    /// Server is at max capacity.
    ServerFull,
    /// Invalid state transition.
    InvalidTransition {
        player_id: PlayerId,
        from: SessionState,
        to: SessionState,
    },
}

/// Manages player sessions including join, disconnect, and reconnect.
#[derive(Debug)]
pub struct SessionManager {
    sessions: HashMap<PlayerId, PlayerSession>,
    reconnect_window_ms: u64,
    max_sessions: usize,
    next_connection_id: u64,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            reconnect_window_ms: DEFAULT_RECONNECT_WINDOW_MS,
            max_sessions: DEFAULT_MAX_SESSIONS,
            next_connection_id: 1,
        }
    }

    pub fn with_config(reconnect_window_ms: u64, max_sessions: usize) -> Self {
        Self {
            sessions: HashMap::new(),
            reconnect_window_ms,
            max_sessions: max_sessions.max(1),
            next_connection_id: 1,
        }
    }

    /// Attempt to add a new player session.
    pub fn join(
        &mut self,
        player_id: PlayerId,
        now_ms: u64,
    ) -> Result<SessionEvent, SessionError> {
        // Check for existing session
        if let Some(existing) = self.sessions.get(&player_id) {
            match &existing.state {
                SessionState::Disconnected { .. } => {
                    // Allow reconnect
                    return self.begin_reconnect(player_id);
                }
                _ => {
                    return Err(SessionError::AlreadyExists { player_id });
                }
            }
        }

        // Check capacity
        let active_count = self
            .sessions
            .values()
            .filter(|s| !matches!(s.state, SessionState::Disconnected { .. }))
            .count();

        if active_count >= self.max_sessions {
            return Err(SessionError::ServerFull);
        }

        let connection_id = self.next_connection_id;
        self.next_connection_id += 1;

        let session = PlayerSession {
            player_id,
            connection_id,
            joined_at_ms: now_ms,
            last_input_tick: 0,
            state: SessionState::Connecting,
        };

        self.sessions.insert(player_id, session);
        Ok(SessionEvent::PlayerJoined { player_id })
    }

    /// Transition a connecting player to active.
    pub fn activate(&mut self, player_id: &PlayerId) -> Result<SessionEvent, SessionError> {
        let session = self
            .sessions
            .get_mut(player_id)
            .ok_or(SessionError::NotFound {
                player_id: *player_id,
            })?;

        match &session.state {
            SessionState::Connecting | SessionState::Reconnecting => {
                session.state = SessionState::Active;
                Ok(SessionEvent::PlayerActive {
                    player_id: *player_id,
                })
            }
            other => Err(SessionError::InvalidTransition {
                player_id: *player_id,
                from: other.clone(),
                to: SessionState::Active,
            }),
        }
    }

    /// Mark a player as disconnected.
    pub fn disconnect(
        &mut self,
        player_id: &PlayerId,
        now_ms: u64,
    ) -> Result<SessionEvent, SessionError> {
        let session = self
            .sessions
            .get_mut(player_id)
            .ok_or(SessionError::NotFound {
                player_id: *player_id,
            })?;

        match &session.state {
            SessionState::Active | SessionState::Connecting | SessionState::Reconnecting => {
                session.state = SessionState::Disconnected { since_ms: now_ms };
                Ok(SessionEvent::PlayerDisconnected {
                    player_id: *player_id,
                })
            }
            other => Err(SessionError::InvalidTransition {
                player_id: *player_id,
                from: other.clone(),
                to: SessionState::Disconnected { since_ms: now_ms },
            }),
        }
    }

    /// Begin reconnection for a disconnected player.
    fn begin_reconnect(
        &mut self,
        player_id: PlayerId,
    ) -> Result<SessionEvent, SessionError> {
        let session = self
            .sessions
            .get_mut(&player_id)
            .ok_or(SessionError::NotFound { player_id })?;

        match &session.state {
            SessionState::Disconnected { .. } => {
                session.state = SessionState::Reconnecting;
                let connection_id = self.next_connection_id;
                self.next_connection_id += 1;
                session.connection_id = connection_id;
                Ok(SessionEvent::PlayerReconnecting { player_id })
            }
            other => Err(SessionError::InvalidTransition {
                player_id,
                from: other.clone(),
                to: SessionState::Reconnecting,
            }),
        }
    }

    /// Update the last input tick for a player.
    pub fn update_input_tick(
        &mut self,
        player_id: &PlayerId,
        tick: u64,
    ) -> Result<(), SessionError> {
        let session = self
            .sessions
            .get_mut(player_id)
            .ok_or(SessionError::NotFound {
                player_id: *player_id,
            })?;
        session.last_input_tick = tick;
        Ok(())
    }

    /// Check for timed-out disconnected sessions and remove them.
    /// Returns events for each timed-out player.
    pub fn sweep_timeouts(&mut self, now_ms: u64) -> Vec<SessionEvent> {
        let mut timed_out = Vec::new();

        for (player_id, session) in &self.sessions {
            if let SessionState::Disconnected { since_ms } = &session.state {
                if now_ms.saturating_sub(*since_ms) > self.reconnect_window_ms {
                    timed_out.push(*player_id);
                }
            }
        }

        let mut events = Vec::new();
        for player_id in timed_out {
            self.sessions.remove(&player_id);
            events.push(SessionEvent::PlayerTimedOut { player_id });
        }
        events
    }

    /// Get a player's session.
    pub fn get_session(&self, player_id: &PlayerId) -> Option<&PlayerSession> {
        self.sessions.get(player_id)
    }

    /// Get all active player IDs.
    pub fn active_players(&self) -> Vec<PlayerId> {
        self.sessions
            .iter()
            .filter(|(_, s)| matches!(s.state, SessionState::Active))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get total session count (including disconnected).
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Get count of active sessions only.
    pub fn active_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|s| matches!(s.state, SessionState::Active))
            .count()
    }

    /// Remove a session entirely (force kick).
    pub fn remove(&mut self, player_id: &PlayerId) -> bool {
        self.sessions.remove(player_id).is_some()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_join_creates_connecting_session() {
        let mut mgr = SessionManager::new();
        let pid = Uuid::new_v4();

        let event = mgr.join(pid, 1000).unwrap();
        assert_eq!(event, SessionEvent::PlayerJoined { player_id: pid });

        let session = mgr.get_session(&pid).unwrap();
        assert_eq!(session.state, SessionState::Connecting);
        assert_eq!(session.joined_at_ms, 1000);
    }

    #[test]
    fn test_activate_from_connecting() {
        let mut mgr = SessionManager::new();
        let pid = Uuid::new_v4();
        mgr.join(pid, 1000).unwrap();

        let event = mgr.activate(&pid).unwrap();
        assert_eq!(event, SessionEvent::PlayerActive { player_id: pid });

        let session = mgr.get_session(&pid).unwrap();
        assert_eq!(session.state, SessionState::Active);
    }

    #[test]
    fn test_disconnect_from_active() {
        let mut mgr = SessionManager::new();
        let pid = Uuid::new_v4();
        mgr.join(pid, 1000).unwrap();
        mgr.activate(&pid).unwrap();

        let event = mgr.disconnect(&pid, 5000).unwrap();
        assert_eq!(event, SessionEvent::PlayerDisconnected { player_id: pid });

        let session = mgr.get_session(&pid).unwrap();
        assert_eq!(session.state, SessionState::Disconnected { since_ms: 5000 });
    }

    #[test]
    fn test_reconnect_from_disconnected() {
        let mut mgr = SessionManager::new();
        let pid = Uuid::new_v4();
        mgr.join(pid, 1000).unwrap();
        mgr.activate(&pid).unwrap();
        mgr.disconnect(&pid, 5000).unwrap();

        // Join again triggers reconnect
        let event = mgr.join(pid, 6000).unwrap();
        assert_eq!(event, SessionEvent::PlayerReconnecting { player_id: pid });

        let session = mgr.get_session(&pid).unwrap();
        assert_eq!(session.state, SessionState::Reconnecting);

        // Activate completes the reconnect
        let event = mgr.activate(&pid).unwrap();
        assert_eq!(event, SessionEvent::PlayerActive { player_id: pid });
    }

    #[test]
    fn test_reconnect_gets_new_connection_id() {
        let mut mgr = SessionManager::new();
        let pid = Uuid::new_v4();
        mgr.join(pid, 1000).unwrap();
        let original_conn_id = mgr.get_session(&pid).unwrap().connection_id;

        mgr.activate(&pid).unwrap();
        mgr.disconnect(&pid, 5000).unwrap();
        mgr.join(pid, 6000).unwrap();

        let new_conn_id = mgr.get_session(&pid).unwrap().connection_id;
        assert_ne!(original_conn_id, new_conn_id);
    }

    #[test]
    fn test_duplicate_join_rejected() {
        let mut mgr = SessionManager::new();
        let pid = Uuid::new_v4();
        mgr.join(pid, 1000).unwrap();

        let result = mgr.join(pid, 2000);
        assert_eq!(result.unwrap_err(), SessionError::AlreadyExists { player_id: pid });
    }

    #[test]
    fn test_server_full() {
        let mut mgr = SessionManager::with_config(30_000, 2);
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        let p3 = Uuid::new_v4();

        mgr.join(p1, 1000).unwrap();
        mgr.join(p2, 1000).unwrap();

        let result = mgr.join(p3, 1000);
        assert_eq!(result.unwrap_err(), SessionError::ServerFull);
    }

    #[test]
    fn test_server_full_disconnected_dont_count() {
        let mut mgr = SessionManager::with_config(30_000, 2);
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        let p3 = Uuid::new_v4();

        mgr.join(p1, 1000).unwrap();
        mgr.activate(&p1).unwrap();
        mgr.join(p2, 1000).unwrap();
        mgr.activate(&p2).unwrap();

        // Disconnect p1 frees a slot
        mgr.disconnect(&p1, 2000).unwrap();

        // p3 can now join
        let result = mgr.join(p3, 3000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sweep_timeouts() {
        let mut mgr = SessionManager::with_config(5_000, 256);
        let pid = Uuid::new_v4();

        mgr.join(pid, 1000).unwrap();
        mgr.activate(&pid).unwrap();
        mgr.disconnect(&pid, 2000).unwrap();

        // Before timeout expires
        let events = mgr.sweep_timeouts(6000);
        assert!(events.is_empty());
        assert!(mgr.get_session(&pid).is_some());

        // After timeout expires (2000 + 5000 = 7000, so 7001 should timeout)
        let events = mgr.sweep_timeouts(7001);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], SessionEvent::PlayerTimedOut { player_id: pid });
        assert!(mgr.get_session(&pid).is_none());
    }

    #[test]
    fn test_update_input_tick() {
        let mut mgr = SessionManager::new();
        let pid = Uuid::new_v4();
        mgr.join(pid, 1000).unwrap();
        mgr.activate(&pid).unwrap();

        mgr.update_input_tick(&pid, 42).unwrap();
        assert_eq!(mgr.get_session(&pid).unwrap().last_input_tick, 42);
    }

    #[test]
    fn test_update_input_tick_not_found() {
        let mut mgr = SessionManager::new();
        let pid = Uuid::new_v4();
        let result = mgr.update_input_tick(&pid, 42);
        assert_eq!(
            result.unwrap_err(),
            SessionError::NotFound { player_id: pid }
        );
    }

    #[test]
    fn test_active_players() {
        let mut mgr = SessionManager::new();
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        let p3 = Uuid::new_v4();

        mgr.join(p1, 1000).unwrap();
        mgr.activate(&p1).unwrap();
        mgr.join(p2, 1000).unwrap();
        mgr.activate(&p2).unwrap();
        mgr.join(p3, 1000).unwrap(); // still connecting

        let active = mgr.active_players();
        assert_eq!(active.len(), 2);
        assert!(active.contains(&p1));
        assert!(active.contains(&p2));
    }

    #[test]
    fn test_session_count_vs_active_count() {
        let mut mgr = SessionManager::new();
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();

        mgr.join(p1, 1000).unwrap();
        mgr.activate(&p1).unwrap();
        mgr.join(p2, 1000).unwrap();
        mgr.activate(&p2).unwrap();
        mgr.disconnect(&p2, 2000).unwrap();

        assert_eq!(mgr.session_count(), 2);
        assert_eq!(mgr.active_count(), 1);
    }

    #[test]
    fn test_remove_session() {
        let mut mgr = SessionManager::new();
        let pid = Uuid::new_v4();

        mgr.join(pid, 1000).unwrap();
        assert!(mgr.remove(&pid));
        assert!(mgr.get_session(&pid).is_none());
        assert!(!mgr.remove(&pid)); // already removed
    }

    #[test]
    fn test_invalid_activate_from_active() {
        let mut mgr = SessionManager::new();
        let pid = Uuid::new_v4();
        mgr.join(pid, 1000).unwrap();
        mgr.activate(&pid).unwrap();

        let result = mgr.activate(&pid);
        assert!(matches!(result.unwrap_err(), SessionError::InvalidTransition { .. }));
    }

    #[test]
    fn test_invalid_disconnect_from_disconnected() {
        let mut mgr = SessionManager::new();
        let pid = Uuid::new_v4();
        mgr.join(pid, 1000).unwrap();
        mgr.activate(&pid).unwrap();
        mgr.disconnect(&pid, 5000).unwrap();

        let result = mgr.disconnect(&pid, 6000);
        assert!(matches!(result.unwrap_err(), SessionError::InvalidTransition { .. }));
    }

    #[test]
    fn test_full_lifecycle() {
        let mut mgr = SessionManager::with_config(10_000, 100);
        let pid = Uuid::new_v4();

        // Join
        let e = mgr.join(pid, 100).unwrap();
        assert_eq!(e, SessionEvent::PlayerJoined { player_id: pid });

        // Activate
        let e = mgr.activate(&pid).unwrap();
        assert_eq!(e, SessionEvent::PlayerActive { player_id: pid });

        // Play for a while
        mgr.update_input_tick(&pid, 50).unwrap();
        assert_eq!(mgr.get_session(&pid).unwrap().last_input_tick, 50);

        // Disconnect
        let e = mgr.disconnect(&pid, 5000).unwrap();
        assert_eq!(e, SessionEvent::PlayerDisconnected { player_id: pid });

        // Reconnect before timeout
        let e = mgr.join(pid, 8000).unwrap();
        assert_eq!(e, SessionEvent::PlayerReconnecting { player_id: pid });

        let e = mgr.activate(&pid).unwrap();
        assert_eq!(e, SessionEvent::PlayerActive { player_id: pid });

        // Disconnect again
        mgr.disconnect(&pid, 12000).unwrap();

        // This time timeout
        let events = mgr.sweep_timeouts(23000);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], SessionEvent::PlayerTimedOut { player_id: pid });
    }
}
