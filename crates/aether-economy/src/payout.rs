#[derive(Debug, Clone)]
pub enum SettlementState {
    Queued,
    InFlight,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub enum PayoutDestination {
    Wallet { wallet_id: String },
    BankAccount { masked: String },
    ExternalLedger { address: String },
}

#[derive(Debug, Clone)]
pub struct PayoutRecord {
    pub payout_id: String,
    pub tx_id: String,
    pub destination: PayoutDestination,
    pub amount_minor: i128,
    pub fee_minor: i128,
    pub state: SettlementState,
}

#[derive(Debug)]
pub struct SettlementStream {
    pub stream_name: String,
    pub pending: Vec<PayoutRecord>,
}

impl SettlementStream {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            stream_name: name.into(),
            pending: Vec::new(),
        }
    }

    pub fn enqueue(&mut self, record: PayoutRecord) {
        self.pending.push(record);
    }
}

