//! Double-entry ledger: every transaction posts a debit and a matching credit.
//! The sum across all entries is always zero.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerKind {
    Debit,
    Credit,
}

#[derive(Debug, Clone)]
pub struct LedgerEntry {
    pub wallet_id: String,
    pub tx_id: String,
    pub kind: LedgerKind,
    pub currency: String,
    pub amount_minor: i128,
    pub created_ms: u64,
    pub memo: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LedgerRecord {
    pub journal_id: u64,
    pub debit: LedgerEntry,
    pub credit: LedgerEntry,
    pub balance_after_debit: i128,
    pub balance_after_credit: i128,
}

/// Append-only double-entry journal.
#[derive(Debug, Default)]
pub struct CurrencyLedger {
    next_journal_id: u64,
    pub entries: Vec<LedgerRecord>,
}

impl CurrencyLedger {
    pub fn new() -> Self {
        Self {
            next_journal_id: 0,
            entries: Vec::new(),
        }
    }

    /// Posts a balanced double-entry record.
    ///
    /// The caller must ensure the debit entry has kind `Debit` and the credit
    /// entry has kind `Credit`, and that their amounts match. Balance-after
    /// values are supplied by the caller (computed from the wallet state after
    /// mutation).
    pub fn post_double_entry(
        &mut self,
        debit: LedgerEntry,
        credit: LedgerEntry,
        balance_after_debit: i128,
        balance_after_credit: i128,
    ) -> Result<u64, LedgerError> {
        if debit.kind != LedgerKind::Debit {
            return Err(LedgerError::InvalidEntryKind);
        }
        if credit.kind != LedgerKind::Credit {
            return Err(LedgerError::InvalidEntryKind);
        }
        if debit.amount_minor != credit.amount_minor {
            return Err(LedgerError::UnbalancedEntry);
        }
        if debit.tx_id != credit.tx_id {
            return Err(LedgerError::MismatchedTxId);
        }

        self.next_journal_id = self.next_journal_id.saturating_add(1);
        let journal_id = self.next_journal_id;

        self.entries.push(LedgerRecord {
            journal_id,
            debit,
            credit,
            balance_after_debit,
            balance_after_credit,
        });

        Ok(journal_id)
    }

    /// Returns the total number of journal entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns all entries for a given wallet_id.
    pub fn entries_for_wallet(&self, wallet_id: &str) -> Vec<&LedgerRecord> {
        self.entries
            .iter()
            .filter(|r| r.debit.wallet_id == wallet_id || r.credit.wallet_id == wallet_id)
            .collect()
    }

