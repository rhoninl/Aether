//! Parental controls: age gates, time limits, and content category filtering.

/// Threshold in minutes before expiry at which a warning is issued.
const WARNING_THRESHOLD_MINUTES: u32 = 5;

#[derive(Debug, Clone, PartialEq)]
pub enum ContentFilter {
    Off,
    Mild,
    Strict,
}

#[derive(Debug, Clone)]
pub struct TimeLimit {
    pub minutes_per_day: u32,
    pub hard_stop: bool,
}

#[derive(Debug, Clone)]
pub struct ParentalControl {
    pub enabled: bool,
    pub filter: ContentFilter,
    pub time_limit: Option<TimeLimit>,
    pub social_allowed: bool,
    /// Minimum age required for content access (None = no restriction).
    pub age_gate_minimum: Option<u8>,
    /// Categories that are explicitly blocked.
    pub blocked_categories: Vec<String>,
}

impl Default for ParentalControl {
    fn default() -> Self {
        Self {
            enabled: false,
            filter: ContentFilter::Off,
            time_limit: None,
            social_allowed: true,
            age_gate_minimum: None,
            blocked_categories: Vec::new(),
        }
    }
}

/// The result of a time limit check.
#[derive(Debug, Clone, PartialEq)]
pub enum TimeLimitStatus {
    /// The user has time remaining. `remaining` is minutes left.
    Allowed { remaining: u32 },
    /// The user is close to their limit. `remaining` is minutes left.
    Warning { remaining: u32 },
    /// The user has exhausted their daily limit.
    Expired,
}

/// Check whether a user passes the age gate for a world.
///
/// Returns `true` if:
/// - Parental controls are disabled, OR
/// - No age gate is set on the parental control, OR
/// - The world has no minimum age requirement, OR
/// - The user's age meets both the parental gate and the world minimum.
pub fn check_age_gate(
    control: &ParentalControl,
    world_min_age: Option<u8>,
    user_age: u8,
) -> bool {
    if !control.enabled {
        return true;
    }

    if let Some(gate) = control.age_gate_minimum {
        if user_age < gate {
            return false;
        }
    }

    if let Some(world_min) = world_min_age {
        if user_age < world_min {
            return false;
        }
    }

    true
}

/// Check the time limit status given how many minutes have been used today.
///
/// Returns `TimeLimitStatus::Allowed` if no limit is set or parental controls
/// are disabled.
pub fn check_time_remaining(control: &ParentalControl, minutes_used: u32) -> TimeLimitStatus {
    if !control.enabled {
        return TimeLimitStatus::Allowed {
            remaining: u32::MAX,
        };
    }

    let limit = match &control.time_limit {
        Some(l) => l,
        None => {
            return TimeLimitStatus::Allowed {
                remaining: u32::MAX,
            }
        }
    };

    if minutes_used >= limit.minutes_per_day {
        return TimeLimitStatus::Expired;
    }

    let remaining = limit.minutes_per_day - minutes_used;

    if remaining <= WARNING_THRESHOLD_MINUTES {
        TimeLimitStatus::Warning { remaining }
    } else {
        TimeLimitStatus::Allowed { remaining }
    }
}

