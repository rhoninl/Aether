use crate::config::WorldPersistenceProfile;

pub type SnapshotId = u64;
pub type ActorId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotKind {
    Ephemeral,
    Durable,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub id: SnapshotId,
    pub world_id: String,
    pub tick: u64,
    pub captured_ms: u64,
    pub kind: SnapshotKind,
    pub actor_count: u32,
}

#[derive(Debug, Clone)]
pub struct SnapshotWindow {
    pub last_snapshot_ms: u64,
    pub interval_ms: u64,
}

impl SnapshotWindow {
    pub fn new(now_ms: u64, interval_ms: u64) -> Self {
        Self {
            last_snapshot_ms: now_ms,
            interval_ms,
        }
    }

    pub fn should_snapshot(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.last_snapshot_ms) >= self.interval_ms
    }

    pub fn next_deadline_ms(&self) -> u64 {
        self.last_snapshot_ms.saturating_add(self.interval_ms)
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotPolicy {
    pub min_interval_ms: u64,
    pub max_entity_sample_count: u32,
    pub always_persist_actor_transforms: bool,
}

impl SnapshotPolicy {
    pub fn new(profile: &WorldPersistenceProfile) -> Self {
        let min_interval_ms = u64::try_from(profile.snapshot_interval.as_millis()).unwrap_or(5_000);
        Self {
            min_interval_ms,
            max_entity_sample_count: 5_000,
            always_persist_actor_transforms: true,
        }
    }

    pub fn ephemeral_enabled(&self) -> bool {
        self.min_interval_ms > 0
    }
}

#[derive(Debug, Default)]
pub struct SnapshotRecorder {
    next_id: SnapshotId,
    last: Option<SnapshotWindow>,
    pub frames: Vec<Snapshot>,
}

impl SnapshotRecorder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next_id(&mut self) -> SnapshotId {
        self.next_id = self.next_id.saturating_add(1);
        self.next_id
    }

    pub fn push(
        &mut self,
        profile: &WorldPersistenceProfile,
        tick: u64,
        now_ms: u64,
        actor_count: u32,
    ) -> Option<Snapshot> {
        let policy = SnapshotPolicy::new(profile);

        // Check window timing: initialise the window if absent and test the deadline.
        let window = self
            .last
            .get_or_insert_with(|| SnapshotWindow::new(now_ms, policy.min_interval_ms));
        let should_snap = window.should_snapshot(now_ms);

        if !should_snap && actor_count == 0 {
            return None;
        }
        if actor_count > policy.max_entity_sample_count {
            return None;
        }

        // Drop the mutable borrow on `self.last` so `self.next_id()` can borrow `self`.
        let id = self.next_id();

        let snapshot = Snapshot {
            id,
            world_id: profile.world_id.clone(),
            tick,
            captured_ms: now_ms,
            kind: if profile.requires_stateful_storage() {
                SnapshotKind::Durable
            } else {
                SnapshotKind::Ephemeral
            },
            actor_count,
        };

        // Update the window timestamp after releasing the earlier borrow.
        if let Some(ref mut w) = self.last {
            w.last_snapshot_ms = now_ms;
        }

        self.frames.push(snapshot.clone());
        Some(snapshot)
    }

    pub fn prune_older_than_ms(&mut self, age_ms: u64, now_ms: u64) {
        self.frames
            .retain(|entry| now_ms.saturating_sub(entry.captured_ms) <= age_ms);
    }
}
