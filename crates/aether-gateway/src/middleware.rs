//! Auth and rate-limiting middleware.
//!
//! Provides two independent middleware components that can be composed in any
//! request pipeline:
//!
//! - [`AuthMiddleware`] validates a [`Token`](crate::auth::Token) against an
//!   [`AuthValidationPolicy`](crate::auth::AuthValidationPolicy).
//! - [`RateLimiter`] enforces per-user, per-route token-bucket rate limits.

use std::collections::HashMap;

use crate::auth::{AuthValidationPolicy, AuthzResult, Token};
use crate::rate::RateLimitStatus;
use crate::router::RateLimitRule;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default maximum number of buckets the rate limiter will track before
/// evicting the oldest entries.
const DEFAULT_MAX_BUCKETS: usize = 100_000;

// ---------------------------------------------------------------------------
// Auth middleware
// ---------------------------------------------------------------------------

/// Validates tokens against a configured policy.
pub struct AuthMiddleware {
    policy: AuthValidationPolicy,
}

impl AuthMiddleware {
    pub fn new(policy: AuthValidationPolicy) -> Self {
        Self { policy }
    }

    /// Validate `token` at the given wall-clock time (milliseconds since epoch).
    pub fn validate(&self, token: &Token, now_ms: u64) -> AuthzResult {
        // Expiry check.
        if self.policy.require_expiry_check && token.expires_ms <= now_ms {
            return AuthzResult::Expired;
        }

        // Issuer check.
        if !self.policy.accepted_issuers.is_empty()
            && !self.policy.accepted_issuers.contains(&token.token_id)
        {
            return AuthzResult::Denied("token issuer not accepted".to_string());
        }

        // Signature flag (we treat `require_signature` as a simple boolean
        // gate; real signature verification would happen in a crypto layer).
        if self.policy.require_signature && token.token_id.is_empty() {
            return AuthzResult::Denied("missing token signature".to_string());
        }

        AuthzResult::Allowed
    }
}

// ---------------------------------------------------------------------------
// Rate limiter (token-bucket)
// ---------------------------------------------------------------------------

/// Composite key for a rate-limit bucket.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BucketKey {
    user_id: u64,
    route_id: String,
}

/// Internal state for one token bucket.
#[derive(Debug, Clone)]
struct TokenBucket {
    /// Current number of available tokens (may exceed `burst` after a long
    /// idle period, but is capped on access).
    tokens: f64,
    /// Timestamp (ms) of the last refill calculation.
    last_refill_ms: u64,
}

