#[derive(Debug, Clone)]
pub struct VelocityWindow {
    pub player_id: u64,
    pub last_ms: u64,
    pub tx_per_minute: u32,
    pub amount_last_minute_minor: i128,
}

#[derive(Debug, Clone)]
pub struct FraudScore {
    pub score: f32,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub enum FraudSignal {
    HighVelocity,
    RepeatedDeniedTx,
    OutlierAmount,
    GeofenceMismatch,
    NewAccountBurst,
}

#[derive(Debug, Clone)]
pub struct AnomalySignal {
    pub player_id: u64,
    pub signals: Vec<FraudSignal>,
    pub score: FraudScore,
}

impl AnomalySignal {
    pub fn is_block_candidate(&self, threshold: f32) -> bool {
        self.score.score >= threshold
    }
}
