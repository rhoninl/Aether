//! Visibility mode enforcement.
//!
//! Determines whether one user can see another based on their
//! visibility settings and friendship status.

#[derive(Debug, Clone, PartialEq)]
pub enum VisibilityMode {
    /// Visible to everyone.
    Visible,
    /// Visible only to friends.
    FriendsOnly,
    /// Invisible to everyone (except the user themselves).
    Invisible,
}

#[derive(Debug, Clone)]
pub struct VisibleScope {
    pub mode: VisibilityMode,
    pub include_friends: bool,
}

impl Default for VisibleScope {
    fn default() -> Self {
        Self {
            mode: VisibilityMode::Visible,
            include_friends: true,
        }
    }
}

/// Determine whether the observer can see the target.
///
/// Rules:
/// - If the target is `Invisible`, the observer cannot see them.
/// - If the target is `FriendsOnly`, the observer can only see them
///   if `are_friends` is true.
/// - If the target is `Visible`, the observer can always see them.
///
/// Note: the observer's own visibility mode does not affect what they
/// can see -- only the target's mode matters.
pub fn can_see(target: &VisibleScope, are_friends: bool) -> bool {
    match target.mode {
        VisibilityMode::Invisible => false,
        VisibilityMode::FriendsOnly => are_friends,
        VisibilityMode::Visible => true,
    }
}

/// Filter a list of targets, returning only those visible to the observer.
///
/// `targets` is a slice of `(user_id, visibility_scope, is_friend)` tuples.
pub fn filter_visible_targets(targets: &[(u64, VisibleScope, bool)]) -> Vec<u64> {
    targets
        .iter()
        .filter(|(_, scope, is_friend)| can_see(scope, *is_friend))
        .map(|(id, _, _)| *id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(mode: VisibilityMode) -> VisibleScope {
        VisibleScope {
            mode,
            include_friends: true,
        }
    }

    // --- can_see tests ---

    #[test]
    fn visible_target_seen_by_stranger() {
        assert!(can_see(&scope(VisibilityMode::Visible), false));
    }

    #[test]
    fn visible_target_seen_by_friend() {
        assert!(can_see(&scope(VisibilityMode::Visible), true));
    }

    #[test]
    fn friends_only_target_seen_by_friend() {
        assert!(can_see(&scope(VisibilityMode::FriendsOnly), true));
    }

    #[test]
    fn friends_only_target_hidden_from_stranger() {
        assert!(!can_see(&scope(VisibilityMode::FriendsOnly), false));
    }

    #[test]
    fn invisible_target_hidden_from_friend() {
        assert!(!can_see(&scope(VisibilityMode::Invisible), true));
    }

    #[test]
    fn invisible_target_hidden_from_stranger() {
        assert!(!can_see(&scope(VisibilityMode::Invisible), false));
    }

    // --- filter_visible_targets tests ---

    #[test]
    fn filter_visible_mixed_modes() {
        let targets = vec![
            (1, scope(VisibilityMode::Visible), false),
            (2, scope(VisibilityMode::FriendsOnly), false),
            (3, scope(VisibilityMode::FriendsOnly), true),
            (4, scope(VisibilityMode::Invisible), true),
            (5, scope(VisibilityMode::Visible), true),
        ];
        let visible = filter_visible_targets(&targets);
        assert_eq!(visible, vec![1, 3, 5]);
    }

    #[test]
    fn filter_visible_all_visible() {
        let targets = vec![
            (1, scope(VisibilityMode::Visible), false),
            (2, scope(VisibilityMode::Visible), true),
        ];
        let visible = filter_visible_targets(&targets);
        assert_eq!(visible, vec![1, 2]);
    }

    #[test]
    fn filter_visible_all_invisible() {
        let targets = vec![
            (1, scope(VisibilityMode::Invisible), true),
            (2, scope(VisibilityMode::Invisible), false),
        ];
        let visible = filter_visible_targets(&targets);
        assert!(visible.is_empty());
    }

    #[test]
    fn filter_visible_empty_input() {
        let targets: Vec<(u64, VisibleScope, bool)> = vec![];
        let visible = filter_visible_targets(&targets);
        assert!(visible.is_empty());
    }

    // --- Default tests ---

    #[test]
    fn default_scope_is_visible() {
        let s = VisibleScope::default();
        assert_eq!(s.mode, VisibilityMode::Visible);
        assert!(s.include_friends);
    }

    #[test]
    fn default_scope_visible_to_everyone() {
        let s = VisibleScope::default();
        assert!(can_see(&s, false));
        assert!(can_see(&s, true));
    }
}