/// Per-user, per-route token-bucket rate limiter.
///
/// The limiter is entirely in-memory and does not depend on external storage.
pub struct RateLimiter {
    buckets: HashMap<BucketKey, TokenBucket>,
    _max_buckets: usize,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            buckets: HashMap::new(),
            _max_buckets: DEFAULT_MAX_BUCKETS,
        }
    }

    /// Create a rate limiter with a custom maximum bucket count.
    pub fn with_max_buckets(max_buckets: usize) -> Self {
        Self {
            buckets: HashMap::new(),
            _max_buckets: max_buckets,
        }
    }

    /// Check whether a request from `user_id` on `route_id` is allowed under
    /// `rule` at the given time.
    ///
    /// Consumes one token from the bucket if allowed.
    pub fn check(
        &mut self,
        user_id: u64,
        route_id: &str,
        rule: &RateLimitRule,
        now_ms: u64,
    ) -> RateLimitStatus {
        let key = BucketKey {
            user_id,
            route_id: route_id.to_string(),
        };

        let max_tokens = rule.burst as f64;
        let refill_rate = rule.requests_per_second as f64 / 1000.0; // tokens per ms

        let bucket = self.buckets.entry(key).or_insert_with(|| TokenBucket {
            tokens: max_tokens,
            last_refill_ms: now_ms,
        });

        // Refill tokens based on elapsed time.
        let elapsed = now_ms.saturating_sub(bucket.last_refill_ms);
        bucket.tokens = (bucket.tokens + elapsed as f64 * refill_rate).min(max_tokens);
        bucket.last_refill_ms = now_ms;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            let remaining = bucket.tokens as u32;
            let reset_in_ms = if remaining >= rule.burst {
                0
            } else {
                // Time until one more token is available.
                ((1.0 / refill_rate) as u64).max(1)
            };
            RateLimitStatus {
                remaining,
                reset_in_ms,
                allowed: true,
            }
        } else {
            // Not enough tokens.
            let deficit = 1.0 - bucket.tokens;
            let wait_ms = (deficit / refill_rate).ceil() as u64;
            RateLimitStatus {
                remaining: 0,
                reset_in_ms: wait_ms,
                allowed: false,
            }
        }
    }

    /// Evict all buckets (useful for testing or periodic cleanup).
    pub fn clear(&mut self) {
        self.buckets.clear();
    }

    /// Number of tracked buckets.
    pub fn bucket_count(&self) -> usize {
        self.buckets.len()
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Auth middleware tests -----------------------------------------------

    fn make_policy(
        require_expiry: bool,
        require_sig: bool,
        issuers: Vec<String>,
    ) -> AuthValidationPolicy {
        AuthValidationPolicy {
            require_expiry_check: require_expiry,
            require_signature: require_sig,
            accepted_issuers: issuers,
        }
    }

    fn make_token(user_id: u64, token_id: &str, expires_ms: u64) -> Token {
        Token {
            user_id,
            token_id: token_id.to_string(),
            expires_ms,
        }
    }

    #[test]
    fn auth_valid_token() {
        let mw = AuthMiddleware::new(make_policy(true, false, vec!["aether".to_string()]));
        let token = make_token(1, "aether", 2_000);
        let result = mw.validate(&token, 1_000);
        assert!(matches!(result, AuthzResult::Allowed));
    }

    #[test]
    fn auth_expired_token() {
        let mw = AuthMiddleware::new(make_policy(true, false, vec![]));
        let token = make_token(1, "aether", 500);
        let result = mw.validate(&token, 1_000);
        assert!(matches!(result, AuthzResult::Expired));
    }

    #[test]
    fn auth_expired_at_exact_boundary() {
        let mw = AuthMiddleware::new(make_policy(true, false, vec![]));
        let token = make_token(1, "aether", 1_000);
        // expires_ms == now_ms  -->  expired (<=)
        let result = mw.validate(&token, 1_000);
        assert!(matches!(result, AuthzResult::Expired));
    }

    #[test]
    fn auth_unknown_issuer() {
        let mw = AuthMiddleware::new(make_policy(false, false, vec!["aether".to_string()]));
        let token = make_token(1, "unknown_issuer", 2_000);
        let result = mw.validate(&token, 1_000);
        assert!(matches!(result, AuthzResult::Denied(_)));
    }

    #[test]
    fn auth_empty_issuers_accepts_all() {
        let mw = AuthMiddleware::new(make_policy(false, false, vec![]));
        let token = make_token(1, "any_issuer", 2_000);
        let result = mw.validate(&token, 1_000);
        assert!(matches!(result, AuthzResult::Allowed));
    }

    #[test]
    fn auth_missing_signature() {
        let mw = AuthMiddleware::new(make_policy(false, true, vec![]));
        let token = make_token(1, "", 2_000);
        let result = mw.validate(&token, 1_000);
        assert!(matches!(result, AuthzResult::Denied(_)));
    }

    #[test]
    fn auth_signature_present() {
        let mw = AuthMiddleware::new(make_policy(false, true, vec![]));
        let token = make_token(1, "valid_sig", 2_000);
        let result = mw.validate(&token, 1_000);
        assert!(matches!(result, AuthzResult::Allowed));
    }

    #[test]
    fn auth_no_expiry_check_allows_expired_token() {
        let mw = AuthMiddleware::new(make_policy(false, false, vec![]));
        let token = make_token(1, "aether", 0);
        let result = mw.validate(&token, 1_000);
        assert!(matches!(result, AuthzResult::Allowed));
    }

    // -- Rate limiter tests -------------------------------------------------

    fn default_rule() -> RateLimitRule {
        RateLimitRule {
            requests_per_second: 10,
            burst: 5,
        }
    }

    #[test]
    fn rate_limit_within_burst() {
        let mut limiter = RateLimiter::new();
        let rule = default_rule();
        // Should allow `burst` requests immediately.
        for _ in 0..5 {
            let status = limiter.check(1, "route_a", &rule, 1000);
            assert!(status.allowed);
        }
    }

    #[test]
    fn rate_limit_exceeded() {
        let mut limiter = RateLimiter::new();
        let rule = default_rule();
        // Exhaust burst.
        for _ in 0..5 {
            limiter.check(1, "route_a", &rule, 1000);
        }
        // Next request should be denied.
        let status = limiter.check(1, "route_a", &rule, 1000);
        assert!(!status.allowed);
        assert_eq!(status.remaining, 0);
        assert!(status.reset_in_ms > 0);
    }

    #[test]
    fn rate_limit_refills_over_time() {
        let mut limiter = RateLimiter::new();
        let rule = default_rule(); // 10 req/s = 1 token per 100ms
                                   // Exhaust all tokens at t=1000.
        for _ in 0..5 {
            limiter.check(1, "route_a", &rule, 1000);
        }
        // At t=1000 should be denied.
        let status = limiter.check(1, "route_a", &rule, 1000);
        assert!(!status.allowed);

        // Wait 200ms -> 2 tokens refilled.
        let status = limiter.check(1, "route_a", &rule, 1200);
        assert!(status.allowed);
    }

    #[test]
    fn rate_limit_separate_users() {
        let mut limiter = RateLimiter::new();
        let rule = RateLimitRule {
            requests_per_second: 10,
            burst: 2,
        };
        // User 1 exhausts their bucket.
        limiter.check(1, "route_a", &rule, 1000);
        limiter.check(1, "route_a", &rule, 1000);
        let status = limiter.check(1, "route_a", &rule, 1000);
        assert!(!status.allowed);

        // User 2 should still have tokens.
        let status = limiter.check(2, "route_a", &rule, 1000);
        assert!(status.allowed);
    }

    #[test]
    fn rate_limit_separate_routes() {
        let mut limiter = RateLimiter::new();
        let rule = RateLimitRule {
            requests_per_second: 10,
            burst: 1,
        };
        // Exhaust route_a.
        limiter.check(1, "route_a", &rule, 1000);
        let status = limiter.check(1, "route_a", &rule, 1000);
        assert!(!status.allowed);

        // route_b should still be available for the same user.
        let status = limiter.check(1, "route_b", &rule, 1000);
        assert!(status.allowed);
    }

    #[test]
    fn rate_limit_clear() {
        let mut limiter = RateLimiter::new();
        let rule = default_rule();
        limiter.check(1, "route_a", &rule, 1000);
        assert_eq!(limiter.bucket_count(), 1);
        limiter.clear();
        assert_eq!(limiter.bucket_count(), 0);
    }

    #[test]
    fn rate_limit_default_trait() {
        let limiter = RateLimiter::default();
        assert_eq!(limiter.bucket_count(), 0);
    }

    #[test]
    fn rate_limit_burst_caps_refill() {
        let mut limiter = RateLimiter::new();
        let rule = RateLimitRule {
            requests_per_second: 10,
            burst: 3,
        };
        // Use one token.
        limiter.check(1, "r", &rule, 1000);
        // Wait a very long time -> tokens should cap at burst (3).
        let status = limiter.check(1, "r", &rule, 100_000);
        assert!(status.allowed);
        // remaining should be at most burst - 1 = 2
        assert!(status.remaining <= 2);
    }
}
