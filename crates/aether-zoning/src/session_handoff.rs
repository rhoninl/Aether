//! Cross-world session handoff protocol types.
//!
//! Provides types for transferring a player session from one world server
//! to another, including session tokens, player state snapshots, and
//! the handoff envelope that wraps all transfer data.

use crate::aether_url::AetherUrl;

/// Default session token length in bytes.
const DEFAULT_SESSION_TOKEN_LENGTH: usize = 32;
/// Default session handoff timeout in milliseconds.
const DEFAULT_SESSION_HANDOFF_TIMEOUT_MS: u64 = 30_000;
/// Maximum player state snapshot size in bytes (1 MB).
const MAX_SNAPSHOT_SIZE: usize = 1_048_576;

/// An opaque session token authorizing a player on the target world server.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionToken {
    /// Raw token bytes.
    pub bytes: Vec<u8>,
    /// Timestamp when the token was issued (ms since epoch).
    pub issued_ms: u64,
    /// Timestamp when the token expires (ms since epoch).
    pub expires_ms: u64,
    /// Player ID the token is bound to.
    pub player_id: u64,
}

impl SessionToken {
    /// Create a new session token.
    pub fn new(bytes: Vec<u8>, player_id: u64, issued_ms: u64, ttl_ms: u64) -> Self {
        Self {
            bytes,
            issued_ms,
            expires_ms: issued_ms + ttl_ms,
            player_id,
        }
    }

    /// Check if the token has expired.
    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms >= self.expires_ms
    }

    /// Check if the token has the expected minimum length.
    pub fn is_valid_length(&self) -> bool {
        self.bytes.len() >= DEFAULT_SESSION_TOKEN_LENGTH
    }
}

/// Serialized player state for transfer between worlds.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerStateSnapshot {
    /// Player entity ID.
    pub player_id: u64,
    /// Serialized player state (inventory, avatar, etc.).
    pub data: Vec<u8>,
    /// Revision number for conflict detection.
    pub revision: u64,
    /// Timestamp when the snapshot was taken.
    pub captured_ms: u64,
}

/// Errors that can occur during session handoff validation.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionHandoffError {
    /// Session token has expired.
    TokenExpired,
    /// Session token is too short.
    TokenTooShort,
    /// Player ID mismatch between token and snapshot.
    PlayerIdMismatch { token_player: u64, snapshot_player: u64 },
    /// Snapshot exceeds maximum allowed size.
    SnapshotTooLarge { size: usize, max: usize },
    /// Handoff has timed out.
    Timeout,
    /// Source and destination are the same world.
    SameWorld,
}

impl std::fmt::Display for SessionHandoffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokenExpired => write!(f, "session token has expired"),
            Self::TokenTooShort => write!(f, "session token is too short"),
            Self::PlayerIdMismatch {
                token_player,
                snapshot_player,
            } => write!(
                f,
                "player ID mismatch: token={}, snapshot={}",
                token_player, snapshot_player
            ),
            Self::SnapshotTooLarge { size, max } => {
                write!(f, "snapshot too large: {} bytes (max {})", size, max)
            }
            Self::Timeout => write!(f, "session handoff timed out"),
            Self::SameWorld => write!(f, "source and destination are the same world"),
        }
    }
}

/// The session handoff envelope containing all data needed to transfer a player.
#[derive(Debug, Clone)]
pub struct SessionHandoffEnvelope {
    /// Unique handoff identifier.
    pub handoff_id: u128,
    /// Session token for the target server.
    pub token: SessionToken,
    /// Player state snapshot.
    pub snapshot: PlayerStateSnapshot,
    /// Source world URL.
    pub source: AetherUrl,
    /// Destination world URL.
    pub destination: AetherUrl,
    /// Sequence number for ordering.
    pub sequence: u64,
    /// Timestamp when the handoff was initiated.
    pub initiated_ms: u64,
    /// Timeout for the handoff.
    pub timeout_ms: u64,
}

