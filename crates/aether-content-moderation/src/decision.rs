/// Auto-moderation decision engine with configurable rules and thresholds.
use crate::scanner::{AggregatedScanResult, FlagCategory, ModerationAction};
use crate::severity::ContentSeverity;

/// Minimum confidence threshold below which we always require human review.
const MIN_AUTO_CONFIDENCE: f32 = 0.5;

/// A rule that maps a flag category to a severity override.
#[derive(Debug, Clone)]
pub struct DecisionRule {
    /// The flag category this rule matches.
    pub category: FlagCategory,
    /// The minimum severity to apply when this category is flagged.
    pub min_severity: ContentSeverity,
    /// Whether to auto-reject when this rule matches.
    pub auto_reject: bool,
}

/// Configuration for the decision engine.
#[derive(Debug, Clone)]
pub struct DecisionConfig {
    /// Confidence threshold for auto-approve (content must be Clean + above this).
    pub auto_approve_confidence: f32,
    /// Confidence threshold for auto-reject (content must be Critical + above this).
    pub auto_reject_confidence: f32,
    /// Custom rules that override default behavior.
    pub rules: Vec<DecisionRule>,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            auto_approve_confidence: 0.9,
            auto_reject_confidence: 0.8,
            rules: Vec::new(),
        }
    }
}

/// The outcome of the decision engine evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Automatically approve the content.
    AutoApprove,
    /// Automatically reject the content.
    AutoReject { reason: String },
    /// Send to human review queue.
    SendToReview,
}

/// Decision engine that evaluates scan results and makes moderation decisions.
pub struct DecisionEngine {
    config: DecisionConfig,
}

impl DecisionEngine {
    pub fn new(config: DecisionConfig) -> Self {
        Self { config }
    }

