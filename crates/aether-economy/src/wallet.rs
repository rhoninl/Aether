#[derive(Debug, Clone)]
pub struct WalletConstraint {
    pub min_balance_minor: i128,
}

#[derive(Debug, Clone)]
pub struct WalletSummary {
    pub wallet_id: String,
    pub available_minor: i128,
    pub locked_minor: i128,
    pub updated_ms: u64,
}

#[derive(Debug)]
pub enum WalletOperation {
    Deposit(i128),
    Withdraw(i128),
    Hold(i128),
    Release(i128),
}

#[derive(Debug)]
pub struct WalletAccount {
    pub wallet_id: String,
    pub owner: u64,
    pub balance_minor: i128,
    pub constraint: WalletConstraint,
}

impl WalletAccount {
    pub fn new(wallet_id: impl Into<String>, owner: u64) -> Self {
        Self {
            wallet_id: wallet_id.into(),
            owner,
            balance_minor: 0,
            constraint: WalletConstraint { min_balance_minor: 0 },
        }
    }

    pub fn apply(&mut self, operation: &WalletOperation) -> bool {
        match operation {
            WalletOperation::Deposit(amount) => {
                self.balance_minor = self.balance_minor.saturating_add(*amount);
                true
            }
            WalletOperation::Withdraw(amount) => {
                if self.balance_minor.saturating_sub(*amount) < self.constraint.min_balance_minor {
                    false
                } else {
                    self.balance_minor = self.balance_minor.saturating_sub(*amount);
                    true
                }
            }
            WalletOperation::Hold(amount) => {
                if self.balance_minor.saturating_sub(*amount) < self.constraint.min_balance_minor {
                    false
                } else {
                    self.balance_minor = self.balance_minor.saturating_sub(*amount);
                    true
                }
            }
            WalletOperation::Release(amount) => {
                self.balance_minor = self.balance_minor.saturating_add(*amount);
                true
            }
        }
    }

    pub fn summary(&self, now_ms: u64) -> WalletSummary {
        WalletSummary {
            wallet_id: self.wallet_id.clone(),
            available_minor: self.balance_minor,
            locked_minor: 0,
            updated_ms: now_ms,
        }
    }
}