impl SessionHandoffEnvelope {
    /// Create a new handoff envelope.
    pub fn new(
        handoff_id: u128,
        token: SessionToken,
        snapshot: PlayerStateSnapshot,
        source: AetherUrl,
        destination: AetherUrl,
        sequence: u64,
        initiated_ms: u64,
    ) -> Self {
        Self {
            handoff_id,
            token,
            snapshot,
            source,
            destination,
            sequence,
            initiated_ms,
            timeout_ms: DEFAULT_SESSION_HANDOFF_TIMEOUT_MS,
        }
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Validate the envelope for correctness.
    pub fn validate(&self, now_ms: u64) -> Result<(), SessionHandoffError> {
        // Check token expiry
        if self.token.is_expired(now_ms) {
            return Err(SessionHandoffError::TokenExpired);
        }

        // Check token length
        if !self.token.is_valid_length() {
            return Err(SessionHandoffError::TokenTooShort);
        }

        // Check player ID consistency
        if self.token.player_id != self.snapshot.player_id {
            return Err(SessionHandoffError::PlayerIdMismatch {
                token_player: self.token.player_id,
                snapshot_player: self.snapshot.player_id,
            });
        }

        // Check snapshot size
        if self.snapshot.data.len() > MAX_SNAPSHOT_SIZE {
            return Err(SessionHandoffError::SnapshotTooLarge {
                size: self.snapshot.data.len(),
                max: MAX_SNAPSHOT_SIZE,
            });
        }

        // Check timeout
        if now_ms.saturating_sub(self.initiated_ms) > self.timeout_ms {
            return Err(SessionHandoffError::Timeout);
        }

        // Check not same world
        if self.source.world_id == self.destination.world_id
            && self.source.host == self.destination.host
        {
            return Err(SessionHandoffError::SameWorld);
        }

        Ok(())
    }

    /// Check if the handoff has timed out.
    pub fn is_timed_out(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.initiated_ms) > self.timeout_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_token(player_id: u64, issued_ms: u64, ttl_ms: u64) -> SessionToken {
        SessionToken::new(vec![0u8; DEFAULT_SESSION_TOKEN_LENGTH], player_id, issued_ms, ttl_ms)
    }

    fn make_snapshot(player_id: u64) -> PlayerStateSnapshot {
        PlayerStateSnapshot {
            player_id,
            data: vec![1, 2, 3, 4],
            revision: 1,
            captured_ms: 100,
        }
    }

    fn make_source() -> AetherUrl {
        AetherUrl::parse("aether://host/source-world").unwrap()
    }

    fn make_dest() -> AetherUrl {
        AetherUrl::parse("aether://host/dest-world").unwrap()
    }

    fn make_envelope() -> SessionHandoffEnvelope {
        SessionHandoffEnvelope::new(
            1,
            make_token(42, 0, 60_000),
            make_snapshot(42),
            make_source(),
            make_dest(),
            1,
            0,
        )
    }

    // --- SessionToken ---

    #[test]
    fn token_not_expired() {
        let token = make_token(1, 0, 10_000);
        assert!(!token.is_expired(5_000));
    }

    #[test]
    fn token_expired() {
        let token = make_token(1, 0, 10_000);
        assert!(token.is_expired(10_000));
    }

    #[test]
    fn token_valid_length() {
        let token = make_token(1, 0, 10_000);
        assert!(token.is_valid_length());
    }

    #[test]
    fn token_too_short() {
        let token = SessionToken::new(vec![0u8; 10], 1, 0, 10_000);
        assert!(!token.is_valid_length());
    }

    #[test]
    fn token_exact_minimum_length() {
        let token = SessionToken::new(vec![0u8; DEFAULT_SESSION_TOKEN_LENGTH], 1, 0, 10_000);
        assert!(token.is_valid_length());
    }

    // --- Envelope validation: valid ---

    #[test]
    fn valid_envelope() {
        let envelope = make_envelope();
        assert!(envelope.validate(100).is_ok());
    }

    // --- Envelope validation: token expired ---

    #[test]
    fn rejects_expired_token() {
        let envelope = SessionHandoffEnvelope::new(
            1,
            make_token(42, 0, 1_000),
            make_snapshot(42),
            make_source(),
            make_dest(),
            1,
            0,
        );
        assert_eq!(envelope.validate(2_000).unwrap_err(), SessionHandoffError::TokenExpired);
    }

    // --- Envelope validation: token too short ---

    #[test]
    fn rejects_short_token() {
        let envelope = SessionHandoffEnvelope::new(
            1,
            SessionToken::new(vec![0u8; 5], 42, 0, 60_000),
            make_snapshot(42),
            make_source(),
            make_dest(),
            1,
            0,
        );
        assert_eq!(
            envelope.validate(100).unwrap_err(),
            SessionHandoffError::TokenTooShort
        );
    }

    // --- Envelope validation: player ID mismatch ---

    #[test]
    fn rejects_player_id_mismatch() {
        let envelope = SessionHandoffEnvelope::new(
            1,
            make_token(42, 0, 60_000),
            make_snapshot(99), // different player
            make_source(),
            make_dest(),
            1,
            0,
        );
        assert_eq!(
            envelope.validate(100).unwrap_err(),
            SessionHandoffError::PlayerIdMismatch {
                token_player: 42,
                snapshot_player: 99,
            }
        );
    }

    // --- Envelope validation: snapshot too large ---

    #[test]
    fn rejects_oversized_snapshot() {
        let mut snapshot = make_snapshot(42);
        snapshot.data = vec![0u8; MAX_SNAPSHOT_SIZE + 1];
        let envelope = SessionHandoffEnvelope::new(
            1,
            make_token(42, 0, 60_000),
            snapshot,
            make_source(),
            make_dest(),
            1,
            0,
        );
        assert!(matches!(
            envelope.validate(100).unwrap_err(),
            SessionHandoffError::SnapshotTooLarge { .. }
        ));
    }

    // --- Envelope validation: timeout ---

    #[test]
    fn rejects_timed_out_handoff() {
        let envelope = make_envelope().with_timeout_ms(1_000);
        assert_eq!(
            envelope.validate(2_000).unwrap_err(),
            SessionHandoffError::Timeout
        );
    }

    // --- Envelope validation: same world ---

    #[test]
    fn rejects_same_world() {
        let source = AetherUrl::parse("aether://host/same-world").unwrap();
        let dest = AetherUrl::parse("aether://host/same-world").unwrap();
        let envelope = SessionHandoffEnvelope::new(
            1,
            make_token(42, 0, 60_000),
            make_snapshot(42),
            source,
            dest,
            1,
            0,
        );
        assert_eq!(
            envelope.validate(100).unwrap_err(),
            SessionHandoffError::SameWorld
        );
    }

    #[test]
    fn different_host_same_world_id_is_allowed() {
        let source = AetherUrl::parse("aether://host-a/world").unwrap();
        let dest = AetherUrl::parse("aether://host-b/world").unwrap();
        let envelope = SessionHandoffEnvelope::new(
            1,
            make_token(42, 0, 60_000),
            make_snapshot(42),
            source,
            dest,
            1,
            0,
        );
        assert!(envelope.validate(100).is_ok());
    }

    // --- Timeout check ---

    #[test]
    fn is_timed_out() {
        let envelope = make_envelope().with_timeout_ms(5_000);
        assert!(!envelope.is_timed_out(3_000));
        assert!(envelope.is_timed_out(6_000));
    }

    // --- Snapshot at exact max size ---

    #[test]
    fn snapshot_at_exact_max_size_is_valid() {
        let mut snapshot = make_snapshot(42);
        snapshot.data = vec![0u8; MAX_SNAPSHOT_SIZE];
        let envelope = SessionHandoffEnvelope::new(
            1,
            make_token(42, 0, 60_000),
            snapshot,
            make_source(),
            make_dest(),
            1,
            0,
        );
        assert!(envelope.validate(100).is_ok());
    }

    // --- Default constants ---

    #[test]
    fn default_constants() {
        assert_eq!(DEFAULT_SESSION_TOKEN_LENGTH, 32);
        assert_eq!(DEFAULT_SESSION_HANDOFF_TIMEOUT_MS, 30_000);
        assert_eq!(MAX_SNAPSHOT_SIZE, 1_048_576);
    }

    // --- Builder pattern ---

    #[test]
    fn with_timeout_ms() {
        let envelope = make_envelope().with_timeout_ms(99_000);
        assert_eq!(envelope.timeout_ms, 99_000);
    }

    // --- Handoff ID ---

    #[test]
    fn handoff_id_is_stored() {
        let envelope = make_envelope();
        assert_eq!(envelope.handoff_id, 1);
    }

    // --- Token boundary: exact expiry time ---

    #[test]
    fn token_exact_expiry_boundary() {
        let token = make_token(1, 0, 1000);
        // At exactly expires_ms, the token is expired
        assert!(token.is_expired(1000));
        assert!(!token.is_expired(999));
    }
}
