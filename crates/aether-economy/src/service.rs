//! EconomyService facade: ties together wallet, ledger, idempotency, fraud,
//! and settlement into a single API surface.

use crate::fraud::FraudDetector;
use crate::ledger::{CurrencyLedger, LedgerEntry, LedgerKind};
use crate::settlement::{PayoutRecord, SettlementProcessor};
use crate::transaction::{IdempotencyGuard, TransactionDirection};
use crate::wallet::{WalletAccount, WalletError, WalletManager, WalletOperation};

/// Default idempotency TTL in days.
const DEFAULT_IDEMPOTENCY_TTL_DAYS: u16 = 30;

/// Default platform fee in basis points (2.5%).
const DEFAULT_PLATFORM_FEE_BPS: u64 = 250;

/// Request to process a transaction through the economy service.
#[derive(Debug, Clone)]
pub struct TransactionRequest {
    pub tx_id: String,
    pub from_wallet: String,
    pub to_wallet: String,
    pub amount_minor: i128,
    pub direction: TransactionDirection,
    pub memo: Option<String>,
    pub timestamp_ms: u64,
}

/// Errors returned by the economy service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceError {
    WalletError(WalletError),
    InsufficientFunds,
    FraudBlocked(String),
    WalletFrozen(String),
    InvalidAmount,
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceError::WalletError(e) => write!(f, "wallet error: {e}"),
            ServiceError::InsufficientFunds => write!(f, "insufficient funds"),
            ServiceError::FraudBlocked(reason) => write!(f, "fraud blocked: {reason}"),
            ServiceError::WalletFrozen(id) => write!(f, "wallet frozen: {id}"),
            ServiceError::InvalidAmount => write!(f, "invalid amount"),
        }
    }
}

impl From<WalletError> for ServiceError {
    fn from(e: WalletError) -> Self {
        ServiceError::WalletError(e)
    }
}

/// Result type for economy service operations.
pub type ServiceResult<T> = Result<T, ServiceError>;

/// The main economy service facade.
#[derive(Debug)]
pub struct EconomyService {
    wallets: WalletManager,
    ledger: CurrencyLedger,
    idempotency: IdempotencyGuard,
    fraud: FraudDetector,
    settlement: SettlementProcessor,
    platform_fee_bps: u64,
}

