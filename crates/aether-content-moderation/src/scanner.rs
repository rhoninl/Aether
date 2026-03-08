/// Content scanner trait and supporting types for automated content analysis.

use crate::severity::ContentSeverity;

/// Types of content that can be scanned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentType {
    Image,
    Mesh,
    Audio,
    Wasm,
}

/// A content item to be scanned.
#[derive(Debug, Clone)]
pub struct ContentItem {
    /// Unique identifier for this content.
    pub content_id: String,
    /// The type of content.
    pub content_type: ContentType,
    /// Raw content bytes.
    pub data: Vec<u8>,
    /// Optional metadata (e.g., file name, uploader ID).
    pub metadata: std::collections::HashMap<String, String>,
}

/// A flag raised during content scanning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentFlag {
    /// Human-readable label for this flag.
    pub label: String,
    /// Category of the flag.
    pub category: FlagCategory,
}

/// Categories for content flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlagCategory {
    Violence,
    Nudity,
    Harassment,
    Copyright,
    Malware,
    Spam,
    Other,
}

/// Moderation actions that can be recommended by a scanner.
#[derive(Debug, Clone, PartialEq)]
pub enum ModerationAction {
    /// Approve the content.
    Approve,
    /// Reject the content with a reason.
    Reject { reason: String },
    /// Request changes from the creator.
    RequireChanges { feedback: String },
    /// Escalate to senior moderator.
    Escalate,
}

/// Result of scanning a content item.
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// Assessed severity of the content.
    pub severity: ContentSeverity,
    /// List of flags raised.
    pub flags: Vec<ContentFlag>,
    /// Confidence score from 0.0 (no confidence) to 1.0 (certain).
    pub confidence: f32,
    /// Optional auto-decision if scanner is confident enough.
    pub auto_decision: Option<ModerationAction>,
    /// Name of the scanner that produced this result.
    pub scanner_name: String,
}

/// Trait for content scanners. Implementations can wrap ML models, rule engines, etc.
pub trait ContentScanner: Send + Sync {
    /// Scan a content item and return a result.
    fn scan(&self, content: &ContentItem) -> ScanResult;

    /// Returns the name of this scanner.
    fn scanner_name(&self) -> &str;
}

/// Aggregates results from multiple scanners and returns the worst-case severity.
pub struct ScannerPipeline {
    scanners: Vec<Box<dyn ContentScanner>>,
}

impl ScannerPipeline {
    pub fn new() -> Self {
        Self {
            scanners: Vec::new(),
        }
    }

    pub fn add_scanner(&mut self, scanner: Box<dyn ContentScanner>) {
        self.scanners.push(scanner);
    }

    /// Run all scanners and return aggregated results.
    /// The aggregated severity is the maximum across all scanner results.
    /// All flags are collected. Confidence is the maximum.
    /// Auto-decision uses the most severe scanner's recommendation.
    pub fn scan_all(&self, content: &ContentItem) -> AggregatedScanResult {
        let mut results = Vec::new();
        for scanner in &self.scanners {
            results.push(scanner.scan(content));
        }

        if results.is_empty() {
            return AggregatedScanResult {
                severity: ContentSeverity::Clean,
                all_flags: Vec::new(),
                max_confidence: 0.0,
                auto_decision: None,
                individual_results: results,
            };
        }

        let severity = results
            .iter()
            .map(|r| r.severity)
            .max()
            .unwrap_or(ContentSeverity::Clean);

        let all_flags: Vec<ContentFlag> = results.iter().flat_map(|r| r.flags.clone()).collect();

        let max_confidence = results
            .iter()
            .map(|r| r.confidence)
            .fold(0.0_f32, f32::max);

        // Use the auto-decision from the scanner with the highest severity
        let auto_decision = results
            .iter()
            .filter(|r| r.severity == severity)
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal))
            .and_then(|r| r.auto_decision.clone());

        AggregatedScanResult {
            severity,
            all_flags,
            max_confidence,
            auto_decision,
            individual_results: results,
        }
    }
}

impl Default for ScannerPipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Aggregated result from running multiple scanners.
#[derive(Debug)]
pub struct AggregatedScanResult {
    /// Worst-case severity across all scanners.
    pub severity: ContentSeverity,
    /// All flags from all scanners.
    pub all_flags: Vec<ContentFlag>,
    /// Maximum confidence across all scanners.
    pub max_confidence: f32,
    /// Auto-decision from the most severe/confident scanner.
    pub auto_decision: Option<ModerationAction>,
    /// Individual results from each scanner.
    pub individual_results: Vec<ScanResult>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mock scanner that always returns clean results.
    struct CleanScanner;

    impl ContentScanner for CleanScanner {
        fn scan(&self, _content: &ContentItem) -> ScanResult {
            ScanResult {
                severity: ContentSeverity::Clean,
                flags: Vec::new(),
                confidence: 0.95,
                auto_decision: Some(ModerationAction::Approve),
                scanner_name: "clean_scanner".to_string(),
            }
        }

        fn scanner_name(&self) -> &str {
            "clean_scanner"
        }
    }

    /// A mock scanner that always flags content as critical.
    struct CriticalScanner;

    impl ContentScanner for CriticalScanner {
        fn scan(&self, _content: &ContentItem) -> ScanResult {
            ScanResult {
                severity: ContentSeverity::Critical,
                flags: vec![ContentFlag {
                    label: "malware_detected".to_string(),
                    category: FlagCategory::Malware,
                }],
                confidence: 0.99,
                auto_decision: Some(ModerationAction::Reject {
                    reason: "malware detected".to_string(),
                }),
                scanner_name: "critical_scanner".to_string(),
            }
        }

