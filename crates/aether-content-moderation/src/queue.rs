#[derive(Debug, Clone)]
pub enum ReviewState {
    Pending,
    InReview,
    Approved,
    Rejected,
}

#[derive(Debug, Clone)]
pub enum ReportPriority {
    Low,
    Medium,
    High,
    Urgent,
}

#[derive(Debug)]
pub struct ReportItem {
    pub report_id: String,
    pub artifact_id: String,
    pub submitter_user_id: u64,
    pub priority: ReportPriority,
    pub reason: String,
    pub created_ms: u64,
}

#[derive(Debug)]
pub enum ReviewAction {
    Escalate,
    Approve,
    Dismiss,
    Blacklist,
}

#[derive(Debug)]
pub struct ReviewQueue {
    pub items: Vec<ReportItem>,
}

