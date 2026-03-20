/// Content rating categories and decisions for age-appropriate classification.
use crate::severity::ContentSeverity;

/// Content rating categories for age-appropriate access control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RatingCategory {
    /// Suitable for all audiences.
    General,
    /// Suitable for teenagers (13+).
    Teens,
    /// Mature content (17+).
    Mature,
    /// Adult-only content (18+).
    Adult,
}

/// A rating decision for a specific content item.
#[derive(Debug, Clone)]
pub struct RatingDecision {
    /// Content that was rated.
    pub content_id: String,
    /// Assigned category.
    pub category: RatingCategory,
    /// Explanation for the rating.
    pub rationale: String,
}

impl RatingDecision {
    pub fn new(content_id: String, category: RatingCategory, rationale: String) -> Self {
        Self {
            content_id,
            category,
            rationale,
        }
    }
}

/// Map a content severity to a suggested rating category.
pub fn suggested_rating(severity: ContentSeverity) -> RatingCategory {
    match severity {
        ContentSeverity::Clean => RatingCategory::General,
        ContentSeverity::Low => RatingCategory::Teens,
        ContentSeverity::Medium => RatingCategory::Mature,
        ContentSeverity::High | ContentSeverity::Critical => RatingCategory::Adult,
    }
}

/// Returns true if the given rating is restricted (requires age verification).
pub fn is_restricted(category: RatingCategory) -> bool {
    matches!(category, RatingCategory::Mature | RatingCategory::Adult)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggested_rating_clean() {
        assert_eq!(
            suggested_rating(ContentSeverity::Clean),
            RatingCategory::General
        );
    }

    #[test]
    fn test_suggested_rating_low() {
        assert_eq!(
            suggested_rating(ContentSeverity::Low),
            RatingCategory::Teens
        );
    }

    #[test]
    fn test_suggested_rating_medium() {
        assert_eq!(
            suggested_rating(ContentSeverity::Medium),
            RatingCategory::Mature
        );
    }

    #[test]
    fn test_suggested_rating_high() {
        assert_eq!(
            suggested_rating(ContentSeverity::High),
            RatingCategory::Adult
        );
    }

    #[test]
    fn test_suggested_rating_critical() {
        assert_eq!(
            suggested_rating(ContentSeverity::Critical),
            RatingCategory::Adult
        );
    }

    #[test]
    fn test_is_restricted() {
        assert!(!is_restricted(RatingCategory::General));
        assert!(!is_restricted(RatingCategory::Teens));
        assert!(is_restricted(RatingCategory::Mature));
        assert!(is_restricted(RatingCategory::Adult));
    }

    #[test]
    fn test_rating_decision_new() {
        let decision = RatingDecision::new(
            "content-1".to_string(),
            RatingCategory::Teens,
            "mild violence".to_string(),
        );
        assert_eq!(decision.content_id, "content-1");
        assert_eq!(decision.category, RatingCategory::Teens);
        assert_eq!(decision.rationale, "mild violence");
    }
}
