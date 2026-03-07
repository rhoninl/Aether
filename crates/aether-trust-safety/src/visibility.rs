#[derive(Debug)]
pub enum VisibilityMode {
    Visible,
    FriendsOnly,
    Invisible,
}

#[derive(Debug)]
pub struct VisibleScope {
    pub mode: VisibilityMode,
    pub include_friends: bool,
}

