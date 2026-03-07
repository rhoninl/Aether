use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub struct RateLimiter {
    rate_per_second: u32,
    max_tokens: f64,
    tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    pub fn new(rate_per_second: u32, now: Instant) -> Self {
        let max_tokens = rate_per_second as f64;
        Self {
            rate_per_second,
            max_tokens,
            tokens: max_tokens,
            last_refill: now,
        }
    }

    pub fn try_take(&mut self, now: Instant, amount: u32) -> bool {
        self.refill(now);
        let needed = amount as f64;
        if self.tokens < needed {
            return false;
        }
        self.tokens -= needed;
        true
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last_refill).as_secs_f64();
        let refill = elapsed * self.rate_per_second as f64;
        self.tokens = (self.tokens + refill).min(self.max_tokens);
        self.last_refill = now;
    }

    #[allow(dead_code)]
    pub fn available_tokens(&self) -> u64 {
        self.tokens.floor() as u64
    }
}
