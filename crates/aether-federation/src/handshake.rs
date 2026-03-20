//! Federation handshake protocol for mutual authentication between servers.

use std::collections::HashMap;

/// State of a handshake negotiation.
#[derive(Debug, Clone, PartialEq)]
pub enum HandshakeState {
    /// Initiator has sent a challenge, awaiting response.
    Initiated,
    /// Responder has received the challenge and sent a response with a counter-challenge.
    ChallengeReceived,
    /// Both sides have verified; handshake is complete.
    Completed,
    /// Handshake failed due to timeout, invalid nonce, or other error.
    Failed(String),
}

/// A challenge sent from one server to another to begin a handshake.
#[derive(Debug, Clone)]
pub struct HandshakeChallenge {
    /// ID of the server sending the challenge.
    pub server_id: String,
    /// Random nonce for the challenge.
    pub nonce: String,
    /// Timestamp of challenge creation in milliseconds.
    pub timestamp_ms: u64,
}

/// Response to a challenge, including the signed nonce and a counter-challenge.
#[derive(Debug, Clone)]
pub struct HandshakeResponse {
    /// ID of the responding server.
    pub server_id: String,
    /// The original nonce, signed by the responder (simulated as prefixed string).
    pub nonce_signed: String,
    /// Counter-challenge nonce for mutual authentication.
    pub challenge_back_nonce: String,
}

/// Final message completing the handshake.
#[derive(Debug, Clone)]
pub struct HandshakeComplete {
    /// The counter-challenge nonce, signed by the initiator.
    pub nonce_signed_back: String,
}

/// Tracks a single in-progress or completed handshake session.
#[derive(Debug, Clone)]
pub struct HandshakeSession {
    pub initiator_id: String,
    pub responder_id: String,
    pub state: HandshakeState,
    pub initiated_at_ms: u64,
    pub original_nonce: String,
    pub counter_nonce: Option<String>,
}

const HANDSHAKE_TIMEOUT_MS: u64 = 30_000;

/// Manages handshake sessions between federated servers.
#[derive(Debug)]
pub struct HandshakeManager {
    /// Key is "{initiator_id}:{responder_id}".
    sessions: HashMap<String, HandshakeSession>,
    timeout_ms: u64,
}

impl HandshakeManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            timeout_ms: HANDSHAKE_TIMEOUT_MS,
        }
    }

    pub fn with_timeout(timeout_ms: u64) -> Self {
        Self {
            sessions: HashMap::new(),
            timeout_ms,
        }
    }

    fn session_key(initiator: &str, responder: &str) -> String {
        format!("{}:{}", initiator, responder)
    }

    /// Initiate a handshake by creating a challenge.
    /// Returns the challenge to send to the responder.
    pub fn initiate(
        &mut self,
        challenge: HandshakeChallenge,
        responder_id: &str,
    ) -> Result<(), HandshakeError> {
        let key = Self::session_key(&challenge.server_id, responder_id);
        if let Some(existing) = self.sessions.get(&key) {
            if existing.state == HandshakeState::Initiated
                || existing.state == HandshakeState::ChallengeReceived
            {
                return Err(HandshakeError::AlreadyInProgress);
            }
        }
        let session = HandshakeSession {
            initiator_id: challenge.server_id.clone(),
            responder_id: responder_id.to_string(),
            state: HandshakeState::Initiated,
            initiated_at_ms: challenge.timestamp_ms,
            original_nonce: challenge.nonce.clone(),
            counter_nonce: None,
        };
        self.sessions.insert(key, session);
        Ok(())
    }

    /// Process a response from the responder.
    /// Verifies the signed nonce and records the counter-challenge.
    pub fn process_response(
        &mut self,
        initiator_id: &str,
        response: HandshakeResponse,
        now_ms: u64,
    ) -> Result<(), HandshakeError> {
        let key = Self::session_key(initiator_id, &response.server_id);
        let session = self
            .sessions
            .get_mut(&key)
            .ok_or(HandshakeError::SessionNotFound)?;

        if session.state != HandshakeState::Initiated {
            return Err(HandshakeError::InvalidState);
        }

        if now_ms.saturating_sub(session.initiated_at_ms) > self.timeout_ms {
            session.state = HandshakeState::Failed("timeout".to_string());
            return Err(HandshakeError::Timeout);
        }

        // Verify the signed nonce: in this simulation, the signed nonce must be
        // "signed:{original_nonce}".
        let expected_signed = format!("signed:{}", session.original_nonce);
        if response.nonce_signed != expected_signed {
            session.state = HandshakeState::Failed("invalid_nonce_signature".to_string());
            return Err(HandshakeError::InvalidNonce);
        }

        session.counter_nonce = Some(response.challenge_back_nonce.clone());
        session.state = HandshakeState::ChallengeReceived;
        Ok(())
    }

    /// Complete the handshake by verifying the initiator's signed counter-nonce.
    pub fn complete(
        &mut self,
        initiator_id: &str,
        responder_id: &str,
        completion: HandshakeComplete,
        now_ms: u64,
    ) -> Result<(), HandshakeError> {
        let key = Self::session_key(initiator_id, responder_id);
        let session = self
            .sessions
            .get_mut(&key)
            .ok_or(HandshakeError::SessionNotFound)?;

        if session.state != HandshakeState::ChallengeReceived {
            return Err(HandshakeError::InvalidState);
        }

        if now_ms.saturating_sub(session.initiated_at_ms) > self.timeout_ms {
            session.state = HandshakeState::Failed("timeout".to_string());
            return Err(HandshakeError::Timeout);
        }

        let counter_nonce = session
            .counter_nonce
            .as_ref()
            .ok_or(HandshakeError::InvalidState)?;

        let expected_signed_back = format!("signed:{}", counter_nonce);
        if completion.nonce_signed_back != expected_signed_back {
            session.state = HandshakeState::Failed("invalid_counter_signature".to_string());
            return Err(HandshakeError::InvalidNonce);
        }

        session.state = HandshakeState::Completed;
        Ok(())
    }

    /// Get the current state of a handshake session.
    pub fn get_session(&self, initiator_id: &str, responder_id: &str) -> Option<&HandshakeSession> {
        let key = Self::session_key(initiator_id, responder_id);
        self.sessions.get(&key)
    }

    /// Check if a handshake between two servers is completed.
    pub fn is_authenticated(&self, initiator_id: &str, responder_id: &str) -> bool {
        self.get_session(initiator_id, responder_id)
            .is_some_and(|s| s.state == HandshakeState::Completed)
    }

    /// Remove expired or failed sessions.
    pub fn purge_expired(&mut self, now_ms: u64) {
        self.sessions.retain(|_, session| {
            let expired = now_ms.saturating_sub(session.initiated_at_ms) > self.timeout_ms;
            let failed = matches!(session.state, HandshakeState::Failed(_));
            // Keep if not expired and not failed, or if completed.
            session.state == HandshakeState::Completed || (!expired && !failed)
        });
    }
}

