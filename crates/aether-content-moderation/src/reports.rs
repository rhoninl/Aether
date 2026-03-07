#[derive(Debug)]
pub enum ModerationSeverity {
    Informational,
    Warning,
    Critical,
}

#[derive(Debug)]
pub struct ModerationReport {
    pub report_id: String,
    pub artifact_id: String,
    pub details: String,
    pub severity: ModerationSeverity,
}

#[derive(Debug)]
pub struct ReportCase {
    pub case_id: String,
    pub report_id: String,
    pub escalated: bool,
}

