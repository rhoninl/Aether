/// Collision layer membership and filter bitmask.
///
/// A collider belongs to layers defined by `membership` bits and interacts with
/// layers defined by `filter` bits. Two colliders A and B collide only if
/// `(A.membership & B.filter) != 0 && (B.membership & A.filter) != 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionLayers {
    pub membership: u16,
    pub filter: u16,
}

pub const LAYER_DEFAULT: u16 = 1 << 0;
pub const LAYER_PLAYER: u16 = 1 << 1;
pub const LAYER_PROP: u16 = 1 << 2;
pub const LAYER_TERRAIN: u16 = 1 << 3;
pub const LAYER_TRIGGER: u16 = 1 << 4;
pub const LAYER_ALL: u16 = 0xFFFF;

impl Default for CollisionLayers {
    fn default() -> Self {
        Self {
            membership: LAYER_DEFAULT,
            filter: LAYER_ALL,
        }
    }
}

impl CollisionLayers {
    pub fn new(membership: u16, filter: u16) -> Self {
        Self { membership, filter }
    }

    /// Check if these layers can interact with another set of layers.
    pub fn interacts_with(&self, other: &CollisionLayers) -> bool {
        (self.membership & other.filter) != 0 && (other.membership & self.filter) != 0
    }

    pub fn player() -> Self {
        Self {
            membership: LAYER_PLAYER,
            filter: LAYER_ALL,
        }
    }

    pub fn terrain() -> Self {
        Self {
            membership: LAYER_TERRAIN,
            filter: LAYER_ALL,
        }
    }

    pub fn trigger() -> Self {
        Self {
            membership: LAYER_TRIGGER,
            filter: LAYER_PLAYER, // Triggers only interact with players by default
        }
    }

    pub fn prop() -> Self {
        Self {
            membership: LAYER_PROP,
            filter: LAYER_ALL,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layers_interact_with_everything() {
        let a = CollisionLayers::default();
        let b = CollisionLayers::default();
        assert!(a.interacts_with(&b));
    }

    #[test]
    fn same_layer_interacts() {
        let a = CollisionLayers::new(LAYER_PLAYER, LAYER_PLAYER);
        let b = CollisionLayers::new(LAYER_PLAYER, LAYER_PLAYER);
        assert!(a.interacts_with(&b));
    }

    #[test]
    fn different_layers_no_interaction() {
        let a = CollisionLayers::new(LAYER_PLAYER, LAYER_PLAYER);
        let b = CollisionLayers::new(LAYER_PROP, LAYER_PROP);
        assert!(!a.interacts_with(&b));
    }

    #[test]
    fn asymmetric_filter() {
        // A is player, filters for all
        let a = CollisionLayers::new(LAYER_PLAYER, LAYER_ALL);
        // B is prop, filters only for prop (not player)
        let b = CollisionLayers::new(LAYER_PROP, LAYER_PROP);
        // A.membership(PLAYER) & B.filter(PROP) = 0 → no interaction
        assert!(!a.interacts_with(&b));
    }

    #[test]
    fn trigger_only_interacts_with_player() {
        let trigger = CollisionLayers::trigger();
        let player = CollisionLayers::player();
        let prop = CollisionLayers::prop();

        assert!(trigger.interacts_with(&player));
        assert!(!trigger.interacts_with(&prop));
    }

    #[test]
    fn terrain_interacts_with_all() {
        let terrain = CollisionLayers::terrain();
        let player = CollisionLayers::player();
        let prop = CollisionLayers::prop();
        assert!(terrain.interacts_with(&player));
        assert!(terrain.interacts_with(&prop));
    }

    #[test]
    fn interaction_is_symmetric_when_filters_match() {
        let a = CollisionLayers::new(LAYER_PLAYER, LAYER_ALL);
        let b = CollisionLayers::new(LAYER_PROP, LAYER_ALL);
        assert!(a.interacts_with(&b));
        assert!(b.interacts_with(&a));
    }

    #[test]
    fn no_membership_means_no_interaction() {
        let a = CollisionLayers::new(0, LAYER_ALL);
        let b = CollisionLayers::new(LAYER_PLAYER, LAYER_ALL);
        assert!(!a.interacts_with(&b));
    }
}
