//! Token-bucket action rate limiter.
//!
//! Implements per-(player, action) rate limiting using a token bucket
//! algorithm. Each action type has configurable tokens-per-second and
//! burst capacity.

use std::collections::HashMap;
use std::fmt;

use crate::ratelimit::ActionKey;

/// Default burst capacity (max tokens in bucket).
const DEFAULT_BURST_CAPACITY: u32 = 10;

/// Default token refill rate (tokens per second).
const DEFAULT_TOKENS_PER_SECOND: f64 = 5.0;

/// Configuration for a single action type's rate limit.
#[derive(Debug, Clone)]
pub struct ActionRateConfig {
    /// Maximum tokens that can accumulate (burst capacity).
    pub burst_capacity: u32,
    /// Tokens added per second.
    pub tokens_per_second: f64,
}

impl Default for ActionRateConfig {
    fn default() -> Self {
        Self {
            burst_capacity: DEFAULT_BURST_CAPACITY,
            tokens_per_second: DEFAULT_TOKENS_PER_SECOND,
        }
    }
}

/// Internal bucket state for a (player, action) pair.
#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: f64,
    last_refill_secs: f64,
}

/// Result of attempting to consume a rate limit token.
#[derive(Debug, Clone, PartialEq)]
pub enum RateLimitResult {
    /// Action is allowed. Contains remaining tokens.
    Allowed { remaining_tokens: u32 },
    /// Action is rate-limited. Contains seconds until next token is available.
    Limited { retry_after_secs: f64 },
    /// Invalid input.
    InvalidInput { reason: String },
}

impl fmt::Display for RateLimitResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RateLimitResult::Allowed { remaining_tokens } => {
                write!(f, "allowed, {} tokens remaining", remaining_tokens)
            }
            RateLimitResult::Limited { retry_after_secs } => {
                write!(f, "rate limited, retry after {:.2}s", retry_after_secs)
            }
            RateLimitResult::InvalidInput { reason } => {
                write!(f, "invalid input: {}", reason)
            }
        }
    }
}

/// Per-player, per-action token bucket rate limiter.
#[derive(Debug)]
pub struct ActionRateLimiter {
    /// Per-action configuration. Actions not in this map use the default config.
    configs: HashMap<ActionKey, ActionRateConfig>,
    /// Default config for actions without explicit configuration.
    default_config: ActionRateConfig,
    /// Bucket state per (player_id, action).
    buckets: HashMap<(u64, ActionKey), TokenBucket>,
}

impl ActionRateLimiter {
    /// Creates a new rate limiter with default configuration for all actions.
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            default_config: ActionRateConfig::default(),
            buckets: HashMap::new(),
        }
    }

    /// Creates a new rate limiter with the given default configuration.
    pub fn with_default_config(config: ActionRateConfig) -> Self {
        Self {
            configs: HashMap::new(),
            default_config: config,
            buckets: HashMap::new(),
        }
    }

    /// Sets the rate limit configuration for a specific action type.
    pub fn set_action_config(&mut self, action: ActionKey, config: ActionRateConfig) {
        self.configs.insert(action, config);
    }

    /// Attempts to consume one token for the given player and action.
    ///
    /// # Arguments
    /// - `player_id`: The player performing the action.
    /// - `action`: The type of action being performed.
    /// - `now_secs`: Current time in seconds (monotonic clock).
    ///
    /// # Returns
    /// `RateLimitResult::Allowed` if the action is permitted,
    /// `RateLimitResult::Limited` if the player is rate-limited.
    pub fn try_consume(
        &mut self,
        player_id: u64,
        action: ActionKey,
        now_secs: f64,
    ) -> RateLimitResult {
        if now_secs < 0.0 {
            return RateLimitResult::InvalidInput {
                reason: "timestamp must be non-negative".to_string(),
            };
        }

        let config = self.configs.get(&action).unwrap_or(&self.default_config);
        let burst = config.burst_capacity;
        let rate = config.tokens_per_second;

        let key = (player_id, action);
        let bucket = self.buckets.entry(key).or_insert_with(|| TokenBucket {
            tokens: burst as f64,
            last_refill_secs: now_secs,
        });

        // Refill tokens based on elapsed time
        let elapsed = now_secs - bucket.last_refill_secs;
        if elapsed > 0.0 {
            bucket.tokens = (bucket.tokens + elapsed * rate).min(burst as f64);
            bucket.last_refill_secs = now_secs;
        }

        // Try to consume one token
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            RateLimitResult::Allowed {
                remaining_tokens: bucket.tokens as u32,
            }
        } else {
            let deficit = 1.0 - bucket.tokens;
            let retry_after = if rate > 0.0 { deficit / rate } else { f64::MAX };
            RateLimitResult::Limited {
                retry_after_secs: retry_after,
            }
        }
    }

    /// Resets all buckets for a player (e.g., on disconnect).
    pub fn reset_player(&mut self, player_id: u64) {
        self.buckets.retain(|(pid, _), _| *pid != player_id);
    }

    /// Returns the number of active buckets.
    pub fn bucket_count(&self) -> usize {
        self.buckets.len()
    }
}

