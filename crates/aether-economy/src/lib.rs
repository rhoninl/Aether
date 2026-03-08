//! Economy and ledger service for Aether: double-entry ledger, wallet
//! management, fraud detection, idempotent transaction processing, and
//! settlement/payout lifecycle.

pub mod fraud;
pub mod ledger;
pub mod service;
pub mod settlement;
pub mod transaction;
pub mod wallet;

pub use fraud::{AnomalySignal, FraudDetector, FraudScore, FraudSignal, VelocityWindow};
pub use ledger::{CurrencyLedger, LedgerEntry, LedgerError, LedgerKind, LedgerRecord};
pub use service::{EconomyService, ServiceError, ServiceResult, TransactionRequest};
pub use settlement::{
    PayoutDestination, PayoutRecord, SettlementError, SettlementProcessor, SettlementState,
};
pub use transaction::{
    EconomyTransaction, IdempotencyGuard, IdempotencyRecord, TransactionCoordinator,
    TransactionDirection, TransactionError, TransactionKind, TransactionState,
};
pub use wallet::{
    WalletAccount, WalletConstraint, WalletError, WalletManager, WalletOperation, WalletStatus,
    WalletSummary,
};
