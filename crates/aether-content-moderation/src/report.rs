/// User-submitted report handling with category classification and aggregation.
use uuid::Uuid;

/// Categories for user-submitted reports.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReportCategory {
    Harassment,
    Violence,
    Nudity,
    Spam,
    Copyright,
    HateSpeech,
    Other,
}

/// A user-submitted report against a piece of content.
#[derive(Debug, Clone)]
pub struct Report {
    /// Unique report identifier.
    pub report_id: Uuid,
    /// The content being reported.
    pub content_id: String,
    /// User who submitted the report.
    pub reporter_id: String,
    /// Category of the report.
    pub category: ReportCategory,
    /// Free-text description.
    pub description: String,
    /// Timestamp in milliseconds since epoch.
    pub created_at_ms: u64,
}

impl Report {
    pub fn new(
        content_id: String,
        reporter_id: String,
        category: ReportCategory,
        description: String,
        created_at_ms: u64,
    ) -> Self {
        Self {
            report_id: Uuid::new_v4(),
            content_id,
            reporter_id,
            category,
            description,
            created_at_ms,
        }
    }
}

/// Summary of aggregated reports for a single content item.
#[derive(Debug, Clone)]
pub struct ReportSummary {
    pub content_id: String,
    pub total_reports: usize,
    pub categories: std::collections::HashMap<ReportCategory, usize>,
    pub unique_reporters: usize,
    pub escalated: bool,
}

/// Default threshold for automatic escalation based on report count.
const DEFAULT_ESCALATION_THRESHOLD: usize = 5;

/// Aggregates reports by content and tracks escalation thresholds.
pub struct ReportAggregator {
    reports: Vec<Report>,
    escalation_threshold: usize,
}

impl ReportAggregator {
    pub fn new(escalation_threshold: usize) -> Self {
        Self {
            reports: Vec::new(),
            escalation_threshold,
        }
    }

    /// Submit a new report. Returns the report ID.
    pub fn submit_report(&mut self, report: Report) -> Uuid {
        let id = report.report_id;
        self.reports.push(report);
        id
    }

    /// Get all reports for a specific content item.
    pub fn reports_for_content(&self, content_id: &str) -> Vec<&Report> {
        self.reports
            .iter()
            .filter(|r| r.content_id == content_id)
            .collect()
    }

    /// Check whether the escalation threshold has been reached for a content item.
    pub fn check_threshold(&self, content_id: &str) -> bool {
        let count = self
            .reports
            .iter()
            .filter(|r| r.content_id == content_id)
            .count();
        count >= self.escalation_threshold
    }

    /// Generate a summary of reports for a content item.
    pub fn summarize(&self, content_id: &str) -> ReportSummary {
        let matching: Vec<&Report> = self.reports_for_content(content_id);

        let mut categories = std::collections::HashMap::new();
        let mut reporters = std::collections::HashSet::new();

        for report in &matching {
            *categories.entry(report.category.clone()).or_insert(0) += 1;
            reporters.insert(report.reporter_id.clone());
        }

        let total_reports = matching.len();
        let escalated = total_reports >= self.escalation_threshold;

        ReportSummary {
            content_id: content_id.to_string(),
            total_reports,
            categories,
            unique_reporters: reporters.len(),
            escalated,
        }
    }

    /// Returns the total number of reports stored.
    pub fn total_reports(&self) -> usize {
        self.reports.len()
    }
}

impl Default for ReportAggregator {
    fn default() -> Self {
        Self::new(DEFAULT_ESCALATION_THRESHOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_report(content_id: &str, reporter_id: &str, category: ReportCategory) -> Report {
        Report::new(
            content_id.to_string(),
            reporter_id.to_string(),
            category,
            "test report".to_string(),
            1000,
        )
    }

    #[test]
    fn test_submit_report() {
        let mut agg = ReportAggregator::new(3);
        let report = make_report("content-1", "user-1", ReportCategory::Spam);
        let id = agg.submit_report(report);
        assert_eq!(agg.total_reports(), 1);
        let reports = agg.reports_for_content("content-1");
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].report_id, id);
    }

