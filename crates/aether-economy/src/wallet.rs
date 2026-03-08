//! Wallet management: create, query, freeze/unfreeze, deposit/withdraw.

use std::collections::HashMap;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletStatus {
    Active,
    Frozen,
}

#[derive(Debug)]
pub struct WalletAccount {
    pub wallet_id: String,
    pub owner: u64,
    pub balance_minor: i128,
    pub constraint: WalletConstraint,
    pub status: WalletStatus,
}

impl WalletAccount {
    pub fn new(wallet_id: impl Into<String>, owner: u64) -> Self {
        Self {
            wallet_id: wallet_id.into(),
            owner,
            balance_minor: 0,
            constraint: WalletConstraint {
                min_balance_minor: 0,
            },
            status: WalletStatus::Active,
        }
    }

    pub fn is_frozen(&self) -> bool {
        self.status == WalletStatus::Frozen
    }

    pub fn apply(&mut self, operation: &WalletOperation) -> bool {
        if self.is_frozen() {
            return false;
        }
        match operation {
            WalletOperation::Deposit(amount) => {
                self.balance_minor = self.balance_minor.saturating_add(*amount);
                true
            }
            WalletOperation::Withdraw(amount) => {
                let after = self.balance_minor.saturating_sub(*amount);
                if after < self.constraint.min_balance_minor {
                    false
                } else {
                    self.balance_minor = after;
                    true
                }
            }
            WalletOperation::Hold(amount) => {
                let after = self.balance_minor.saturating_sub(*amount);
                if after < self.constraint.min_balance_minor {
                    false
                } else {
                    self.balance_minor = after;
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

/// Manages a collection of wallets keyed by wallet_id.
#[derive(Debug)]
pub struct WalletManager {
    wallets: HashMap<String, WalletAccount>,
}

impl WalletManager {
    pub fn new() -> Self {
        Self {
            wallets: HashMap::new(),
        }
    }

    /// Creates a new wallet. Returns `Err` if wallet_id already exists.
    pub fn create_wallet(
        &mut self,
        wallet_id: &str,
        owner: u64,
    ) -> Result<(), WalletError> {
        if self.wallets.contains_key(wallet_id) {
            return Err(WalletError::AlreadyExists);
        }
        self.wallets
            .insert(wallet_id.to_string(), WalletAccount::new(wallet_id, owner));
        Ok(())
    }

    pub fn get(&self, wallet_id: &str) -> Option<&WalletAccount> {
        self.wallets.get(wallet_id)
    }

    pub fn get_mut(&mut self, wallet_id: &str) -> Option<&mut WalletAccount> {
        self.wallets.get_mut(wallet_id)
    }

    pub fn freeze(&mut self, wallet_id: &str) -> Result<(), WalletError> {
        let wallet = self
            .wallets
            .get_mut(wallet_id)
            .ok_or(WalletError::NotFound)?;
        wallet.status = WalletStatus::Frozen;
        Ok(())
    }

    pub fn unfreeze(&mut self, wallet_id: &str) -> Result<(), WalletError> {
        let wallet = self
            .wallets
            .get_mut(wallet_id)
            .ok_or(WalletError::NotFound)?;
        wallet.status = WalletStatus::Active;
        Ok(())
    }

    pub fn balance(&self, wallet_id: &str) -> Result<i128, WalletError> {
        let wallet = self.wallets.get(wallet_id).ok_or(WalletError::NotFound)?;
        Ok(wallet.balance_minor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalletError {
    NotFound,
    AlreadyExists,
    Frozen,
    InsufficientFunds,
}

impl std::fmt::Display for WalletError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WalletError::NotFound => write!(f, "wallet not found"),
            WalletError::AlreadyExists => write!(f, "wallet already exists"),
            WalletError::Frozen => write!(f, "wallet is frozen"),
            WalletError::InsufficientFunds => write!(f, "insufficient funds"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_wallet_and_query_balance() {
        let mut mgr = WalletManager::new();
        mgr.create_wallet("w1", 42).unwrap();
        assert_eq!(mgr.balance("w1").unwrap(), 0);
    }

    #[test]
    fn create_duplicate_wallet_fails() {
        let mut mgr = WalletManager::new();
        mgr.create_wallet("w1", 1).unwrap();
        let err = mgr.create_wallet("w1", 2).unwrap_err();
        assert_eq!(err, WalletError::AlreadyExists);
    }

    #[test]
    fn deposit_and_withdraw() {
        let mut mgr = WalletManager::new();
        mgr.create_wallet("w1", 1).unwrap();
        let w = mgr.get_mut("w1").unwrap();
        assert!(w.apply(&WalletOperation::Deposit(1000)));
        assert_eq!(w.balance_minor, 1000);
        assert!(w.apply(&WalletOperation::Withdraw(400)));
        assert_eq!(w.balance_minor, 600);
    }

    #[test]
    fn overdraft_prevented() {
        let mut mgr = WalletManager::new();
        mgr.create_wallet("w1", 1).unwrap();
        let w = mgr.get_mut("w1").unwrap();
        w.apply(&WalletOperation::Deposit(100));
        assert!(!w.apply(&WalletOperation::Withdraw(200)));
        assert_eq!(w.balance_minor, 100); // unchanged
    }

    #[test]
    fn freeze_blocks_operations() {
        let mut mgr = WalletManager::new();
        mgr.create_wallet("w1", 1).unwrap();
        mgr.get_mut("w1")
            .unwrap()
            .apply(&WalletOperation::Deposit(500));
        mgr.freeze("w1").unwrap();
        let w = mgr.get_mut("w1").unwrap();
        assert!(!w.apply(&WalletOperation::Deposit(100)));
        assert!(!w.apply(&WalletOperation::Withdraw(100)));
        assert_eq!(w.balance_minor, 500); // unchanged
    }

    #[test]
    fn unfreeze_restores_operations() {
        let mut mgr = WalletManager::new();
        mgr.create_wallet("w1", 1).unwrap();
        mgr.get_mut("w1")
            .unwrap()
            .apply(&WalletOperation::Deposit(500));
        mgr.freeze("w1").unwrap();
        mgr.unfreeze("w1").unwrap();
        let w = mgr.get_mut("w1").unwrap();
        assert!(w.apply(&WalletOperation::Withdraw(100)));
        assert_eq!(w.balance_minor, 400);
    }

    #[test]
    fn query_nonexistent_wallet_returns_none() {
        let mgr = WalletManager::new();
        assert!(mgr.get("nonexistent").is_none());
        assert_eq!(
            mgr.balance("nonexistent").unwrap_err(),
            WalletError::NotFound
        );
    }

    #[test]
    fn freeze_nonexistent_wallet_fails() {
        let mut mgr = WalletManager::new();
        assert_eq!(
            mgr.freeze("ghost").unwrap_err(),
            WalletError::NotFound
        );
    }

    #[test]
    fn wallet_summary_reflects_balance() {
        let mut w = WalletAccount::new("w1", 1);
        w.apply(&WalletOperation::Deposit(999));
        let s = w.summary(12345);
        assert_eq!(s.available_minor, 999);
        assert_eq!(s.updated_ms, 12345);
    }

    #[test]
    fn hold_and_release() {
        let mut w = WalletAccount::new("w1", 1);
        w.apply(&WalletOperation::Deposit(1000));
        assert!(w.apply(&WalletOperation::Hold(300)));
        assert_eq!(w.balance_minor, 700);
        assert!(w.apply(&WalletOperation::Release(300)));
        assert_eq!(w.balance_minor, 1000);
    }

    #[test]
    fn hold_insufficient_funds() {
        let mut w = WalletAccount::new("w1", 1);
        w.apply(&WalletOperation::Deposit(100));
        assert!(!w.apply(&WalletOperation::Hold(200)));
        assert_eq!(w.balance_minor, 100);
    }
}
