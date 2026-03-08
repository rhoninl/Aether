//! Settlement / payout lifecycle: Queued -> InFlight -> Completed | Failed.

#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Processes payouts through the settlement lifecycle.
#[derive(Debug)]
pub struct SettlementProcessor {
    pub stream_name: String,
    queued: Vec<PayoutRecord>,
    in_flight: Vec<PayoutRecord>,
    completed: Vec<PayoutRecord>,
    failed: Vec<PayoutRecord>,
}

impl SettlementProcessor {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            stream_name: name.into(),
            queued: Vec::new(),
            in_flight: Vec::new(),
            completed: Vec::new(),
            failed: Vec::new(),
        }
    }

    /// Enqueue a payout for processing. Sets state to Queued.
    pub fn enqueue(&mut self, mut record: PayoutRecord) {
        record.state = SettlementState::Queued;
        self.queued.push(record);
    }

    /// Moves the next queued payout to in-flight. Returns the payout_id if
    /// one was available.
    pub fn process_next(&mut self) -> Option<String> {
        if let Some(mut record) = self.queued.pop() {
            record.state = SettlementState::InFlight;
            let id = record.payout_id.clone();
            self.in_flight.push(record);
            Some(id)
        } else {
            None
        }
    }

    /// Marks an in-flight payout as completed.
    pub fn complete(&mut self, payout_id: &str) -> Result<(), SettlementError> {
        let idx = self
            .in_flight
            .iter()
            .position(|r| r.payout_id == payout_id)
            .ok_or(SettlementError::NotFound)?;
        let mut record = self.in_flight.swap_remove(idx);
        record.state = SettlementState::Completed;
        self.completed.push(record);
        Ok(())
    }

    /// Marks an in-flight payout as failed.
    pub fn fail(&mut self, payout_id: &str) -> Result<(), SettlementError> {
        let idx = self
            .in_flight
            .iter()
            .position(|r| r.payout_id == payout_id)
            .ok_or(SettlementError::NotFound)?;
        let mut record = self.in_flight.swap_remove(idx);
        record.state = SettlementState::Failed;
        self.failed.push(record);
        Ok(())
    }

    pub fn queued_count(&self) -> usize {
        self.queued.len()
    }

    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    pub fn failed_count(&self) -> usize {
        self.failed.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettlementError {
    NotFound,
}

impl std::fmt::Display for SettlementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettlementError::NotFound => write!(f, "payout not found"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_payout(id: &str, amount: i128) -> PayoutRecord {
        PayoutRecord {
            payout_id: id.to_string(),
            tx_id: format!("tx-{id}"),
            destination: PayoutDestination::Wallet {
                wallet_id: "w-creator".to_string(),
            },
            amount_minor: amount,
            fee_minor: 0,
            state: SettlementState::Queued,
        }
    }

    #[test]
    fn enqueue_and_process() {
        let mut proc = SettlementProcessor::new("test-stream");
        proc.enqueue(make_payout("p1", 1000));
        assert_eq!(proc.queued_count(), 1);
        assert_eq!(proc.in_flight_count(), 0);

        let id = proc.process_next().unwrap();
        assert_eq!(id, "p1");
        assert_eq!(proc.queued_count(), 0);
        assert_eq!(proc.in_flight_count(), 1);
    }

    #[test]
    fn complete_payout() {
        let mut proc = SettlementProcessor::new("test-stream");
        proc.enqueue(make_payout("p1", 1000));
        proc.process_next();
        proc.complete("p1").unwrap();
        assert_eq!(proc.in_flight_count(), 0);
        assert_eq!(proc.completed_count(), 1);
    }

    #[test]
    fn fail_payout() {
        let mut proc = SettlementProcessor::new("test-stream");
        proc.enqueue(make_payout("p1", 1000));
        proc.process_next();
        proc.fail("p1").unwrap();
        assert_eq!(proc.in_flight_count(), 0);
        assert_eq!(proc.failed_count(), 1);
    }

    #[test]
    fn complete_nonexistent_fails() {
        let mut proc = SettlementProcessor::new("test-stream");
        let err = proc.complete("ghost").unwrap_err();
        assert_eq!(err, SettlementError::NotFound);
    }

    #[test]
    fn fail_nonexistent_fails() {
        let mut proc = SettlementProcessor::new("test-stream");
        let err = proc.fail("ghost").unwrap_err();
        assert_eq!(err, SettlementError::NotFound);
    }

    #[test]
    fn process_empty_queue_returns_none() {
        let mut proc = SettlementProcessor::new("test-stream");
        assert!(proc.process_next().is_none());
    }

    #[test]
    fn multiple_payouts_lifecycle() {
        let mut proc = SettlementProcessor::new("test-stream");
        proc.enqueue(make_payout("p1", 100));
        proc.enqueue(make_payout("p2", 200));
        proc.enqueue(make_payout("p3", 300));

        // Process all three
        let id1 = proc.process_next().unwrap();
        let id2 = proc.process_next().unwrap();
        let id3 = proc.process_next().unwrap();

        // Complete 2, fail 1
        proc.complete(&id1).unwrap();
        proc.complete(&id2).unwrap();
        proc.fail(&id3).unwrap();

        assert_eq!(proc.queued_count(), 0);
        assert_eq!(proc.in_flight_count(), 0);
        assert_eq!(proc.completed_count(), 2);
        assert_eq!(proc.failed_count(), 1);
    }
}