    #[test]
    fn test_reports_for_different_content() {
        let mut agg = ReportAggregator::new(3);
        agg.submit_report(make_report("content-1", "user-1", ReportCategory::Spam));
        agg.submit_report(make_report("content-2", "user-2", ReportCategory::Violence));
        agg.submit_report(make_report("content-1", "user-3", ReportCategory::Nudity));

        assert_eq!(agg.reports_for_content("content-1").len(), 2);
        assert_eq!(agg.reports_for_content("content-2").len(), 1);
        assert_eq!(agg.reports_for_content("content-3").len(), 0);
    }

    #[test]
    fn test_threshold_not_reached() {
        let mut agg = ReportAggregator::new(3);
        agg.submit_report(make_report("content-1", "user-1", ReportCategory::Spam));
        agg.submit_report(make_report("content-1", "user-2", ReportCategory::Spam));
        assert!(!agg.check_threshold("content-1"));
    }

    #[test]
    fn test_threshold_reached() {
        let mut agg = ReportAggregator::new(3);
        agg.submit_report(make_report("content-1", "user-1", ReportCategory::Spam));
        agg.submit_report(make_report("content-1", "user-2", ReportCategory::Spam));
        agg.submit_report(make_report("content-1", "user-3", ReportCategory::Spam));
        assert!(agg.check_threshold("content-1"));
    }

    #[test]
    fn test_threshold_exceeded() {
        let mut agg = ReportAggregator::new(2);
        agg.submit_report(make_report("content-1", "user-1", ReportCategory::Spam));
        agg.submit_report(make_report("content-1", "user-2", ReportCategory::Spam));
        agg.submit_report(make_report("content-1", "user-3", ReportCategory::Spam));
        assert!(agg.check_threshold("content-1"));
    }

    #[test]
    fn test_threshold_different_content() {
        let mut agg = ReportAggregator::new(3);
        agg.submit_report(make_report("content-1", "user-1", ReportCategory::Spam));
        agg.submit_report(make_report("content-2", "user-2", ReportCategory::Spam));
        agg.submit_report(make_report("content-1", "user-3", ReportCategory::Spam));
        assert!(!agg.check_threshold("content-1"));
        assert!(!agg.check_threshold("content-2"));
    }

    #[test]
    fn test_summarize_empty() {
        let agg = ReportAggregator::new(3);
        let summary = agg.summarize("nonexistent");
        assert_eq!(summary.total_reports, 0);
        assert_eq!(summary.unique_reporters, 0);
        assert!(!summary.escalated);
        assert!(summary.categories.is_empty());
    }

    #[test]
    fn test_summarize_with_reports() {
        let mut agg = ReportAggregator::new(3);
        agg.submit_report(make_report("content-1", "user-1", ReportCategory::Spam));
        agg.submit_report(make_report("content-1", "user-2", ReportCategory::Spam));
        agg.submit_report(make_report("content-1", "user-3", ReportCategory::Violence));

        let summary = agg.summarize("content-1");
        assert_eq!(summary.total_reports, 3);
        assert_eq!(summary.unique_reporters, 3);
        assert!(summary.escalated); // 3 >= threshold of 3
        assert_eq!(summary.categories[&ReportCategory::Spam], 2);
        assert_eq!(summary.categories[&ReportCategory::Violence], 1);
    }

    #[test]
    fn test_summarize_duplicate_reporter() {
        let mut agg = ReportAggregator::new(5);
        agg.submit_report(make_report("content-1", "user-1", ReportCategory::Spam));
        agg.submit_report(make_report("content-1", "user-1", ReportCategory::Violence));

        let summary = agg.summarize("content-1");
        assert_eq!(summary.total_reports, 2);
        assert_eq!(summary.unique_reporters, 1); // same reporter
        assert!(!summary.escalated);
    }

    #[test]
    fn test_default_aggregator() {
        let agg = ReportAggregator::default();
        assert_eq!(agg.escalation_threshold, DEFAULT_ESCALATION_THRESHOLD);
        assert_eq!(agg.total_reports(), 0);
    }

    #[test]
    fn test_report_new_generates_uuid() {
        let r1 = Report::new(
            "c1".to_string(),
            "u1".to_string(),
            ReportCategory::Spam,
            "test".to_string(),
            1000,
        );
        let r2 = Report::new(
            "c1".to_string(),
            "u1".to_string(),
            ReportCategory::Spam,
            "test".to_string(),
            1000,
        );
        assert_ne!(r1.report_id, r2.report_id);
    }
}
