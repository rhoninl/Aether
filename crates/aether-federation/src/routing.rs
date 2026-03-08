//! Cross-server portal routing for federation.

use std::collections::HashMap;

/// A route through a portal from one server to another.
#[derive(Debug, Clone, PartialEq)]
pub struct PortalRoute {
    pub portal_id: String,
    pub source_server: String,
    pub destination_server: String,
    pub destination_world: String,
    pub active: bool,
}

/// Error returned by routing operations.
#[derive(Debug, Clone, PartialEq)]
pub enum RoutingError {
    RouteAlreadyExists,
    RouteNotFound,
    InvalidRoute(String),
}

/// Resolved destination for a portal traversal.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedDestination {
    pub portal_id: String,
    pub destination_server: String,
    pub destination_world: String,
}

/// In-memory routing table for cross-server portal traversals.
#[derive(Debug)]
pub struct RoutingTable {
    routes: HashMap<String, PortalRoute>,
}

impl RoutingTable {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    /// Add a new portal route. Returns error if portal_id already exists or fields are invalid.
    pub fn add_route(&mut self, route: PortalRoute) -> Result<(), RoutingError> {
        if route.portal_id.is_empty() {
            return Err(RoutingError::InvalidRoute(
                "portal_id is empty".to_string(),
            ));
        }
        if route.source_server.is_empty() || route.destination_server.is_empty() {
            return Err(RoutingError::InvalidRoute(
                "server IDs must not be empty".to_string(),
            ));
        }
        if route.destination_world.is_empty() {
            return Err(RoutingError::InvalidRoute(
                "destination_world is empty".to_string(),
            ));
        }
        if self.routes.contains_key(&route.portal_id) {
            return Err(RoutingError::RouteAlreadyExists);
        }
        self.routes.insert(route.portal_id.clone(), route);
        Ok(())
    }

    /// Remove a portal route by ID. Returns the removed route or error if not found.
    pub fn remove_route(&mut self, portal_id: &str) -> Result<PortalRoute, RoutingError> {
        self.routes
            .remove(portal_id)
            .ok_or(RoutingError::RouteNotFound)
    }

    /// Look up a route by portal ID.
    pub fn lookup(&self, portal_id: &str) -> Option<&PortalRoute> {
        self.routes.get(portal_id)
    }

    /// List all routes where the given server is the source.
    pub fn list_by_source(&self, server_id: &str) -> Vec<&PortalRoute> {
        self.routes
            .values()
            .filter(|r| r.source_server == server_id)
            .collect()
    }

    /// List all routes where the given server is the destination.
    pub fn list_by_destination(&self, server_id: &str) -> Vec<&PortalRoute> {
        self.routes
            .values()
            .filter(|r| r.destination_server == server_id)
            .collect()
    }

    /// Set a route's active status. Returns error if not found.
    pub fn set_active(
        &mut self,
        portal_id: &str,
        active: bool,
    ) -> Result<(), RoutingError> {
        let route = self
            .routes
            .get_mut(portal_id)
            .ok_or(RoutingError::RouteNotFound)?;
        route.active = active;
        Ok(())
    }

    /// Resolve the destination for a portal traversal. Only returns active routes.
    pub fn resolve_destination(
        &self,
        portal_id: &str,
    ) -> Result<ResolvedDestination, RoutingError> {
        let route = self
            .routes
            .get(portal_id)
            .ok_or(RoutingError::RouteNotFound)?;
        if !route.active {
            return Err(RoutingError::InvalidRoute(
                "route is inactive".to_string(),
            ));
        }
        Ok(ResolvedDestination {
            portal_id: route.portal_id.clone(),
            destination_server: route.destination_server.clone(),
            destination_world: route.destination_world.clone(),
        })
    }

    /// Return the total number of routes.
    pub fn count(&self) -> usize {
        self.routes.len()
    }

    /// Return the number of active routes.
    pub fn active_count(&self) -> usize {
        self.routes.values().filter(|r| r.active).count()
    }
}