impl EconomyService {
    pub fn new() -> Self {
        let ttl_days = std::env::var("AETHER_ECONOMY_IDEMPOTENCY_TTL_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_IDEMPOTENCY_TTL_DAYS);

        let platform_fee_bps = std::env::var("AETHER_ECONOMY_PLATFORM_FEE_BPS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_PLATFORM_FEE_BPS);

        Self {
            wallets: WalletManager::new(),
            ledger: CurrencyLedger::new(),
            idempotency: IdempotencyGuard::new(ttl_days),
            fraud: FraudDetector::from_env(),
            settlement: SettlementProcessor::new("default"),
            platform_fee_bps,
        }
    }

    /// Creates the service with explicit configuration (for testing).
    pub fn with_config(
        idempotency_ttl_days: u16,
        max_tx_per_minute: u32,
        anomaly_amount_threshold: i128,
        fraud_block_threshold: f32,
        platform_fee_bps: u64,
    ) -> Self {
        Self {
            wallets: WalletManager::new(),
            ledger: CurrencyLedger::new(),
            idempotency: IdempotencyGuard::new(idempotency_ttl_days),
            fraud: FraudDetector::with_config(
                max_tx_per_minute,
                anomaly_amount_threshold,
                fraud_block_threshold,
            ),
            settlement: SettlementProcessor::new("default"),
            platform_fee_bps,
        }
    }

    // ── Wallet operations ──────────────────────────────────────────

    pub fn create_wallet(&mut self, wallet_id: &str, owner: u64) -> ServiceResult<()> {
        self.wallets.create_wallet(wallet_id, owner)?;
        Ok(())
    }

    pub fn get_wallet(&self, wallet_id: &str) -> Option<&WalletAccount> {
        self.wallets.get(wallet_id)
    }

    pub fn freeze_wallet(&mut self, wallet_id: &str) -> ServiceResult<()> {
        self.wallets.freeze(wallet_id)?;
        Ok(())
    }

    pub fn unfreeze_wallet(&mut self, wallet_id: &str) -> ServiceResult<()> {
        self.wallets.unfreeze(wallet_id)?;
        Ok(())
    }

    pub fn get_balance(&self, wallet_id: &str) -> ServiceResult<i128> {
        Ok(self.wallets.balance(wallet_id)?)
    }

    /// Deposits funds into a wallet (used for initial funding / top-up).
    pub fn deposit(&mut self, wallet_id: &str, amount: i128) -> ServiceResult<()> {
        if amount <= 0 {
            return Err(ServiceError::InvalidAmount);
        }
        let wallet = self
            .wallets
            .get_mut(wallet_id)
            .ok_or(ServiceError::WalletError(WalletError::NotFound))?;
        if wallet.is_frozen() {
            return Err(ServiceError::WalletFrozen(wallet_id.to_string()));
        }
        wallet.apply(&WalletOperation::Deposit(amount));
        Ok(())
    }

    // ── Transaction processing ─────────────────────────────────────

    /// Processes a transaction (purchase, tip, trade, reward) atomically.
    ///
    /// Returns the journal_id on success, or an idempotent replay of the
    /// original journal_id if the tx_id was already processed.
    pub fn process_transaction(&mut self, req: &TransactionRequest) -> ServiceResult<u64> {
        // 1. Idempotency check
        if let Some(journal_id) = self.idempotency.check(&req.tx_id) {
            return Ok(journal_id);
        }

        if req.amount_minor <= 0 {
            return Err(ServiceError::InvalidAmount);
        }

        // 2. Fraud check (keyed on the sender)
        let sender_owner = self
            .wallets
            .get(&req.from_wallet)
            .ok_or(ServiceError::WalletError(WalletError::NotFound))?
            .owner;
        let signal = self
            .fraud
            .evaluate(sender_owner, req.amount_minor, req.timestamp_ms);
        if self.fraud.should_block(&signal) {
            let reason = signal
                .score
                .reason
                .unwrap_or_else(|| "fraud score exceeded threshold".to_string());
            return Err(ServiceError::FraudBlocked(reason));
        }

        // 3. Check frozen status
        if self
            .wallets
            .get(&req.from_wallet)
            .map_or(false, |w| w.is_frozen())
        {
            return Err(ServiceError::WalletFrozen(req.from_wallet.clone()));
        }
        if self
            .wallets
            .get(&req.to_wallet)
            .map_or(false, |w| w.is_frozen())
        {
            return Err(ServiceError::WalletFrozen(req.to_wallet.clone()));
        }

        // 4. Debit sender (overdraft prevention is inside WalletAccount::apply)
        let from_wallet = self
            .wallets
            .get_mut(&req.from_wallet)
            .ok_or(ServiceError::WalletError(WalletError::NotFound))?;
        if !from_wallet.apply(&WalletOperation::Withdraw(req.amount_minor)) {
            return Err(ServiceError::InsufficientFunds);
        }
        let balance_after_debit = from_wallet.balance_minor;

        // 5. Credit receiver
        let to_wallet = self
            .wallets
            .get_mut(&req.to_wallet)
            .ok_or(ServiceError::WalletError(WalletError::NotFound))?;
        to_wallet.apply(&WalletOperation::Deposit(req.amount_minor));
        let balance_after_credit = to_wallet.balance_minor;

        // 6. Post double-entry to ledger
        let debit_entry = LedgerEntry {
            wallet_id: req.from_wallet.clone(),
            tx_id: req.tx_id.clone(),
            kind: LedgerKind::Debit,
            currency: "AEC".to_string(),
            amount_minor: req.amount_minor,
            created_ms: req.timestamp_ms,
            memo: req.memo.clone(),
        };
        let credit_entry = LedgerEntry {
            wallet_id: req.to_wallet.clone(),
            tx_id: req.tx_id.clone(),
            kind: LedgerKind::Credit,
            currency: "AEC".to_string(),
            amount_minor: req.amount_minor,
            created_ms: req.timestamp_ms,
            memo: req.memo.clone(),
        };
        let journal_id = self
            .ledger
            .post_double_entry(debit_entry, credit_entry, balance_after_debit, balance_after_credit)
            .expect("balanced entry should always succeed");

        // 7. Record idempotency
        self.idempotency
            .record(&req.tx_id, journal_id, req.timestamp_ms);

        Ok(journal_id)
    }

    // ── Settlement ─────────────────────────────────────────────────

    pub fn enqueue_payout(&mut self, record: PayoutRecord) {
        self.settlement.enqueue(record);
    }

    pub fn process_next_payout(&mut self) -> Option<String> {
        self.settlement.process_next()
    }

    pub fn complete_payout(&mut self, payout_id: &str) -> Result<(), crate::settlement::SettlementError> {
        self.settlement.complete(payout_id)
    }

    pub fn fail_payout(&mut self, payout_id: &str) -> Result<(), crate::settlement::SettlementError> {
        self.settlement.fail(payout_id)
    }

    // ── Accessors (for testing / inspection) ───────────────────────

    pub fn ledger(&self) -> &CurrencyLedger {
        &self.ledger
    }

    pub fn platform_fee_bps(&self) -> u64 {
        self.platform_fee_bps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlement::{PayoutDestination, SettlementState};
    use crate::transaction::TransactionDirection;

    /// Helper to create a service with relaxed fraud limits for most tests.
    fn test_service() -> EconomyService {
        EconomyService::with_config(
            30,        // idempotency TTL days
            1000,      // max tx per minute (high so fraud doesn't interfere)
            i128::MAX, // anomaly threshold (effectively disabled)
            0.99,      // block threshold (very high)
            250,       // platform fee bps
        )
    }

    fn fund_wallet(svc: &mut EconomyService, wallet_id: &str, owner: u64, amount: i128) {
        svc.create_wallet(wallet_id, owner).unwrap();
        if amount > 0 {
            svc.deposit(wallet_id, amount).unwrap();
        }
    }

    fn make_request(
        tx_id: &str,
        from: &str,
        to: &str,
        amount: i128,
        direction: TransactionDirection,
    ) -> TransactionRequest {
        TransactionRequest {
            tx_id: tx_id.to_string(),
            from_wallet: from.to_string(),
            to_wallet: to.to_string(),
            amount_minor: amount,
            direction,
            memo: None,
            timestamp_ms: 1000,
        }
    }

    // ── Wallet lifecycle ───────────────────────────────────────────

    #[test]
    fn create_and_query_wallet() {
        let mut svc = test_service();
        svc.create_wallet("w1", 1).unwrap();
        assert_eq!(svc.get_balance("w1").unwrap(), 0);
        assert!(svc.get_wallet("w1").is_some());
    }

    #[test]
    fn deposit_increases_balance() {
        let mut svc = test_service();
        svc.create_wallet("w1", 1).unwrap();
        svc.deposit("w1", 5000).unwrap();
        assert_eq!(svc.get_balance("w1").unwrap(), 5000);
    }

    #[test]
    fn deposit_zero_or_negative_fails() {
        let mut svc = test_service();
        svc.create_wallet("w1", 1).unwrap();
        assert_eq!(svc.deposit("w1", 0).unwrap_err(), ServiceError::InvalidAmount);
        assert_eq!(svc.deposit("w1", -10).unwrap_err(), ServiceError::InvalidAmount);
    }

    #[test]
    fn freeze_and_unfreeze_wallet() {
        let mut svc = test_service();
        fund_wallet(&mut svc, "w1", 1, 1000);
        svc.freeze_wallet("w1").unwrap();
        assert!(svc.get_wallet("w1").unwrap().is_frozen());
        svc.unfreeze_wallet("w1").unwrap();
        assert!(!svc.get_wallet("w1").unwrap().is_frozen());
    }

    // ── Transaction processing ─────────────────────────────────────

    #[test]
    fn purchase_transfers_funds() {
        let mut svc = test_service();
        fund_wallet(&mut svc, "buyer", 1, 1000);
        fund_wallet(&mut svc, "seller", 2, 0);

        let req = make_request("tx-1", "buyer", "seller", 400, TransactionDirection::Purchase);
        let jid = svc.process_transaction(&req).unwrap();
        assert!(jid > 0);
        assert_eq!(svc.get_balance("buyer").unwrap(), 600);
        assert_eq!(svc.get_balance("seller").unwrap(), 400);
    }

    #[test]
    fn tip_transfers_funds() {
        let mut svc = test_service();
        fund_wallet(&mut svc, "tipper", 1, 500);
        fund_wallet(&mut svc, "creator", 2, 100);

        let req = make_request("tx-tip", "tipper", "creator", 50, TransactionDirection::Tip);
        svc.process_transaction(&req).unwrap();
        assert_eq!(svc.get_balance("tipper").unwrap(), 450);
        assert_eq!(svc.get_balance("creator").unwrap(), 150);
    }

    #[test]
    fn trade_transfers_funds() {
        let mut svc = test_service();
        fund_wallet(&mut svc, "alice", 1, 300);
        fund_wallet(&mut svc, "bob", 2, 200);

        let req = make_request("tx-trade", "alice", "bob", 100, TransactionDirection::Trade);
        svc.process_transaction(&req).unwrap();
        assert_eq!(svc.get_balance("alice").unwrap(), 200);
        assert_eq!(svc.get_balance("bob").unwrap(), 300);
    }

    // ── Double-entry invariant ─────────────────────────────────────

    #[test]
    fn ledger_balanced_after_transactions() {
        let mut svc = test_service();
        fund_wallet(&mut svc, "a", 1, 10000);
        fund_wallet(&mut svc, "b", 2, 10000);

        for i in 0..10 {
            let req = make_request(
                &format!("tx-{i}"),
                "a",
                "b",
                100,
                TransactionDirection::Purchase,
            );
            svc.process_transaction(&req).unwrap();
        }
        assert!(svc.ledger().verify_balance_invariant());
        assert_eq!(svc.ledger().len(), 10);
    }

    // ── Overdraft prevention ───────────────────────────────────────

    #[test]
    fn overdraft_prevented() {
        let mut svc = test_service();
        fund_wallet(&mut svc, "poor", 1, 50);
        fund_wallet(&mut svc, "rich", 2, 0);

        let req = make_request("tx-big", "poor", "rich", 100, TransactionDirection::Purchase);
        let err = svc.process_transaction(&req).unwrap_err();
        assert_eq!(err, ServiceError::InsufficientFunds);
        // Balance unchanged
        assert_eq!(svc.get_balance("poor").unwrap(), 50);
        assert_eq!(svc.get_balance("rich").unwrap(), 0);
    }

    // ── Idempotency ────────────────────────────────────────────────

    #[test]
    fn idempotent_replay_returns_same_journal_id() {
        let mut svc = test_service();
        fund_wallet(&mut svc, "a", 1, 1000);
        fund_wallet(&mut svc, "b", 2, 0);

        let req = make_request("tx-idem", "a", "b", 200, TransactionDirection::Purchase);
        let jid1 = svc.process_transaction(&req).unwrap();
        let jid2 = svc.process_transaction(&req).unwrap();
        assert_eq!(jid1, jid2);
        // Balance only debited once
        assert_eq!(svc.get_balance("a").unwrap(), 800);
        assert_eq!(svc.get_balance("b").unwrap(), 200);
    }

    // ── Fraud detection ────────────────────────────────────────────

    #[test]
    fn fraud_blocks_velocity_violation() {
        // max 2 tx per minute
        let mut svc = EconomyService::with_config(30, 2, i128::MAX, 0.4, 250);
        fund_wallet(&mut svc, "spammer", 1, 100_000);
        fund_wallet(&mut svc, "target", 2, 0);

        // First two succeed
        for i in 0..2 {
            let req = make_request(
                &format!("tx-v{i}"),
                "spammer",
                "target",
                10,
                TransactionDirection::Purchase,
            );
            svc.process_transaction(&req).unwrap();
        }

        // Third triggers velocity → 3 > 2, score=0.5 >= 0.4
        let req = make_request("tx-v2", "spammer", "target", 10, TransactionDirection::Purchase);
        let err = svc.process_transaction(&req).unwrap_err();
        assert!(matches!(err, ServiceError::FraudBlocked(_)));
    }

    #[test]
    fn fraud_blocks_anomaly_amount() {
        // anomaly threshold = 100
        let mut svc = EconomyService::with_config(30, 1000, 100, 0.3, 250);
        fund_wallet(&mut svc, "whale", 1, 1_000_000);
        fund_wallet(&mut svc, "target", 2, 0);

        let req = make_request(
            "tx-whale",
            "whale",
            "target",
            500,
            TransactionDirection::Purchase,
        );
        let err = svc.process_transaction(&req).unwrap_err();
        assert!(matches!(err, ServiceError::FraudBlocked(_)));
    }

    // ── Frozen wallet ──────────────────────────────────────────────

    #[test]
    fn frozen_sender_wallet_rejected() {
        let mut svc = test_service();
        fund_wallet(&mut svc, "frozen-sender", 1, 1000);
        fund_wallet(&mut svc, "receiver", 2, 0);
        svc.freeze_wallet("frozen-sender").unwrap();

        let req = make_request(
            "tx-frozen",
            "frozen-sender",
            "receiver",
            100,
            TransactionDirection::Purchase,
        );
        let err = svc.process_transaction(&req).unwrap_err();
        assert_eq!(
            err,
            ServiceError::WalletFrozen("frozen-sender".to_string())
        );
    }

    #[test]
    fn frozen_receiver_wallet_rejected() {
        let mut svc = test_service();
        fund_wallet(&mut svc, "sender", 1, 1000);
        fund_wallet(&mut svc, "frozen-recv", 2, 0);
        svc.freeze_wallet("frozen-recv").unwrap();

        let req = make_request(
            "tx-frozen2",
            "sender",
            "frozen-recv",
            100,
            TransactionDirection::Purchase,
        );
        let err = svc.process_transaction(&req).unwrap_err();
        assert_eq!(
            err,
            ServiceError::WalletFrozen("frozen-recv".to_string())
        );
    }

    #[test]
    fn deposit_to_frozen_wallet_rejected() {
        let mut svc = test_service();
        svc.create_wallet("w1", 1).unwrap();
        svc.freeze_wallet("w1").unwrap();
        let err = svc.deposit("w1", 100).unwrap_err();
        assert_eq!(err, ServiceError::WalletFrozen("w1".to_string()));
    }

    // ── Settlement ─────────────────────────────────────────────────

    #[test]
    fn settlement_lifecycle() {
        let mut svc = test_service();
        let payout = PayoutRecord {
            payout_id: "p1".to_string(),
            tx_id: "tx-p1".to_string(),
            destination: PayoutDestination::Wallet {
                wallet_id: "w-creator".to_string(),
            },
            amount_minor: 1000,
            fee_minor: 25,
            state: SettlementState::Queued,
        };
        svc.enqueue_payout(payout);
        let id = svc.process_next_payout().unwrap();
        assert_eq!(id, "p1");
        svc.complete_payout("p1").unwrap();
    }

    #[test]
    fn settlement_fail_path() {
        let mut svc = test_service();
        let payout = PayoutRecord {
            payout_id: "p-fail".to_string(),
            tx_id: "tx-pf".to_string(),
            destination: PayoutDestination::BankAccount {
                masked: "****1234".to_string(),
            },
            amount_minor: 500,
            fee_minor: 10,
            state: SettlementState::Queued,
        };
        svc.enqueue_payout(payout);
        svc.process_next_payout();
        svc.fail_payout("p-fail").unwrap();
    }

    // ── Invalid amount ─────────────────────────────────────────────

    #[test]
    fn zero_amount_transaction_rejected() {
        let mut svc = test_service();
        fund_wallet(&mut svc, "a", 1, 1000);
        fund_wallet(&mut svc, "b", 2, 0);
        let req = make_request("tx-zero", "a", "b", 0, TransactionDirection::Purchase);
        let err = svc.process_transaction(&req).unwrap_err();
        assert_eq!(err, ServiceError::InvalidAmount);
    }

    #[test]
    fn negative_amount_transaction_rejected() {
        let mut svc = test_service();
        fund_wallet(&mut svc, "a", 1, 1000);
        fund_wallet(&mut svc, "b", 2, 0);
        let req = make_request("tx-neg", "a", "b", -50, TransactionDirection::Purchase);
        let err = svc.process_transaction(&req).unwrap_err();
        assert_eq!(err, ServiceError::InvalidAmount);
    }

    // ── Nonexistent wallet ─────────────────────────────────────────

    #[test]
    fn transaction_with_missing_sender_fails() {
        let mut svc = test_service();
        fund_wallet(&mut svc, "b", 2, 0);
        let req = make_request("tx-no-from", "ghost", "b", 10, TransactionDirection::Purchase);
        let err = svc.process_transaction(&req).unwrap_err();
        assert!(matches!(err, ServiceError::WalletError(WalletError::NotFound)));
    }

    #[test]
    fn transaction_with_missing_receiver_fails() {
        let mut svc = test_service();
        fund_wallet(&mut svc, "a", 1, 1000);
        let req = make_request("tx-no-to", "a", "ghost", 10, TransactionDirection::Purchase);
        let err = svc.process_transaction(&req).unwrap_err();
        assert!(matches!(err, ServiceError::WalletError(WalletError::NotFound)));
    }

    // ── End-to-end: multiple operations ────────────────────────────

    #[test]
    fn end_to_end_purchase_and_tip_flow() {
        let mut svc = test_service();
        fund_wallet(&mut svc, "player", 1, 10_000);
        fund_wallet(&mut svc, "shop", 2, 0);
        fund_wallet(&mut svc, "creator", 3, 0);

        // Player buys item from shop
        let purchase = make_request(
            "tx-buy",
            "player",
            "shop",
            2500,
            TransactionDirection::Purchase,
        );
        svc.process_transaction(&purchase).unwrap();

        // Player tips creator
        let tip = make_request(
            "tx-tip",
            "player",
            "creator",
            500,
            TransactionDirection::Tip,
        );
        svc.process_transaction(&tip).unwrap();

        assert_eq!(svc.get_balance("player").unwrap(), 7000);
        assert_eq!(svc.get_balance("shop").unwrap(), 2500);
        assert_eq!(svc.get_balance("creator").unwrap(), 500);
        assert!(svc.ledger().verify_balance_invariant());
        assert_eq!(svc.ledger().len(), 2);
    }
}