        fn scanner_name(&self) -> &str {
            "critical_scanner"
        }
    }

    /// A mock scanner with medium severity and multiple flags.
    struct MediumScanner;

    impl ContentScanner for MediumScanner {
        fn scan(&self, _content: &ContentItem) -> ScanResult {
            ScanResult {
                severity: ContentSeverity::Medium,
                flags: vec![
                    ContentFlag {
                        label: "suggestive_content".to_string(),
                        category: FlagCategory::Nudity,
                    },
                    ContentFlag {
                        label: "violence_mild".to_string(),
                        category: FlagCategory::Violence,
                    },
                ],
                confidence: 0.7,
                auto_decision: None,
                scanner_name: "medium_scanner".to_string(),
            }
        }

        fn scanner_name(&self) -> &str {
            "medium_scanner"
        }
    }

    fn make_test_content() -> ContentItem {
        ContentItem {
            content_id: "test-001".to_string(),
            content_type: ContentType::Image,
            data: vec![0xFF, 0xD8, 0xFF],
            metadata: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_clean_scanner() {
        let scanner = CleanScanner;
        let content = make_test_content();
        let result = scanner.scan(&content);
        assert_eq!(result.severity, ContentSeverity::Clean);
        assert!(result.flags.is_empty());
        assert_eq!(result.confidence, 0.95);
        assert_eq!(result.auto_decision, Some(ModerationAction::Approve));
    }

    #[test]
    fn test_critical_scanner() {
        let scanner = CriticalScanner;
        let content = make_test_content();
        let result = scanner.scan(&content);
        assert_eq!(result.severity, ContentSeverity::Critical);
        assert_eq!(result.flags.len(), 1);
        assert_eq!(result.flags[0].category, FlagCategory::Malware);
    }

    #[test]
    fn test_pipeline_empty() {
        let pipeline = ScannerPipeline::new();
        let content = make_test_content();
        let result = pipeline.scan_all(&content);
        assert_eq!(result.severity, ContentSeverity::Clean);
        assert!(result.all_flags.is_empty());
        assert_eq!(result.max_confidence, 0.0);
        assert!(result.auto_decision.is_none());
    }

    #[test]
    fn test_pipeline_single_clean_scanner() {
        let mut pipeline = ScannerPipeline::new();
        pipeline.add_scanner(Box::new(CleanScanner));
        let content = make_test_content();
        let result = pipeline.scan_all(&content);
        assert_eq!(result.severity, ContentSeverity::Clean);
        assert!(result.all_flags.is_empty());
        assert_eq!(result.max_confidence, 0.95);
        assert_eq!(result.auto_decision, Some(ModerationAction::Approve));
        assert_eq!(result.individual_results.len(), 1);
    }

    #[test]
    fn test_pipeline_worst_case_severity() {
        let mut pipeline = ScannerPipeline::new();
        pipeline.add_scanner(Box::new(CleanScanner));
        pipeline.add_scanner(Box::new(CriticalScanner));
        let content = make_test_content();
        let result = pipeline.scan_all(&content);
        // Critical is worst-case
        assert_eq!(result.severity, ContentSeverity::Critical);
        assert_eq!(result.max_confidence, 0.99);
        assert_eq!(result.individual_results.len(), 2);
    }

    #[test]
    fn test_pipeline_collects_all_flags() {
        let mut pipeline = ScannerPipeline::new();
        pipeline.add_scanner(Box::new(CriticalScanner));
        pipeline.add_scanner(Box::new(MediumScanner));
        let content = make_test_content();
        let result = pipeline.scan_all(&content);
        // 1 from critical + 2 from medium = 3 total
        assert_eq!(result.all_flags.len(), 3);
    }

    #[test]
    fn test_pipeline_auto_decision_from_worst_scanner() {
        let mut pipeline = ScannerPipeline::new();
        pipeline.add_scanner(Box::new(CleanScanner));
        pipeline.add_scanner(Box::new(CriticalScanner));
        let content = make_test_content();
        let result = pipeline.scan_all(&content);
        // Should use the critical scanner's auto-decision (Reject)
        match &result.auto_decision {
            Some(ModerationAction::Reject { reason }) => {
                assert_eq!(reason, "malware detected");
            }
            other => panic!("expected Reject, got {:?}", other),
        }
    }

    #[test]
    fn test_pipeline_no_auto_decision_when_none() {
        let mut pipeline = ScannerPipeline::new();
        pipeline.add_scanner(Box::new(MediumScanner));
        let content = make_test_content();
        let result = pipeline.scan_all(&content);
        assert!(result.auto_decision.is_none());
    }

    #[test]
    fn test_content_item_metadata() {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("uploader".to_string(), "user-42".to_string());
        let content = ContentItem {
            content_id: "test-002".to_string(),
            content_type: ContentType::Mesh,
            data: vec![],
            metadata,
        };
        assert_eq!(content.metadata.get("uploader").unwrap(), "user-42");
    }

    #[test]
    fn test_scanner_name() {
        let scanner = CleanScanner;
        assert_eq!(scanner.scanner_name(), "clean_scanner");
    }

    #[test]
    fn test_default_pipeline() {
        let pipeline = ScannerPipeline::default();
        assert_eq!(pipeline.scanners.len(), 0);
    }
}
