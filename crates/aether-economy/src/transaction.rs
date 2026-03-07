use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionDirection {
    Purchase,
    Sale,
    Tip,
    Reward,
    Payout,
}

#[derive(Debug, Clone)]
pub enum TransactionKind {
    Sync,
    AsyncSettlement,
}

#[derive(Debug, Clone)]
pub enum TransactionState {
    Queued,
    InFlight,
    Committed,
    Rejected,
    Settled,
    Reversed,
}

#[derive(Debug)]
pub enum TransactionError {
    DuplicateIdempotencyKey,
    InsufficientFunds,
    Conflict,
    Timeout,
}

#[derive(Debug, Clone)]
pub struct EconomyTransaction {
    pub tx_id: String,
    pub player_id: u64,
    pub world_id: String,
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
    pub key_v7: String,
    pub state: TransactionState,
    pub ttl_days: u16,
    pub created_ms: u64,
}

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

    pub fn enqueue(&mut self, transaction: EconomyTransaction) -> Duration {
        self.pending.push(transaction);
        let _ = self.retention_days;
        Duration::from_millis(5)
    }

    pub fn mark_settled(&mut self, tx_id: &str) -> Option<EconomyTransaction> {
        let idx = self
            .pending
            .iter()
            .position(|tx| tx.tx_id == tx_id)?;
        Some(self.pending.swap_remove(idx))
    }
}