impl Default for ActionRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Basic allowed ---

    #[test]
    fn test_first_action_allowed() {
        let mut limiter = ActionRateLimiter::new();
        let result = limiter.try_consume(1, ActionKey::Move, 0.0);
        assert!(matches!(result, RateLimitResult::Allowed { .. }));
    }

    #[test]
    fn test_burst_allows_multiple() {
        let mut limiter = ActionRateLimiter::new();
        // Default burst = 10, so 10 actions should all be allowed
        for i in 0..10 {
            let result = limiter.try_consume(1, ActionKey::Move, 0.0);
            assert!(
                matches!(result, RateLimitResult::Allowed { .. }),
                "action {} should be allowed",
                i
            );
        }
    }

    #[test]
    fn test_remaining_tokens_decreases() {
        let mut limiter = ActionRateLimiter::new();
        match limiter.try_consume(1, ActionKey::Move, 0.0) {
            RateLimitResult::Allowed { remaining_tokens } => {
                assert_eq!(remaining_tokens, 9); // 10 - 1
            }
            other => panic!("expected Allowed, got {:?}", other),
        }
    }

    // --- Rate limiting ---

    #[test]
    fn test_rate_limited_after_burst() {
        let mut limiter = ActionRateLimiter::new();
        // Exhaust all 10 tokens
        for _ in 0..10 {
            limiter.try_consume(1, ActionKey::Move, 0.0);
        }
        // 11th action should be limited
        let result = limiter.try_consume(1, ActionKey::Move, 0.0);
        assert!(matches!(result, RateLimitResult::Limited { .. }));
    }

    #[test]
    fn test_rate_limited_retry_after() {
        let mut limiter = ActionRateLimiter::new();
        for _ in 0..10 {
            limiter.try_consume(1, ActionKey::Move, 0.0);
        }
        match limiter.try_consume(1, ActionKey::Move, 0.0) {
            RateLimitResult::Limited { retry_after_secs } => {
                // Default rate = 5 tokens/sec, need 1 token -> 0.2s
                assert!((retry_after_secs - 0.2).abs() < 0.01);
            }
            other => panic!("expected Limited, got {:?}", other),
        }
    }

    // --- Token refill ---

    #[test]
    fn test_tokens_refill_over_time() {
        let mut limiter = ActionRateLimiter::new();
        // Exhaust burst
        for _ in 0..10 {
            limiter.try_consume(1, ActionKey::Move, 0.0);
        }
        assert!(matches!(
            limiter.try_consume(1, ActionKey::Move, 0.0),
            RateLimitResult::Limited { .. }
        ));

        // Wait 1 second -> 5 tokens refilled at 5/sec
        let result = limiter.try_consume(1, ActionKey::Move, 1.0);
        assert!(matches!(result, RateLimitResult::Allowed { .. }));
    }

    #[test]
    fn test_tokens_dont_exceed_burst() {
        let mut limiter = ActionRateLimiter::new();
        // First action at t=0
        limiter.try_consume(1, ActionKey::Move, 0.0);
        // Wait a very long time -> tokens should cap at burst (10)
        let result = limiter.try_consume(1, ActionKey::Move, 1000.0);
        match result {
            RateLimitResult::Allowed { remaining_tokens } => {
                assert_eq!(remaining_tokens, 9); // capped at 10, minus 1
            }
            other => panic!("expected Allowed, got {:?}", other),
        }
    }

    // --- Per-player isolation ---

    #[test]
    fn test_different_players_independent() {
        let mut limiter = ActionRateLimiter::new();
        // Exhaust player 1's burst
        for _ in 0..10 {
            limiter.try_consume(1, ActionKey::Move, 0.0);
        }
        assert!(matches!(
            limiter.try_consume(1, ActionKey::Move, 0.0),
            RateLimitResult::Limited { .. }
        ));

        // Player 2 should still have full burst
        let result = limiter.try_consume(2, ActionKey::Move, 0.0);
        assert!(matches!(result, RateLimitResult::Allowed { .. }));
    }

    // --- Per-action isolation ---

    #[test]
    fn test_different_actions_independent() {
        let mut limiter = ActionRateLimiter::new();
        // Exhaust Move tokens
        for _ in 0..10 {
            limiter.try_consume(1, ActionKey::Move, 0.0);
        }
        assert!(matches!(
            limiter.try_consume(1, ActionKey::Move, 0.0),
            RateLimitResult::Limited { .. }
        ));

        // Chat should still be allowed
        let result = limiter.try_consume(1, ActionKey::Chat, 0.0);
        assert!(matches!(result, RateLimitResult::Allowed { .. }));
    }

    // --- Custom action config ---

    #[test]
    fn test_custom_action_config() {
        let mut limiter = ActionRateLimiter::new();
        limiter.set_action_config(
            ActionKey::Trade,
            ActionRateConfig {
                burst_capacity: 2,
                tokens_per_second: 1.0,
            },
        );

        // 2 trades allowed
        assert!(matches!(
            limiter.try_consume(1, ActionKey::Trade, 0.0),
            RateLimitResult::Allowed { .. }
        ));
        assert!(matches!(
            limiter.try_consume(1, ActionKey::Trade, 0.0),
            RateLimitResult::Allowed { .. }
        ));
        // 3rd trade limited
        assert!(matches!(
            limiter.try_consume(1, ActionKey::Trade, 0.0),
            RateLimitResult::Limited { .. }
        ));

        // Move still uses default (10 burst)
        assert!(matches!(
            limiter.try_consume(1, ActionKey::Move, 0.0),
            RateLimitResult::Allowed { .. }
        ));
    }

    #[test]
    fn test_custom_refill_rate() {
        let mut limiter = ActionRateLimiter::new();
        limiter.set_action_config(
            ActionKey::Chat,
            ActionRateConfig {
                burst_capacity: 1,
                tokens_per_second: 10.0,
            },
        );
        // Use the only token
        limiter.try_consume(1, ActionKey::Chat, 0.0);
        // Limited
        assert!(matches!(
            limiter.try_consume(1, ActionKey::Chat, 0.0),
            RateLimitResult::Limited { .. }
        ));
        // Wait 0.1 seconds -> 1 token refilled at 10/sec
        let result = limiter.try_consume(1, ActionKey::Chat, 0.1);
        assert!(matches!(result, RateLimitResult::Allowed { .. }));
    }

    // --- Reset player ---

    #[test]
    fn test_reset_player() {
        let mut limiter = ActionRateLimiter::new();
        for _ in 0..10 {
            limiter.try_consume(1, ActionKey::Move, 0.0);
        }
        assert!(matches!(
            limiter.try_consume(1, ActionKey::Move, 0.0),
            RateLimitResult::Limited { .. }
        ));

        limiter.reset_player(1);
        // After reset, should get fresh burst
        let result = limiter.try_consume(1, ActionKey::Move, 0.0);
        assert!(matches!(result, RateLimitResult::Allowed { .. }));
    }

    #[test]
    fn test_reset_player_only_affects_target() {
        let mut limiter = ActionRateLimiter::new();
        limiter.try_consume(1, ActionKey::Move, 0.0);
        limiter.try_consume(2, ActionKey::Move, 0.0);
        assert_eq!(limiter.bucket_count(), 2);

        limiter.reset_player(1);
        assert_eq!(limiter.bucket_count(), 1);
    }

    // --- Invalid input ---

    #[test]
    fn test_negative_timestamp() {
        let mut limiter = ActionRateLimiter::new();
        let result = limiter.try_consume(1, ActionKey::Move, -1.0);
        assert!(matches!(result, RateLimitResult::InvalidInput { .. }));
    }

    // --- Bucket count ---

    #[test]
    fn test_bucket_count() {
        let mut limiter = ActionRateLimiter::new();
        assert_eq!(limiter.bucket_count(), 0);
        limiter.try_consume(1, ActionKey::Move, 0.0);
        assert_eq!(limiter.bucket_count(), 1);
        limiter.try_consume(1, ActionKey::Chat, 0.0);
        assert_eq!(limiter.bucket_count(), 2);
        limiter.try_consume(2, ActionKey::Move, 0.0);
        assert_eq!(limiter.bucket_count(), 3);
    }

    // --- Default trait ---

    #[test]
    fn test_default_trait() {
        let mut limiter = ActionRateLimiter::default();
        let result = limiter.try_consume(1, ActionKey::Move, 0.0);
        assert!(matches!(result, RateLimitResult::Allowed { .. }));
    }

    // --- Display ---

    #[test]
    fn test_display_allowed() {
        let r = RateLimitResult::Allowed {
            remaining_tokens: 5,
        };
        let s = r.to_string();
        assert!(s.contains("allowed"));
        assert!(s.contains("5"));
    }

    #[test]
    fn test_display_limited() {
        let r = RateLimitResult::Limited {
            retry_after_secs: 0.2,
        };
        let s = r.to_string();
        assert!(s.contains("rate limited"));
        assert!(s.contains("0.20"));
    }

    #[test]
    fn test_display_invalid() {
        let r = RateLimitResult::InvalidInput {
            reason: "bad ts".to_string(),
        };
        assert!(r.to_string().contains("bad ts"));
    }

    // --- Edge: zero rate ---

    #[test]
    fn test_zero_rate_stays_limited() {
        let config = ActionRateConfig {
            burst_capacity: 1,
            tokens_per_second: 0.0,
        };
        let mut limiter = ActionRateLimiter::with_default_config(config);
        limiter.try_consume(1, ActionKey::Move, 0.0);
        let result = limiter.try_consume(1, ActionKey::Move, 100.0);
        assert!(matches!(result, RateLimitResult::Limited { .. }));
    }

    // --- Sustained throughput ---

    #[test]
    fn test_sustained_throughput() {
        let mut limiter = ActionRateLimiter::new();
        // At 5 tokens/sec, one action every 0.2 seconds should be sustainable
        for i in 0..20 {
            let t = i as f64 * 0.25; // slightly more than 0.2s apart
            let result = limiter.try_consume(1, ActionKey::Move, t);
            assert!(
                matches!(result, RateLimitResult::Allowed { .. }),
                "action at t={} should be allowed",
                t
            );
        }
    }
}