    /// Evaluate an aggregated scan result and produce a decision.
    pub fn evaluate(&self, scan_result: &AggregatedScanResult) -> Decision {
        // Check custom rules first
        for rule in &self.config.rules {
            let matched = scan_result
                .all_flags
                .iter()
                .any(|f| f.category == rule.category);
            if matched && rule.auto_reject {
                return Decision::AutoReject {
                    reason: format!("rule match: {:?}", rule.category),
                };
            }
        }

        // Auto-reject critical content with sufficient confidence
        if scan_result.severity >= ContentSeverity::Critical
            && scan_result.max_confidence >= self.config.auto_reject_confidence
        {
            let reason = scan_result
                .auto_decision
                .as_ref()
                .and_then(|d| match d {
                    ModerationAction::Reject { reason } => Some(reason.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| "critical severity detected".to_string());
            return Decision::AutoReject { reason };
        }

        // Auto-approve clean content with sufficient confidence
        if scan_result.severity == ContentSeverity::Clean
            && scan_result.max_confidence >= self.config.auto_approve_confidence
        {
            return Decision::AutoApprove;
        }

        // Insufficient confidence always goes to review
        if scan_result.max_confidence < MIN_AUTO_CONFIDENCE {
            return Decision::SendToReview;
        }

        // Everything else goes to human review
        Decision::SendToReview
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{ContentFlag, ScanResult};

    fn make_clean_result(confidence: f32) -> AggregatedScanResult {
        AggregatedScanResult {
            severity: ContentSeverity::Clean,
            all_flags: Vec::new(),
            max_confidence: confidence,
            auto_decision: Some(ModerationAction::Approve),
            individual_results: vec![ScanResult {
                severity: ContentSeverity::Clean,
                flags: Vec::new(),
                confidence,
                auto_decision: Some(ModerationAction::Approve),
                scanner_name: "test".to_string(),
            }],
        }
    }

    fn make_critical_result(confidence: f32) -> AggregatedScanResult {
        AggregatedScanResult {
            severity: ContentSeverity::Critical,
            all_flags: vec![ContentFlag {
                label: "malware".to_string(),
                category: FlagCategory::Malware,
            }],
            max_confidence: confidence,
            auto_decision: Some(ModerationAction::Reject {
                reason: "malware detected".to_string(),
            }),
            individual_results: vec![ScanResult {
                severity: ContentSeverity::Critical,
                flags: vec![ContentFlag {
                    label: "malware".to_string(),
                    category: FlagCategory::Malware,
                }],
                confidence,
                auto_decision: Some(ModerationAction::Reject {
                    reason: "malware detected".to_string(),
                }),
                scanner_name: "test".to_string(),
            }],
        }
    }

    fn make_medium_result(confidence: f32) -> AggregatedScanResult {
        AggregatedScanResult {
            severity: ContentSeverity::Medium,
            all_flags: vec![ContentFlag {
                label: "suggestive".to_string(),
                category: FlagCategory::Nudity,
            }],
            max_confidence: confidence,
            auto_decision: None,
            individual_results: vec![ScanResult {
                severity: ContentSeverity::Medium,
                flags: vec![ContentFlag {
                    label: "suggestive".to_string(),
                    category: FlagCategory::Nudity,
                }],
                confidence,
                auto_decision: None,
                scanner_name: "test".to_string(),
            }],
        }
    }

    #[test]
    fn test_auto_approve_clean_high_confidence() {
        let engine = DecisionEngine::new(DecisionConfig::default());
        let result = make_clean_result(0.95);
        assert_eq!(engine.evaluate(&result), Decision::AutoApprove);
    }

    #[test]
    fn test_no_auto_approve_clean_low_confidence() {
        let engine = DecisionEngine::new(DecisionConfig::default());
        let result = make_clean_result(0.85);
        // Below 0.9 threshold, goes to review
        assert_eq!(engine.evaluate(&result), Decision::SendToReview);
    }

    #[test]
    fn test_auto_reject_critical_high_confidence() {
        let engine = DecisionEngine::new(DecisionConfig::default());
        let result = make_critical_result(0.95);
        match engine.evaluate(&result) {
            Decision::AutoReject { reason } => {
                assert_eq!(reason, "malware detected");
            }
            other => panic!("expected AutoReject, got {:?}", other),
        }
    }

    #[test]
    fn test_no_auto_reject_critical_low_confidence() {
        let engine = DecisionEngine::new(DecisionConfig::default());
        let result = make_critical_result(0.7);
        // Below 0.8 threshold, goes to review
        assert_eq!(engine.evaluate(&result), Decision::SendToReview);
    }

    #[test]
    fn test_medium_severity_always_review() {
        let engine = DecisionEngine::new(DecisionConfig::default());
        let result = make_medium_result(0.95);
        assert_eq!(engine.evaluate(&result), Decision::SendToReview);
    }

    #[test]
    fn test_very_low_confidence_always_review() {
        let engine = DecisionEngine::new(DecisionConfig::default());
        let result = make_clean_result(0.3);
        assert_eq!(engine.evaluate(&result), Decision::SendToReview);
    }

    #[test]
    fn test_custom_rule_auto_reject() {
        let config = DecisionConfig {
            rules: vec![DecisionRule {
                category: FlagCategory::Copyright,
                min_severity: ContentSeverity::High,
                auto_reject: true,
            }],
            ..DecisionConfig::default()
        };
        let engine = DecisionEngine::new(config);

        let result = AggregatedScanResult {
            severity: ContentSeverity::Low,
            all_flags: vec![ContentFlag {
                label: "copyright_match".to_string(),
                category: FlagCategory::Copyright,
            }],
            max_confidence: 0.6,
            auto_decision: None,
            individual_results: Vec::new(),
        };

        match engine.evaluate(&result) {
            Decision::AutoReject { reason } => {
                assert!(reason.contains("Copyright"));
            }
            other => panic!("expected AutoReject, got {:?}", other),
        }
    }

    #[test]
    fn test_custom_rule_no_match() {
        let config = DecisionConfig {
            rules: vec![DecisionRule {
                category: FlagCategory::Copyright,
                min_severity: ContentSeverity::High,
                auto_reject: true,
            }],
            ..DecisionConfig::default()
        };
        let engine = DecisionEngine::new(config);
        // This result has Violence flags, not Copyright
        let result = AggregatedScanResult {
            severity: ContentSeverity::Medium,
            all_flags: vec![ContentFlag {
                label: "violence".to_string(),
                category: FlagCategory::Violence,
            }],
            max_confidence: 0.8,
            auto_decision: None,
            individual_results: Vec::new(),
        };
        assert_eq!(engine.evaluate(&result), Decision::SendToReview);
    }

    #[test]
    fn test_custom_config_thresholds() {
        let config = DecisionConfig {
            auto_approve_confidence: 0.7,
            auto_reject_confidence: 0.6,
            rules: Vec::new(),
        };
        let engine = DecisionEngine::new(config);

        // Clean with 0.75 confidence - above new threshold
        let result = make_clean_result(0.75);
        assert_eq!(engine.evaluate(&result), Decision::AutoApprove);

        // Critical with 0.65 confidence - above new threshold
        let result = make_critical_result(0.65);
        match engine.evaluate(&result) {
            Decision::AutoReject { .. } => {}
            other => panic!("expected AutoReject, got {:?}", other),
        }
    }

    #[test]
    fn test_default_config() {
        let config = DecisionConfig::default();
        assert_eq!(config.auto_approve_confidence, 0.9);
        assert_eq!(config.auto_reject_confidence, 0.8);
        assert!(config.rules.is_empty());
    }

    #[test]
    fn test_critical_without_auto_decision_uses_default_reason() {
        let engine = DecisionEngine::new(DecisionConfig::default());
        let result = AggregatedScanResult {
            severity: ContentSeverity::Critical,
            all_flags: Vec::new(),
            max_confidence: 0.95,
            auto_decision: None,
            individual_results: Vec::new(),
        };
        match engine.evaluate(&result) {
            Decision::AutoReject { reason } => {
                assert_eq!(reason, "critical severity detected");
            }
            other => panic!("expected AutoReject, got {:?}", other),
        }
    }
}