impl Default for RoutingTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_route(
        portal_id: &str,
        source: &str,
        dest_server: &str,
        dest_world: &str,
        active: bool,
    ) -> PortalRoute {
        PortalRoute {
            portal_id: portal_id.to_string(),
            source_server: source.to_string(),
            destination_server: dest_server.to_string(),
            destination_world: dest_world.to_string(),
            active,
        }
    }

    #[test]
    fn add_and_lookup() {
        let mut table = RoutingTable::new();
        table
            .add_route(make_route("p1", "A", "B", "world1", true))
            .unwrap();
        let route = table.lookup("p1").unwrap();
        assert_eq!(route.source_server, "A");
        assert_eq!(route.destination_server, "B");
        assert_eq!(route.destination_world, "world1");
        assert!(route.active);
    }

    #[test]
    fn add_duplicate_is_error() {
        let mut table = RoutingTable::new();
        table
            .add_route(make_route("p1", "A", "B", "w1", true))
            .unwrap();
        assert_eq!(
            table.add_route(make_route("p1", "C", "D", "w2", true)),
            Err(RoutingError::RouteAlreadyExists)
        );
    }

    #[test]
    fn add_empty_portal_id_is_error() {
        let mut table = RoutingTable::new();
        assert!(matches!(
            table.add_route(make_route("", "A", "B", "w1", true)),
            Err(RoutingError::InvalidRoute(_))
        ));
    }

    #[test]
    fn add_empty_server_is_error() {
        let mut table = RoutingTable::new();
        assert!(matches!(
            table.add_route(make_route("p1", "", "B", "w1", true)),
            Err(RoutingError::InvalidRoute(_))
        ));
        assert!(matches!(
            table.add_route(make_route("p1", "A", "", "w1", true)),
            Err(RoutingError::InvalidRoute(_))
        ));
    }

    #[test]
    fn add_empty_world_is_error() {
        let mut table = RoutingTable::new();
        assert!(matches!(
            table.add_route(make_route("p1", "A", "B", "", true)),
            Err(RoutingError::InvalidRoute(_))
        ));
    }

    #[test]
    fn remove_existing() {
        let mut table = RoutingTable::new();
        table
            .add_route(make_route("p1", "A", "B", "w1", true))
            .unwrap();
        let removed = table.remove_route("p1").unwrap();
        assert_eq!(removed.portal_id, "p1");
        assert_eq!(table.count(), 0);
    }

    #[test]
    fn remove_missing_is_error() {
        let mut table = RoutingTable::new();
        assert_eq!(table.remove_route("nope"), Err(RoutingError::RouteNotFound));
    }

    #[test]
    fn lookup_missing_returns_none() {
        let table = RoutingTable::new();
        assert!(table.lookup("nope").is_none());
    }

    #[test]
    fn list_by_source() {
        let mut table = RoutingTable::new();
        table
            .add_route(make_route("p1", "A", "B", "w1", true))
            .unwrap();
        table
            .add_route(make_route("p2", "A", "C", "w2", true))
            .unwrap();
        table
            .add_route(make_route("p3", "B", "A", "w3", true))
            .unwrap();

        let from_a = table.list_by_source("A");
        assert_eq!(from_a.len(), 2);
        assert!(from_a.iter().all(|r| r.source_server == "A"));
    }

    #[test]
    fn list_by_destination() {
        let mut table = RoutingTable::new();
        table
            .add_route(make_route("p1", "A", "B", "w1", true))
            .unwrap();
        table
            .add_route(make_route("p2", "C", "B", "w2", true))
            .unwrap();
        table
            .add_route(make_route("p3", "B", "A", "w3", true))
            .unwrap();

        let to_b = table.list_by_destination("B");
        assert_eq!(to_b.len(), 2);
        assert!(to_b.iter().all(|r| r.destination_server == "B"));
    }

    #[test]
    fn set_active() {
        let mut table = RoutingTable::new();
        table
            .add_route(make_route("p1", "A", "B", "w1", true))
            .unwrap();
        table.set_active("p1", false).unwrap();
        assert!(!table.lookup("p1").unwrap().active);
        table.set_active("p1", true).unwrap();
        assert!(table.lookup("p1").unwrap().active);
    }

    #[test]
    fn set_active_missing_is_error() {
        let mut table = RoutingTable::new();
        assert_eq!(
            table.set_active("nope", true),
            Err(RoutingError::RouteNotFound)
        );
    }

    #[test]
    fn resolve_destination_active() {
        let mut table = RoutingTable::new();
        table
            .add_route(make_route("p1", "A", "B", "w1", true))
            .unwrap();
        let dest = table.resolve_destination("p1").unwrap();
        assert_eq!(
            dest,
            ResolvedDestination {
                portal_id: "p1".to_string(),
                destination_server: "B".to_string(),
                destination_world: "w1".to_string(),
            }
        );
    }

    #[test]
    fn resolve_destination_inactive_is_error() {
        let mut table = RoutingTable::new();
        table
            .add_route(make_route("p1", "A", "B", "w1", false))
            .unwrap();
        assert!(matches!(
            table.resolve_destination("p1"),
            Err(RoutingError::InvalidRoute(_))
        ));
    }

    #[test]
    fn resolve_destination_missing_is_error() {
        let table = RoutingTable::new();
        assert_eq!(
            table.resolve_destination("nope"),
            Err(RoutingError::RouteNotFound)
        );
    }

    #[test]
    fn count_and_active_count() {
        let mut table = RoutingTable::new();
        table
            .add_route(make_route("p1", "A", "B", "w1", true))
            .unwrap();
        table
            .add_route(make_route("p2", "A", "C", "w2", false))
            .unwrap();
        table
            .add_route(make_route("p3", "B", "A", "w3", true))
            .unwrap();
        assert_eq!(table.count(), 3);
        assert_eq!(table.active_count(), 2);
    }

    #[test]
    fn default_creates_empty_table() {
        let table = RoutingTable::default();
        assert_eq!(table.count(), 0);
    }
}
