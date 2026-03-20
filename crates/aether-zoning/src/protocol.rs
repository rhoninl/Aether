pub type SequenceFence = u64;
pub type ZoneId = String;

#[derive(Debug, Clone)]
pub enum HandoffFailureMode {
    Timeout,
    AuthorityMismatch,
    ZoneGone,
    PlayerDisconnect,
}

#[derive(Debug, Clone)]
pub enum CrossZonePhysicsDecision {
    InitiatorAllowed,
    TargetValidated,
    TargetDenied(String),
}

#[derive(Debug, Clone)]
pub enum CrossZoneCombatDecision {
    TargetServerGrant,
    TargetServerReject,
    TargetServerTimedOut,
}

#[derive(Debug, Clone)]
pub struct HandoffDecision {
    pub player_id: u64,
    pub source_zone: ZoneId,
    pub target_zone: ZoneId,
    pub sequence: SequenceFence,
    pub expires_ms: u64,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone)]
pub enum HandoffResult {
    Accepted {
        player_id: u64,
        from: ZoneId,
        to: ZoneId,
        applied_sequence: SequenceFence,
    },
    Rejected {
        player_id: u64,
        reason: HandoffFailureMode,
    },
}

#[derive(Debug)]
pub struct HandoffEnvelope {
    pub player_id: u64,
    pub source_zone: ZoneId,
    pub target_zone: ZoneId,
    pub sequence: SequenceFence,
    pub timeout_ms: u64,
    pub started_ms: u64,
    pub snapshot_revision: u64,
    pub physics_decision: Option<CrossZonePhysicsDecision>,
    pub combat_decision: Option<CrossZoneCombatDecision>,
}

impl HandoffEnvelope {
    pub fn is_fenced(&self, expected: SequenceFence) -> bool {
        self.sequence >= expected
    }

    pub fn is_timeout(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.started_ms) > self.timeout_ms
    }

    pub fn mark_physics_validated(mut self, granted: bool) -> Self {
        self.physics_decision = Some(if granted {
            CrossZonePhysicsDecision::TargetValidated
        } else {
            CrossZonePhysicsDecision::TargetDenied("validation failed".to_string())
        });
        self
    }

    pub fn mark_combat(mut self, decision: CrossZoneCombatDecision) -> Self {
        self.combat_decision = Some(decision);
        self
    }
}
