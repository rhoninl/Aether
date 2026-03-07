#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetentionState {
    Active,
    Frozen,
    Expired,
}

#[derive(Debug)]
pub struct RetentionWindow {
    pub years: u16,
    pub keep_legal_holds: bool,
    pub audit_retained: bool,
}

#[derive(Debug, Clone)]
pub struct RetentionRecord {
    pub table_name: String,
    pub row_id: String,
    pub until_ms: u64,
    pub state: RetentionState,
}
