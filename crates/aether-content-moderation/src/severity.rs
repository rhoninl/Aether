/// Content severity levels and enforcement action mappings.

/// Severity classification for scanned content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentSeverity {
    /// No issues detected.
    Clean,
    /// Minor issues that do not require action.
    Low,
    /// Significant issues requiring review.
    Medium,
    /// Serious violations.
    High,
    /// Immediate action required.
    Critical,
}

/// Enforcement actions that can be taken based on severity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnforcementAction {
    /// No action needed.
    None,
    /// Issue a warning to the content creator.
    Warning,
    /// Remove the content.
    ContentRemoval,
    /// Temporarily ban the user for a given number of seconds.
    TemporaryBan { duration_secs: u64 },
    /// Permanently ban the user.
    PermanentBan,
}

/// Default temporary ban duration in seconds (24 hours).
const DEFAULT_TEMP_BAN_SECS: u64 = 86_400;

/// Returns the recommended enforcement action for a given severity.
pub fn recommended_action(severity: ContentSeverity) -> EnforcementAction {
    match severity {
        ContentSeverity::Clean => EnforcementAction::None,
        ContentSeverity::Low => EnforcementAction::Warning,
        ContentSeverity::Medium => EnforcementAction::ContentRemoval,
        ContentSeverity::High => EnforcementAction::TemporaryBan {
            duration_secs: DEFAULT_TEMP_BAN_SECS,
        },
        ContentSeverity::Critical => EnforcementAction::PermanentBan,
    }
}

impl ContentSeverity {
    /// Returns true if this severity requires immediate action.
    pub fn requires_immediate_action(&self) -> bool {
        matches!(self, ContentSeverity::High | ContentSeverity::Critical)
    }

    /// Returns true if this severity should skip human review.
    pub fn auto_actionable(&self) -> bool {
        matches!(self, ContentSeverity::Clean | ContentSeverity::Critical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_ordering() {
        assert!(ContentSeverity::Clean < ContentSeverity::Low);
        assert!(ContentSeverity::Low < ContentSeverity::Medium);
        assert!(ContentSeverity::Medium < ContentSeverity::High);
        assert!(ContentSeverity::High < ContentSeverity::Critical);
    }

    #[test]
    fn test_recommended_action_clean() {
        assert_eq!(
            recommended_action(ContentSeverity::Clean),
            EnforcementAction::None
        );
    }

    #[test]
    fn test_recommended_action_low() {
        assert_eq!(
            recommended_action(ContentSeverity::Low),
            EnforcementAction::Warning
        );
    }

    #[test]
    fn test_recommended_action_medium() {
        assert_eq!(
            recommended_action(ContentSeverity::Medium),
            EnforcementAction::ContentRemoval
        );
    }

    #[test]
    fn test_recommended_action_high() {
        assert_eq!(
            recommended_action(ContentSeverity::High),
            EnforcementAction::TemporaryBan {
                duration_secs: DEFAULT_TEMP_BAN_SECS
            }
        );
    }

    #[test]
    fn test_recommended_action_critical() {
        assert_eq!(
            recommended_action(ContentSeverity::Critical),
            EnforcementAction::PermanentBan
        );
    }

    #[test]
    fn test_requires_immediate_action() {
        assert!(!ContentSeverity::Clean.requires_immediate_action());
        assert!(!ContentSeverity::Low.requires_immediate_action());
        assert!(!ContentSeverity::Medium.requires_immediate_action());
        assert!(ContentSeverity::High.requires_immediate_action());
        assert!(ContentSeverity::Critical.requires_immediate_action());
    }

    #[test]
    fn test_auto_actionable() {
        assert!(ContentSeverity::Clean.auto_actionable());
        assert!(!ContentSeverity::Low.auto_actionable());
        assert!(!ContentSeverity::Medium.auto_actionable());
        assert!(!ContentSeverity::High.auto_actionable());
        assert!(ContentSeverity::Critical.auto_actionable());
    }
}
