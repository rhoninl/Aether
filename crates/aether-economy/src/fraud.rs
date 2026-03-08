//! Fraud detection: velocity checks and anomaly scoring.

use std::collections::HashMap;

/// Default maximum transactions per minute before velocity flag triggers.
const DEFAULT_MAX_TX_PER_MINUTE: u32 = 60;

/// Default amount threshold (in minor units) for anomaly detection.
const DEFAULT_ANOMALY_AMOUNT_THRESHOLD: i128 = 1_000_000;

/// Default fraud score above which a transaction is blocked.
const DEFAULT_BLOCK_THRESHOLD: f32 = 0.8;

#[derive(Debug, Clone)]
pub struct VelocityWindow {
    pub player_id: u64,
    /// Timestamps (ms) of transactions within the current sliding window.
    pub timestamps_ms: Vec<u64>,
    pub tx_per_minute: u32,
    pub amount_last_minute_minor: i128,
}

#[derive(Debug, Clone)]
pub struct FraudScore {
    pub score: f32,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FraudSignal {
    HighVelocity,
    RepeatedDeniedTx,
    OutlierAmount,
    GeofenceMismatch,
    NewAccountBurst,
}

#[derive(Debug, Clone)]
pub struct AnomalySignal {
    pub player_id: u64,
    pub signals: Vec<FraudSignal>,
    pub score: FraudScore,
}

impl AnomalySignal {
    pub fn is_block_candidate(&self, threshold: f32) -> bool {
        self.score.score >= threshold
    }
}

/// Fraud detector that evaluates transactions against configurable rules.
#[derive(Debug)]
pub struct FraudDetector {
    max_tx_per_minute: u32,
    anomaly_amount_threshold: i128,
    block_threshold: f32,
    velocity: HashMap<u64, VelocityWindow>,
}

impl FraudDetector {
    pub fn new() -> Self {
        Self::with_config(
            DEFAULT_MAX_TX_PER_MINUTE,
            DEFAULT_ANOMALY_AMOUNT_THRESHOLD,
            DEFAULT_BLOCK_THRESHOLD,
        )
    }

    pub fn with_config(
        max_tx_per_minute: u32,
        anomaly_amount_threshold: i128,
        block_threshold: f32,
    ) -> Self {
        Self {
            max_tx_per_minute,
            anomaly_amount_threshold,
            block_threshold,
            velocity: HashMap::new(),
        }
    }

