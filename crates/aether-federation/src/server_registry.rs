//! Server registry for managing federated server lifecycle.

use std::collections::HashMap;

/// Status of a federated server.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerStatus {
    Online,
    Degraded,
    Offline,
    Suspended,
}

/// A registered federated server.
#[derive(Debug, Clone, PartialEq)]
pub struct FederatedServer {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub public_key: Vec<u8>,
    pub registered_at_ms: u64,
    pub last_heartbeat_ms: u64,
    pub status: ServerStatus,
}

/// Error returned by registry operations.
#[derive(Debug, Clone, PartialEq)]
pub enum RegistryError {
    AlreadyRegistered,
    NotFound,
    InvalidEndpoint,
    InvalidName,
}

/// In-memory registry of federated servers.
#[derive(Debug)]
pub struct ServerRegistry {
    servers: HashMap<String, FederatedServer>,
}

impl ServerRegistry {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }

    /// Register a new federated server. Returns error if ID already exists or
    /// endpoint/name are empty.
    pub fn register(&mut self, server: FederatedServer) -> Result<(), RegistryError> {
        if server.endpoint.is_empty() {
            return Err(RegistryError::InvalidEndpoint);
        }
        if server.name.is_empty() {
            return Err(RegistryError::InvalidName);
        }
        if self.servers.contains_key(&server.id) {
            return Err(RegistryError::AlreadyRegistered);
        }
        self.servers.insert(server.id.clone(), server);
        Ok(())
    }

    /// Remove a server by ID. Returns error if not found.
    pub fn deregister(&mut self, server_id: &str) -> Result<FederatedServer, RegistryError> {
        self.servers.remove(server_id).ok_or(RegistryError::NotFound)
    }

    /// Get a server by ID.
    pub fn get(&self, server_id: &str) -> Option<&FederatedServer> {
        self.servers.get(server_id)
    }

    /// List all servers, optionally filtered by status.
    pub fn list(&self, status_filter: Option<&ServerStatus>) -> Vec<&FederatedServer> {
        self.servers
            .values()
            .filter(|s| status_filter.map_or(true, |f| &s.status == f))
            .collect()
    }

    /// Update the status of a server. Returns error if not found.
    pub fn update_status(
        &mut self,
        server_id: &str,
        status: ServerStatus,
    ) -> Result<(), RegistryError> {
        let server = self.servers.get_mut(server_id).ok_or(RegistryError::NotFound)?;
        server.status = status;
        Ok(())
    }

    /// Record a heartbeat for a server. Returns error if not found.
    pub fn record_heartbeat(
        &mut self,
        server_id: &str,
        timestamp_ms: u64,
    ) -> Result<(), RegistryError> {
        let server = self.servers.get_mut(server_id).ok_or(RegistryError::NotFound)?;
        server.last_heartbeat_ms = timestamp_ms;
        Ok(())
    }

    /// Return the total number of registered servers.
    pub fn count(&self) -> usize {
        self.servers.len()
    }
}

impl Default for ServerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_server(id: &str, name: &str, endpoint: &str) -> FederatedServer {
        FederatedServer {
            id: id.to_string(),
            name: name.to_string(),
            endpoint: endpoint.to_string(),
            public_key: vec![1, 2, 3],
            registered_at_ms: 1000,
            last_heartbeat_ms: 1000,
            status: ServerStatus::Online,
        }
    }

    #[test]
    fn register_and_get() {
        let mut reg = ServerRegistry::new();
        let server = make_server("s1", "Server One", "https://s1.example.com");
        assert!(reg.register(server).is_ok());
        assert_eq!(reg.count(), 1);

        let found = reg.get("s1").unwrap();
        assert_eq!(found.name, "Server One");
        assert_eq!(found.endpoint, "https://s1.example.com");
    }

    #[test]
    fn register_duplicate_is_error() {
        let mut reg = ServerRegistry::new();
        let s1 = make_server("s1", "Server One", "https://s1.example.com");
        let s1_dup = make_server("s1", "Server One Dup", "https://s1-dup.example.com");
        assert!(reg.register(s1).is_ok());
        assert_eq!(reg.register(s1_dup), Err(RegistryError::AlreadyRegistered));
        assert_eq!(reg.count(), 1);
    }

    #[test]
    fn register_empty_endpoint_is_error() {
        let mut reg = ServerRegistry::new();
        let server = make_server("s1", "Server One", "");
        assert_eq!(reg.register(server), Err(RegistryError::InvalidEndpoint));
    }

    #[test]
    fn register_empty_name_is_error() {
        let mut reg = ServerRegistry::new();
        let server = make_server("s1", "", "https://s1.example.com");
        assert_eq!(reg.register(server), Err(RegistryError::InvalidName));
    }

    #[test]
    fn deregister_existing() {
        let mut reg = ServerRegistry::new();
        reg.register(make_server("s1", "Server One", "https://s1.example.com"))
            .unwrap();
        let removed = reg.deregister("s1").unwrap();
        assert_eq!(removed.id, "s1");
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn deregister_missing_is_error() {
        let mut reg = ServerRegistry::new();
        assert_eq!(reg.deregister("nope"), Err(RegistryError::NotFound));
    }

    #[test]
    fn list_all() {
        let mut reg = ServerRegistry::new();
        reg.register(make_server("s1", "One", "https://s1.example.com"))
            .unwrap();
        reg.register(make_server("s2", "Two", "https://s2.example.com"))
            .unwrap();
        assert_eq!(reg.list(None).len(), 2);
    }

    #[test]
    fn list_filtered_by_status() {
        let mut reg = ServerRegistry::new();
        reg.register(make_server("s1", "One", "https://s1.example.com"))
            .unwrap();
        reg.register(make_server("s2", "Two", "https://s2.example.com"))
            .unwrap();
        reg.update_status("s2", ServerStatus::Offline).unwrap();

        let online = reg.list(Some(&ServerStatus::Online));
        assert_eq!(online.len(), 1);
        assert_eq!(online[0].id, "s1");

        let offline = reg.list(Some(&ServerStatus::Offline));
        assert_eq!(offline.len(), 1);
        assert_eq!(offline[0].id, "s2");
    }

    #[test]
    fn update_status() {
        let mut reg = ServerRegistry::new();
        reg.register(make_server("s1", "One", "https://s1.example.com"))
            .unwrap();
        reg.update_status("s1", ServerStatus::Degraded).unwrap();
        assert_eq!(reg.get("s1").unwrap().status, ServerStatus::Degraded);
    }

    #[test]
    fn update_status_missing_is_error() {
        let mut reg = ServerRegistry::new();
        assert_eq!(
            reg.update_status("nope", ServerStatus::Online),
            Err(RegistryError::NotFound)
        );
    }

    #[test]
    fn record_heartbeat() {
        let mut reg = ServerRegistry::new();
        reg.register(make_server("s1", "One", "https://s1.example.com"))
            .unwrap();
        reg.record_heartbeat("s1", 5000).unwrap();
        assert_eq!(reg.get("s1").unwrap().last_heartbeat_ms, 5000);
    }

    #[test]
    fn record_heartbeat_missing_is_error() {
        let mut reg = ServerRegistry::new();
        assert_eq!(
            reg.record_heartbeat("nope", 5000),
            Err(RegistryError::NotFound)
        );
    }

    #[test]
    fn get_missing_returns_none() {
        let reg = ServerRegistry::new();
        assert!(reg.get("nope").is_none());
    }

    #[test]
    fn default_creates_empty_registry() {
        let reg = ServerRegistry::default();
        assert_eq!(reg.count(), 0);
    }
}