#[allow(clippy::derivable_impls)]
impl Default for HandshakeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during a handshake.
#[derive(Debug, Clone, PartialEq)]
pub enum HandshakeError {
    AlreadyInProgress,
    SessionNotFound,
    InvalidState,
    InvalidNonce,
    Timeout,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn challenge(server_id: &str, nonce: &str, ts: u64) -> HandshakeChallenge {
        HandshakeChallenge {
            server_id: server_id.to_string(),
            nonce: nonce.to_string(),
            timestamp_ms: ts,
        }
    }

    fn response(server_id: &str, original_nonce: &str, counter_nonce: &str) -> HandshakeResponse {
        HandshakeResponse {
            server_id: server_id.to_string(),
            nonce_signed: format!("signed:{}", original_nonce),
            challenge_back_nonce: counter_nonce.to_string(),
        }
    }

    fn completion(counter_nonce: &str) -> HandshakeComplete {
        HandshakeComplete {
            nonce_signed_back: format!("signed:{}", counter_nonce),
        }
    }

    #[test]
    fn full_handshake_lifecycle() {
        let mut mgr = HandshakeManager::new();

        // Initiate
        mgr.initiate(challenge("A", "nonce_a", 1000), "B").unwrap();
        assert_eq!(
            mgr.get_session("A", "B").unwrap().state,
            HandshakeState::Initiated
        );

        // Process response
        mgr.process_response("A", response("B", "nonce_a", "nonce_b"), 1500)
            .unwrap();
        assert_eq!(
            mgr.get_session("A", "B").unwrap().state,
            HandshakeState::ChallengeReceived
        );

        // Complete
        mgr.complete("A", "B", completion("nonce_b"), 2000).unwrap();
        assert_eq!(
            mgr.get_session("A", "B").unwrap().state,
            HandshakeState::Completed
        );
        assert!(mgr.is_authenticated("A", "B"));
    }

    #[test]
    fn duplicate_initiation_is_error() {
        let mut mgr = HandshakeManager::new();
        mgr.initiate(challenge("A", "n1", 1000), "B").unwrap();
        assert_eq!(
            mgr.initiate(challenge("A", "n2", 1100), "B"),
            Err(HandshakeError::AlreadyInProgress)
        );
    }

    #[test]
    fn response_without_initiation_is_error() {
        let mut mgr = HandshakeManager::new();
        assert_eq!(
            mgr.process_response("A", response("B", "n1", "n2"), 1000),
            Err(HandshakeError::SessionNotFound)
        );
    }