    /// Creates a FraudDetector from environment variables, falling back to
    /// defaults if variables are not set.
    pub fn from_env() -> Self {
        let max_tx = std::env::var("AETHER_ECONOMY_MAX_TX_PER_MINUTE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_TX_PER_MINUTE);
        let anomaly_threshold = std::env::var("AETHER_ECONOMY_ANOMALY_AMOUNT_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_ANOMALY_AMOUNT_THRESHOLD);
        let block_threshold = std::env::var("AETHER_ECONOMY_FRAUD_SCORE_BLOCK_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_BLOCK_THRESHOLD);
        Self::with_config(max_tx, anomaly_threshold, block_threshold)
    }

    /// Evaluates a transaction and returns an `AnomalySignal`.
    /// The caller decides whether to block based on `is_block_candidate`.
    pub fn evaluate(
        &mut self,
        player_id: u64,
        amount_minor: i128,
        now_ms: u64,
    ) -> AnomalySignal {
        let window = self.velocity.entry(player_id).or_insert_with(|| {
            VelocityWindow {
                player_id,
                timestamps_ms: Vec::new(),
                tx_per_minute: 0,
                amount_last_minute_minor: 0,
            }
        });

        // Sliding window: remove timestamps older than 60 seconds
        let cutoff = now_ms.saturating_sub(60_000);
        window.timestamps_ms.retain(|&ts| ts >= cutoff);

        // Add current transaction
        window.timestamps_ms.push(now_ms);
        window.tx_per_minute = window.timestamps_ms.len() as u32;
        window.amount_last_minute_minor = window
            .amount_last_minute_minor
            .saturating_add(amount_minor);

        let mut signals = Vec::new();
        let mut score: f32 = 0.0;

        // Velocity check
        if window.tx_per_minute > self.max_tx_per_minute {
            signals.push(FraudSignal::HighVelocity);
            score += 0.5;
        }

        // Anomaly amount check
        if amount_minor > self.anomaly_amount_threshold {
            signals.push(FraudSignal::OutlierAmount);
            score += 0.4;
        }

        score = score.min(1.0);

        let reason = if signals.is_empty() {
            None
        } else {
            Some(format!("{} signal(s) detected", signals.len()))
        };

        AnomalySignal {
            player_id,
            signals,
            score: FraudScore { score, reason },
        }
    }

    /// Returns the configured block threshold.
    pub fn block_threshold(&self) -> f32 {
        self.block_threshold
    }

    /// Checks whether the given anomaly signal should block the transaction.
    pub fn should_block(&self, signal: &AnomalySignal) -> bool {
        signal.is_block_candidate(self.block_threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_fraud_for_normal_transaction() {
        let mut detector = FraudDetector::with_config(60, 1_000_000, 0.8);
        let signal = detector.evaluate(1, 100, 1000);
        assert!(signal.signals.is_empty());
        assert!(!detector.should_block(&signal));
    }

    #[test]
    fn velocity_triggers_on_exceeded_limit() {
        let mut detector = FraudDetector::with_config(3, 1_000_000, 0.8);
        // 4 transactions in quick succession (same minute)
        for i in 0..4 {
            let _ = detector.evaluate(1, 10, 1000 + i);
        }
        let signal = detector.evaluate(1, 10, 1004);
        assert!(signal.signals.contains(&FraudSignal::HighVelocity));
    }

    #[test]
    fn velocity_resets_after_window_expires() {
        let mut detector = FraudDetector::with_config(3, 1_000_000, 0.8);
        // 3 transactions at t=0
        for i in 0..3 {
            detector.evaluate(1, 10, i);
        }
        // Next transaction after 61 seconds -- old ones should be pruned
        let signal = detector.evaluate(1, 10, 61_000);
        assert!(!signal.signals.contains(&FraudSignal::HighVelocity));
    }

    #[test]
    fn large_amount_triggers_outlier() {
        let mut detector = FraudDetector::with_config(60, 1000, 0.8);
        let signal = detector.evaluate(1, 5000, 1000);
        assert!(signal.signals.contains(&FraudSignal::OutlierAmount));
    }

    #[test]
    fn combined_signals_block_transaction() {
        // max_tx=1 so second tx triggers velocity, anomaly_threshold=100
        let mut detector = FraudDetector::with_config(1, 100, 0.8);
        detector.evaluate(1, 10, 1000);
        // Second tx with large amount in same window
        let signal = detector.evaluate(1, 500, 1001);
        assert!(signal.signals.contains(&FraudSignal::HighVelocity));
        assert!(signal.signals.contains(&FraudSignal::OutlierAmount));
        // 0.5 + 0.4 = 0.9 >= 0.8 threshold
        assert!(detector.should_block(&signal));
    }

    #[test]
    fn different_players_independent() {
        let mut detector = FraudDetector::with_config(2, 1_000_000, 0.8);
        // Player 1: 3 transactions (exceeds limit of 2)
        for i in 0..3 {
            detector.evaluate(1, 10, 1000 + i);
        }
        // Player 2: 1 transaction (well within limit)
        let signal = detector.evaluate(2, 10, 1003);
        assert!(signal.signals.is_empty());
    }

    #[test]
    fn score_capped_at_one() {
        // Both signals fire: 0.5 + 0.4 = 0.9, capped at 1.0 by min()
        let mut detector = FraudDetector::with_config(0, 0, 0.5);
        let signal = detector.evaluate(1, 1, 1000);
        assert!(signal.score.score <= 1.0);
    }

    #[test]
    fn block_threshold_respected() {
        let mut detector = FraudDetector::with_config(60, 1_000_000, 0.3);
        // No signals -> score 0.0
        let signal = detector.evaluate(1, 10, 1000);
        assert!(!detector.should_block(&signal));
    }
}