    /// Verifies the fundamental invariant: for every record, the debit and
    /// credit amounts are equal (i.e. net movement across all wallets is zero).
    pub fn verify_balance_invariant(&self) -> bool {
        self.entries
            .iter()
            .all(|r| r.debit.amount_minor == r.credit.amount_minor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    InvalidEntryKind,
    UnbalancedEntry,
    MismatchedTxId,
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LedgerError::InvalidEntryKind => write!(f, "invalid entry kind"),
            LedgerError::UnbalancedEntry => write!(f, "debit and credit amounts do not match"),
            LedgerError::MismatchedTxId => write!(f, "debit and credit tx_ids do not match"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(wallet_id: &str, kind: LedgerKind, amount: i128, tx_id: &str) -> LedgerEntry {
        LedgerEntry {
            wallet_id: wallet_id.to_string(),
            tx_id: tx_id.to_string(),
            kind,
            currency: "AEC".to_string(),
            amount_minor: amount,
            created_ms: 1000,
            memo: None,
        }
    }

    #[test]
    fn post_valid_double_entry() {
        let mut ledger = CurrencyLedger::new();
        let debit = make_entry("w-sender", LedgerKind::Debit, 500, "tx-1");
        let credit = make_entry("w-receiver", LedgerKind::Credit, 500, "tx-1");
        let jid = ledger.post_double_entry(debit, credit, 500, 500).unwrap();
        assert_eq!(jid, 1);
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn double_entry_sum_is_zero() {
        let mut ledger = CurrencyLedger::new();
        for i in 0..5 {
            let tx_id = format!("tx-{i}");
            let debit = make_entry("w-a", LedgerKind::Debit, 100, &tx_id);
            let credit = make_entry("w-b", LedgerKind::Credit, 100, &tx_id);
            ledger.post_double_entry(debit, credit, 0, 0).unwrap();
        }
        assert!(ledger.verify_balance_invariant());
    }

    #[test]
    fn reject_unbalanced_entry() {
        let mut ledger = CurrencyLedger::new();
        let debit = make_entry("w-a", LedgerKind::Debit, 100, "tx-1");
        let credit = make_entry("w-b", LedgerKind::Credit, 200, "tx-1");
        let err = ledger.post_double_entry(debit, credit, 0, 0).unwrap_err();
        assert_eq!(err, LedgerError::UnbalancedEntry);
    }

    #[test]
    fn reject_wrong_kind() {
        let mut ledger = CurrencyLedger::new();
        // Both Credit
        let debit = make_entry("w-a", LedgerKind::Credit, 100, "tx-1");
        let credit = make_entry("w-b", LedgerKind::Credit, 100, "tx-1");
        let err = ledger.post_double_entry(debit, credit, 0, 0).unwrap_err();
        assert_eq!(err, LedgerError::InvalidEntryKind);
    }

    #[test]
    fn reject_mismatched_tx_id() {
        let mut ledger = CurrencyLedger::new();
        let debit = make_entry("w-a", LedgerKind::Debit, 100, "tx-1");
        let credit = make_entry("w-b", LedgerKind::Credit, 100, "tx-2");
        let err = ledger.post_double_entry(debit, credit, 0, 0).unwrap_err();
        assert_eq!(err, LedgerError::MismatchedTxId);
    }

    #[test]
    fn entries_for_wallet_filters_correctly() {
        let mut ledger = CurrencyLedger::new();
        let d1 = make_entry("w-a", LedgerKind::Debit, 100, "tx-1");
        let c1 = make_entry("w-b", LedgerKind::Credit, 100, "tx-1");
        ledger.post_double_entry(d1, c1, 0, 0).unwrap();

        let d2 = make_entry("w-c", LedgerKind::Debit, 200, "tx-2");
        let c2 = make_entry("w-b", LedgerKind::Credit, 200, "tx-2");
        ledger.post_double_entry(d2, c2, 0, 0).unwrap();

        assert_eq!(ledger.entries_for_wallet("w-a").len(), 1);
        assert_eq!(ledger.entries_for_wallet("w-b").len(), 2);
        assert_eq!(ledger.entries_for_wallet("w-c").len(), 1);
        assert_eq!(ledger.entries_for_wallet("w-z").len(), 0);
    }

    #[test]
    fn journal_ids_increment() {
        let mut ledger = CurrencyLedger::new();
        let d1 = make_entry("w-a", LedgerKind::Debit, 10, "tx-1");
        let c1 = make_entry("w-b", LedgerKind::Credit, 10, "tx-1");
        let j1 = ledger.post_double_entry(d1, c1, 0, 0).unwrap();

        let d2 = make_entry("w-a", LedgerKind::Debit, 20, "tx-2");
        let c2 = make_entry("w-b", LedgerKind::Credit, 20, "tx-2");
        let j2 = ledger.post_double_entry(d2, c2, 0, 0).unwrap();

        assert_eq!(j1, 1);
        assert_eq!(j2, 2);
    }

    #[test]
    fn empty_ledger() {
        let ledger = CurrencyLedger::new();
        assert!(ledger.is_empty());
        assert_eq!(ledger.len(), 0);
        assert!(ledger.verify_balance_invariant());
    }
}