    #[test]
    fn invalid_nonce_signature_fails() {
        let mut mgr = HandshakeManager::new();
        mgr.initiate(challenge("A", "nonce_a", 1000), "B").unwrap();

        let bad_response = HandshakeResponse {
            server_id: "B".to_string(),
            nonce_signed: "wrong_signature".to_string(),
            challenge_back_nonce: "nonce_b".to_string(),
        };
        assert_eq!(
            mgr.process_response("A", bad_response, 1500),
            Err(HandshakeError::InvalidNonce)
        );
        assert!(matches!(
            mgr.get_session("A", "B").unwrap().state,
            HandshakeState::Failed(_)
        ));
    }

    #[test]
    fn timeout_on_response() {
        let mut mgr = HandshakeManager::with_timeout(5000);
        mgr.initiate(challenge("A", "n1", 1000), "B").unwrap();
        // Respond well after timeout
        assert_eq!(
            mgr.process_response("A", response("B", "n1", "n2"), 100_000),
            Err(HandshakeError::Timeout)
        );
    }

    #[test]
    fn timeout_on_completion() {
        let mut mgr = HandshakeManager::with_timeout(5000);
        mgr.initiate(challenge("A", "n1", 1000), "B").unwrap();
        mgr.process_response("A", response("B", "n1", "n2"), 2000)
            .unwrap();
        // Complete well after timeout
        assert_eq!(
            mgr.complete("A", "B", completion("n2"), 100_000),
            Err(HandshakeError::Timeout)
        );
    }

    #[test]
    fn complete_with_wrong_counter_nonce_fails() {
        let mut mgr = HandshakeManager::new();
        mgr.initiate(challenge("A", "n1", 1000), "B").unwrap();
        mgr.process_response("A", response("B", "n1", "n2"), 1500)
            .unwrap();
        let bad_completion = HandshakeComplete {
            nonce_signed_back: "signed:wrong_nonce".to_string(),
        };
        assert_eq!(
            mgr.complete("A", "B", bad_completion, 2000),
            Err(HandshakeError::InvalidNonce)
        );
    }

    #[test]
    fn complete_without_response_is_invalid_state() {
        let mut mgr = HandshakeManager::new();
        mgr.initiate(challenge("A", "n1", 1000), "B").unwrap();
        assert_eq!(
            mgr.complete("A", "B", completion("n2"), 1500),
            Err(HandshakeError::InvalidState)
        );
    }

    #[test]
    fn is_authenticated_false_before_completion() {
        let mut mgr = HandshakeManager::new();
        assert!(!mgr.is_authenticated("A", "B"));
        mgr.initiate(challenge("A", "n1", 1000), "B").unwrap();
        assert!(!mgr.is_authenticated("A", "B"));
    }

    #[test]
    fn purge_removes_expired_and_failed() {
        let mut mgr = HandshakeManager::with_timeout(5000);

        // Expired session (initiated long ago, still in Initiated state)
        mgr.initiate(challenge("A", "n1", 1000), "B").unwrap();

        // Completed session (should survive purge)
        mgr.initiate(challenge("C", "n3", 50_000), "D").unwrap();
        mgr.process_response("C", response("D", "n3", "n4"), 51_000)
            .unwrap();
        mgr.complete("C", "D", completion("n4"), 52_000).unwrap();

        mgr.purge_expired(100_000);

        // Expired session should be removed
        assert!(mgr.get_session("A", "B").is_none());
        // Completed session should remain
        assert!(mgr.is_authenticated("C", "D"));
    }

    #[test]
    fn can_reinitiate_after_failed() {
        let mut mgr = HandshakeManager::new();
        mgr.initiate(challenge("A", "n1", 1000), "B").unwrap();

        // Fail it
        let bad_response = HandshakeResponse {
            server_id: "B".to_string(),
            nonce_signed: "bad".to_string(),
            challenge_back_nonce: "n2".to_string(),
        };
        let _ = mgr.process_response("A", bad_response, 1500);

        // Should be able to reinitiate since the previous one is failed
        mgr.initiate(challenge("A", "n_new", 2000), "B").unwrap();
        assert_eq!(
            mgr.get_session("A", "B").unwrap().state,
            HandshakeState::Initiated
        );
    }

    #[test]
    fn separate_sessions_for_different_pairs() {
        let mut mgr = HandshakeManager::new();
        mgr.initiate(challenge("A", "n1", 1000), "B").unwrap();
        mgr.initiate(challenge("C", "n2", 1000), "D").unwrap();
        assert!(mgr.get_session("A", "B").is_some());
        assert!(mgr.get_session("C", "D").is_some());
        assert!(mgr.get_session("A", "D").is_none());
    }
}
