#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalDurability {
    Ephemeral,
    FsyncBeforeAck,
}

#[derive(Debug, Clone)]
pub struct WalEntry {
    pub sequence: u64,
    pub world_id: String,
    pub key: String,
    pub payload_crc32: u32,
    pub timestamp_ms: u64,
    pub durable: WalDurability,
}

#[derive(Debug, Clone)]
pub struct WalSegment {
    pub id: u64,
    pub committed: bool,
    pub entries: Vec<WalEntry>,
}

#[derive(Debug)]
pub struct WalAppendResult {
    pub segment_id: u64,
    pub sequence: u64,
    pub requires_fsync: bool,
}

#[derive(Debug)]
pub enum WalAppendError {
    SegmentFull,
    InvalidPayload,
}

#[derive(Debug, Clone)]
pub struct WalReplayRecord {
    pub sequence: u64,
    pub key: String,
    pub payload_crc32: u32,
}

#[derive(Debug)]
pub struct WalWriteCoordinator {
    max_segment_entries: usize,
    next_segment_id: u64,
    next_sequence: u64,
    active: WalSegment,
    committed: Vec<WalSegment>,
}

impl WalWriteCoordinator {
    pub fn new(max_segment_entries: usize) -> Self {
        Self {
            max_segment_entries,
            next_segment_id: 1,
            next_sequence: 0,
            active: WalSegment {
                id: 0,
                committed: false,
                entries: Vec::new(),
            },
            committed: Vec::new(),
        }
    }

    pub fn append(
        &mut self,
        world_id: impl Into<String>,
        key: impl Into<String>,
        payload_crc32: u32,
        durable: WalDurability,
        timestamp_ms: u64,
    ) -> Result<WalAppendResult, WalAppendError> {
        if payload_crc32 == 0 {
            return Err(WalAppendError::InvalidPayload);
        }
        if self.active.entries.len() >= self.max_segment_entries {
            return Err(WalAppendError::SegmentFull);
        }
        self.next_sequence = self.next_sequence.saturating_add(1);
        if self.active.id == 0 {
            self.active.id = self.next_segment_id;
            self.next_segment_id = self.next_segment_id.saturating_add(1);
        }

        self.active.entries.push(WalEntry {
            sequence: self.next_sequence,
            world_id: world_id.into(),
            key: key.into(),
            payload_crc32,
            timestamp_ms,
            durable,
        });

        Ok(WalAppendResult {
            segment_id: self.active.id,
            sequence: self.next_sequence,
            requires_fsync: matches!(durable, WalDurability::FsyncBeforeAck),
        })
    }

    pub fn ack(&mut self, max_sequence: u64) -> usize {
        let keep = self
            .active
            .entries
            .iter()
            .position(|entry| entry.sequence > max_sequence);
        let cut = keep.unwrap_or(self.active.entries.len());
        let mut to_commit = self.active.entries.split_off(cut);
        if !to_commit.is_empty() {
            to_commit.reverse();
            let segment = WalSegment {
                id: self.active.id,
                committed: true,
                entries: to_commit,
            };
            self.committed.push(segment);
            if self.active.entries.is_empty() {
                self.active.id = 0;
            }
        }
        cut
    }

    pub fn replay(&self) -> Vec<WalReplayRecord> {
        self.committed
            .iter()
            .chain(std::iter::once(&self.active))
            .flat_map(|segment| segment.entries.iter())
            .filter_map(|entry| {
                if entry.durable == WalDurability::FsyncBeforeAck {
                    Some(WalReplayRecord {
                        sequence: entry.sequence,
                        key: entry.key.clone(),
                        payload_crc32: entry.payload_crc32,
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

