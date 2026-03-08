//! Transaction types and idempotency guard.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionDirection {
    Purchase,
    Sale,
    Tip,
    Reward,
    Payout,
    Trade,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionKind {
    Sync,
    AsyncSettlement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionState {
    Queued,
    InFlight,
    Committed,
    Rejected,
    Settled,
    Reversed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionError {
    DuplicateIdempotencyKey,
    InsufficientFunds,
    Conflict,
    Timeout,
}

impl std::fmt::Display for TransactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransactionError::DuplicateIdempotencyKey => write!(f, "duplicate idempotency key"),
            TransactionError::InsufficientFunds => write!(f, "insufficient funds"),
            TransactionError::Conflict => write!(f, "conflict"),
            TransactionError::Timeout => write!(f, "timeout"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EconomyTransaction {
    pub tx_id: String,
    pub from_wallet: String,
    pub to_wallet: String,
    pub amount_minor: i128,
    pub currency: String,
    pub direction: TransactionDirection,
    pub kind: TransactionKind,
    pub memo: Option<String>,
    pub created_ms: u64,
}

#[derive(Debug, Clone)]
pub struct IdempotencyRecord {
    pub tx_id: String,
    pub journal_id: u64,
    pub state: TransactionState,
    pub ttl_days: u16,
    pub created_ms: u64,
}

/// Guards against duplicate transaction processing by tracking seen tx_ids.
#[derive(Debug)]
pub struct IdempotencyGuard {
    seen: HashMap<String, IdempotencyRecord>,
    ttl_days: u16,
}

impl IdempotencyGuard {
    pub fn new(ttl_days: u16) -> Self {
        Self {
            seen: HashMap::new(),
            ttl_days,
        }
    }

    /// Checks whether a tx_id has already been processed.
    /// Returns `Some(journal_id)` if it was, `None` if it is new.
    pub fn check(&self, tx_id: &str) -> Option<u64> {
        self.seen.get(tx_id).map(|r| r.journal_id)
    }

    /// Records a completed transaction for future deduplication.
    pub fn record(&mut self, tx_id: &str, journal_id: u64, created_ms: u64) {
        self.seen.insert(
            tx_id.to_string(),
            IdempotencyRecord {
                tx_id: tx_id.to_string(),
                journal_id,
                state: TransactionState::Committed,
                ttl_days: self.ttl_days,
                created_ms,
            },
        );
    }

    /// Returns the number of tracked idempotency records.
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

/// Coordinator for pending async transactions.
#[derive(Debug)]
pub struct TransactionCoordinator {
    retention_days: u16,
    pending: Vec<EconomyTransaction>,
}

impl TransactionCoordinator {
    pub fn new(retention_days: u16) -> Self {
        Self {
            retention_days,
            pending: Vec::new(),
        }
    }

    pub fn enqueue(&mut self, transaction: EconomyTransaction) {
        let _ = self.retention_days;
        self.pending.push(transaction);
    }

    pub fn mark_settled(&mut self, tx_id: &str) -> Option<EconomyTransaction> {
        let idx = self.pending.iter().position(|tx| tx.tx_id == tx_id)?;
        Some(self.pending.swap_remove(idx))
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotency_new_tx_returns_none() {
        let guard = IdempotencyGuard::new(30);
        assert!(guard.check("tx-1").is_none());
    }

    #[test]
    fn idempotency_recorded_tx_returns_journal_id() {
        let mut guard = IdempotencyGuard::new(30);
        guard.record("tx-1", 42, 1000);
        assert_eq!(guard.check("tx-1"), Some(42));
    }

    #[test]
    fn idempotency_different_txs_independent() {
        let mut guard = IdempotencyGuard::new(30);
        guard.record("tx-1", 1, 1000);
        guard.record("tx-2", 2, 1001);
        assert_eq!(guard.check("tx-1"), Some(1));
        assert_eq!(guard.check("tx-2"), Some(2));
        assert!(guard.check("tx-3").is_none());
    }

    #[test]
    fn idempotency_len() {
        let mut guard = IdempotencyGuard::new(30);
        assert!(guard.is_empty());
        guard.record("tx-1", 1, 1000);
        assert_eq!(guard.len(), 1);
        guard.record("tx-2", 2, 1001);
        assert_eq!(guard.len(), 2);
    }

    #[test]
    fn coordinator_enqueue_and_settle() {
        let mut coord = TransactionCoordinator::new(30);
        let tx = EconomyTransaction {
            tx_id: "tx-1".to_string(),
            from_wallet: "w-a".to_string(),
            to_wallet: "w-b".to_string(),
            amount_minor: 100,
            currency: "AEC".to_string(),
            direction: TransactionDirection::Purchase,
            kind: TransactionKind::Sync,
            memo: None,
            created_ms: 1000,
        };
        coord.enqueue(tx);
        assert_eq!(coord.pending_count(), 1);
        let settled = coord.mark_settled("tx-1").unwrap();
        assert_eq!(settled.tx_id, "tx-1");
        assert_eq!(coord.pending_count(), 0);
    }

    #[test]
    fn coordinator_settle_nonexistent_returns_none() {
        let mut coord = TransactionCoordinator::new(30);
        assert!(coord.mark_settled("tx-ghost").is_none());
    }
}