/// Check whether a content category is allowed by the parental controls.
///
/// Returns `true` if parental controls are disabled or the category is
/// not in the blocked list. Comparison is case-insensitive.
pub fn is_category_allowed(control: &ParentalControl, category: &str) -> bool {
    if !control.enabled {
        return true;
    }

    let lower = category.to_lowercase();
    !control
        .blocked_categories
        .iter()
        .any(|c| c.to_lowercase() == lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled_control() -> ParentalControl {
        ParentalControl {
            enabled: true,
            ..Default::default()
        }
    }

    // --- Age gate tests ---

    #[test]
    fn age_gate_disabled_always_passes() {
        let control = ParentalControl::default(); // disabled
        assert!(check_age_gate(&control, Some(18), 10));
    }

    #[test]
    fn age_gate_no_restrictions_passes() {
        let control = enabled_control();
        assert!(check_age_gate(&control, None, 5));
    }

    #[test]
    fn age_gate_parental_minimum_passes() {
        let mut control = enabled_control();
        control.age_gate_minimum = Some(13);
        assert!(check_age_gate(&control, None, 13));
        assert!(check_age_gate(&control, None, 16));
    }

    #[test]
    fn age_gate_parental_minimum_fails() {
        let mut control = enabled_control();
        control.age_gate_minimum = Some(13);
        assert!(!check_age_gate(&control, None, 12));
    }

    #[test]
    fn age_gate_world_minimum_passes() {
        let control = enabled_control();
        assert!(check_age_gate(&control, Some(18), 18));
        assert!(check_age_gate(&control, Some(18), 25));
    }

    #[test]
    fn age_gate_world_minimum_fails() {
        let control = enabled_control();
        assert!(!check_age_gate(&control, Some(18), 17));
    }

    #[test]
    fn age_gate_both_minimums_strictest_wins() {
        let mut control = enabled_control();
        control.age_gate_minimum = Some(16);
        // World requires 18, user is 17 -> fails world check
        assert!(!check_age_gate(&control, Some(18), 17));
        // User is 16 -> passes parental but fails world
        assert!(!check_age_gate(&control, Some(18), 16));
        // User is 18 -> passes both
        assert!(check_age_gate(&control, Some(18), 18));
    }

    // --- Time limit tests ---

    #[test]
    fn time_limit_disabled_always_allowed() {
        let control = ParentalControl::default();
        let status = check_time_remaining(&control, 9999);
        assert_eq!(
            status,
            TimeLimitStatus::Allowed {
                remaining: u32::MAX
            }
        );
    }

    #[test]
    fn time_limit_no_limit_set_allowed() {
        let control = enabled_control();
        let status = check_time_remaining(&control, 100);
        assert_eq!(
            status,
            TimeLimitStatus::Allowed {
                remaining: u32::MAX
            }
        );
    }

    #[test]
    fn time_limit_under_limit_allowed() {
        let mut control = enabled_control();
        control.time_limit = Some(TimeLimit {
            minutes_per_day: 60,
            hard_stop: true,
        });
        let status = check_time_remaining(&control, 30);
        assert_eq!(status, TimeLimitStatus::Allowed { remaining: 30 });
    }

    #[test]
    fn time_limit_warning_zone() {
        let mut control = enabled_control();
        control.time_limit = Some(TimeLimit {
            minutes_per_day: 60,
            hard_stop: true,
        });
        let status = check_time_remaining(&control, 56);
        assert_eq!(status, TimeLimitStatus::Warning { remaining: 4 });
    }

    #[test]
    fn time_limit_exactly_at_warning_threshold() {
        let mut control = enabled_control();
        control.time_limit = Some(TimeLimit {
            minutes_per_day: 60,
            hard_stop: true,
        });
        let status = check_time_remaining(&control, 55);
        assert_eq!(status, TimeLimitStatus::Warning { remaining: 5 });
    }

    #[test]
    fn time_limit_expired() {
        let mut control = enabled_control();
        control.time_limit = Some(TimeLimit {
            minutes_per_day: 60,
            hard_stop: true,
        });
        let status = check_time_remaining(&control, 60);
        assert_eq!(status, TimeLimitStatus::Expired);
    }

    #[test]
    fn time_limit_over_limit_still_expired() {
        let mut control = enabled_control();
        control.time_limit = Some(TimeLimit {
            minutes_per_day: 60,
            hard_stop: true,
        });
        let status = check_time_remaining(&control, 120);
        assert_eq!(status, TimeLimitStatus::Expired);
    }

    #[test]
    fn time_limit_zero_used() {
        let mut control = enabled_control();
        control.time_limit = Some(TimeLimit {
            minutes_per_day: 60,
            hard_stop: false,
        });
        let status = check_time_remaining(&control, 0);
        assert_eq!(status, TimeLimitStatus::Allowed { remaining: 60 });
    }

    // --- Category tests ---

    #[test]
    fn category_disabled_always_allowed() {
        let control = ParentalControl::default();
        assert!(is_category_allowed(&control, "violence"));
    }

    #[test]
    fn category_no_blocked_allowed() {
        let control = enabled_control();
        assert!(is_category_allowed(&control, "education"));
    }

    #[test]
    fn category_blocked_rejected() {
        let mut control = enabled_control();
        control.blocked_categories = vec!["violence".to_string(), "gambling".to_string()];
        assert!(!is_category_allowed(&control, "violence"));
        assert!(!is_category_allowed(&control, "gambling"));
    }

    #[test]
    fn category_not_in_blocked_allowed() {
        let mut control = enabled_control();
        control.blocked_categories = vec!["violence".to_string()];
        assert!(is_category_allowed(&control, "education"));
    }

    #[test]
    fn category_case_insensitive() {
        let mut control = enabled_control();
        control.blocked_categories = vec!["Violence".to_string()];
        assert!(!is_category_allowed(&control, "violence"));
        assert!(!is_category_allowed(&control, "VIOLENCE"));
    }

    #[test]
    fn default_parental_control() {
        let pc = ParentalControl::default();
        assert!(!pc.enabled);
        assert_eq!(pc.filter, ContentFilter::Off);
        assert!(pc.time_limit.is_none());
        assert!(pc.social_allowed);
        assert!(pc.age_gate_minimum.is_none());
        assert!(pc.blocked_categories.is_empty());
    }
}
