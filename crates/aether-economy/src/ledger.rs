#[derive(Debug, Clone, Copy)]
pub enum LedgerKind {
    Debit,
    Credit,
}

#[derive(Debug, Clone)]
pub struct LedgerEntry {
    pub world_id: String,
    pub actor_id: u64,
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

#[derive(Debug)]
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

    pub fn post_double_entry(&mut self, debit: LedgerEntry, credit: LedgerEntry) -> Result<u64, String> {
        self.next_journal_id = self.next_journal_id.saturating_add(1);
        let journal_id = self.next_journal_id;
        let debit_balance_after = i128::from(0);
        let credit_balance_after = i128::from(0);
        self.entries.push(LedgerRecord {
            journal_id,
            debit,
            credit,
            balance_after_debit: debit_balance_after,
            balance_after_credit: credit_balance_after,
        });
        Ok(journal_id)
    }
}

