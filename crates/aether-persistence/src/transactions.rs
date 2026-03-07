use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriticalStatePriority {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone)]
pub enum CriticalStateKey {
    Economy,
    Inventory,
    Identity,
    ScriptState(String),
}

#[derive(Debug, Clone)]
pub enum CriticalStateError {
    MissingDependency(String),
    RpcUnavailable,
    Timeout,
    Conflict,
}

#[derive(Debug, Clone)]
pub struct SyncStateMutation {
    pub key: CriticalStateKey,
    pub payload: Vec<u8>,
    pub timestamp_ms: u64,
    pub actor_id: u64,
    pub priority: CriticalStatePriority,
}

#[derive(Debug)]
pub struct CriticalWriteResult {
    pub acknowledged: bool,
    pub elapsed: Duration,
    pub sequence: u64,
}

#[derive(Debug)]
pub struct CriticalStateMutationRecord {
    pub world_id: String,
    pub mutation_id: u64,
    pub mutation: SyncStateMutation,
}

#[derive(Debug, Default)]
pub struct SyncStateChannel {
    next_mutation: u64,
    pending: Vec<CriticalStateMutationRecord>,
    acknowledged: Vec<u64>,
}

impl SyncStateChannel {
    pub fn enqueue(&mut self, world_id: impl Into<String>, mutation: SyncStateMutation) -> CriticalStateMutationRecord {
        self.next_mutation = self.next_mutation.saturating_add(1);
        let record = CriticalStateMutationRecord {
            world_id: world_id.into(),
            mutation_id: self.next_mutation,
            mutation,
        };
        self.pending.push(record.clone());
        record
    }

    pub fn commit(&mut self, mutation_id: u64) -> CriticalWriteResult {
        self.pending.retain(|record| record.mutation_id != mutation_id);
        self.acknowledged.push(mutation_id);
        CriticalWriteResult {
            acknowledged: true,
            elapsed: Duration::from_millis(5),
            sequence: mutation_id,
        }
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

