#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisibilityMode {
    Visible,
    FriendsOnly,
    Invisible,
}

#[derive(Debug, Clone)]
pub struct VisibleScope {
    pub mode: VisibilityMode,
    pub include_friends: bool,
}
