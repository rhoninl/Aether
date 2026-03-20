//! Remote procedure call system for client-server communication.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::input_buffer::PlayerId;

/// Direction of an RPC call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RpcDirection {
    ClientToServer,
    ServerToClient,
}

/// An RPC request from a caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub request_id: u64,
    pub caller: PlayerId,
    pub method: String,
    pub payload: Vec<u8>,
    pub direction: RpcDirection,
}

/// An RPC response returned to the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub request_id: u64,
    pub target: PlayerId,
    pub success: bool,
    pub payload: Vec<u8>,
    pub error_message: Option<String>,
}

/// Errors from RPC dispatch.
#[derive(Debug, Clone, PartialEq)]
pub enum RpcError {
    HandlerNotFound { method: String },
    HandlerFailed { method: String, reason: String },
}

/// A registered RPC handler function.
pub type RpcHandlerFn = Box<dyn Fn(&RpcRequest) -> Result<Vec<u8>, String> + Send + Sync>;

/// Manages RPC handler registration and dispatch.
pub struct RpcDispatcher {
    handlers: HashMap<String, RpcHandlerFn>,
    next_request_id: u64,
}

impl RpcDispatcher {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            next_request_id: 1,
        }
    }

    /// Register a handler for a method name. Overwrites any existing handler.
    pub fn register<F>(&mut self, method: &str, handler: F)
    where
        F: Fn(&RpcRequest) -> Result<Vec<u8>, String> + Send + Sync + 'static,
    {
        self.handlers.insert(method.to_string(), Box::new(handler));
    }

    /// Unregister a handler.
    pub fn unregister(&mut self, method: &str) -> bool {
        self.handlers.remove(method).is_some()
    }

    /// Check whether a method has a registered handler.
    pub fn has_handler(&self, method: &str) -> bool {
        self.handlers.contains_key(method)
    }

    /// Get the number of registered handlers.
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    /// Allocate a new unique request ID.
    pub fn next_request_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        id
    }

    /// Create an RPC request (helper for building requests).
    pub fn create_request(
        &mut self,
        caller: PlayerId,
        method: &str,
        payload: Vec<u8>,
        direction: RpcDirection,
    ) -> RpcRequest {
        let request_id = self.next_request_id();
        RpcRequest {
            request_id,
            caller,
            method: method.to_string(),
            payload,
            direction,
        }
    }

    /// Dispatch an RPC request to the appropriate handler.
    pub fn dispatch(&self, request: &RpcRequest) -> Result<RpcResponse, RpcError> {
        let handler =
            self.handlers
                .get(&request.method)
                .ok_or_else(|| RpcError::HandlerNotFound {
                    method: request.method.clone(),
                })?;

        match handler(request) {
            Ok(response_payload) => Ok(RpcResponse {
                request_id: request.request_id,
                target: request.caller,
                success: true,
                payload: response_payload,
                error_message: None,
            }),
            Err(reason) => Err(RpcError::HandlerFailed {
                method: request.method.clone(),
                reason,
            }),
        }
    }

    /// Dispatch multiple requests, returning responses and errors.
    pub fn dispatch_batch(&self, requests: &[RpcRequest]) -> Vec<Result<RpcResponse, RpcError>> {
        requests.iter().map(|req| self.dispatch(req)).collect()
    }
}

impl Default for RpcDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

// Cannot derive Debug for RpcDispatcher due to trait object in HashMap.
impl std::fmt::Debug for RpcDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcDispatcher")
            .field("handler_count", &self.handlers.len())
            .field("next_request_id", &self.next_request_id)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_register_and_has_handler() {
        let mut dispatcher = RpcDispatcher::new();
        assert!(!dispatcher.has_handler("greet"));

        dispatcher.register("greet", |_req| Ok(b"hello".to_vec()));
        assert!(dispatcher.has_handler("greet"));
        assert_eq!(dispatcher.handler_count(), 1);
    }

    #[test]
    fn test_unregister() {
        let mut dispatcher = RpcDispatcher::new();
        dispatcher.register("greet", |_req| Ok(b"hello".to_vec()));
        assert!(dispatcher.unregister("greet"));
        assert!(!dispatcher.has_handler("greet"));
        assert!(!dispatcher.unregister("greet")); // already removed
    }

    #[test]
    fn test_dispatch_success() {
        let mut dispatcher = RpcDispatcher::new();
        dispatcher.register("echo", |req| Ok(req.payload.clone()));

        let caller = Uuid::new_v4();
        let request = RpcRequest {
            request_id: 1,
            caller,
            method: "echo".to_string(),
            payload: b"test data".to_vec(),
            direction: RpcDirection::ClientToServer,
        };

        let response = dispatcher.dispatch(&request).unwrap();
        assert!(response.success);
        assert_eq!(response.payload, b"test data");
        assert_eq!(response.request_id, 1);
        assert_eq!(response.target, caller);
        assert!(response.error_message.is_none());
    }

    #[test]
    fn test_dispatch_handler_not_found() {
        let dispatcher = RpcDispatcher::new();
        let request = RpcRequest {
            request_id: 1,
            caller: Uuid::new_v4(),
            method: "nonexistent".to_string(),
            payload: vec![],
            direction: RpcDirection::ClientToServer,
        };

        let result = dispatcher.dispatch(&request);
        assert_eq!(
            result.unwrap_err(),
            RpcError::HandlerNotFound {
                method: "nonexistent".to_string()
            }
        );
    }

    #[test]
    fn test_dispatch_handler_error() {
        let mut dispatcher = RpcDispatcher::new();
        dispatcher.register("fail", |_req| Err("something went wrong".to_string()));

        let request = RpcRequest {
            request_id: 1,
            caller: Uuid::new_v4(),
            method: "fail".to_string(),
            payload: vec![],
            direction: RpcDirection::ClientToServer,
        };

        let result = dispatcher.dispatch(&request);
        assert_eq!(
            result.unwrap_err(),
            RpcError::HandlerFailed {
                method: "fail".to_string(),
                reason: "something went wrong".to_string(),
            }
        );
    }

    #[test]
    fn test_request_id_monotonic() {
        let mut dispatcher = RpcDispatcher::new();
        let id1 = dispatcher.next_request_id();
        let id2 = dispatcher.next_request_id();
        let id3 = dispatcher.next_request_id();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn test_create_request() {
        let mut dispatcher = RpcDispatcher::new();
        let caller = Uuid::new_v4();
        let req = dispatcher.create_request(
            caller,
            "test_method",
            b"payload".to_vec(),
            RpcDirection::ServerToClient,
        );

        assert_eq!(req.request_id, 1);
        assert_eq!(req.caller, caller);
        assert_eq!(req.method, "test_method");
        assert_eq!(req.payload, b"payload");
        assert_eq!(req.direction, RpcDirection::ServerToClient);
    }

    #[test]
    fn test_dispatch_batch() {
        let mut dispatcher = RpcDispatcher::new();
        dispatcher.register("echo", |req| Ok(req.payload.clone()));

        let caller = Uuid::new_v4();
        let requests = vec![
            RpcRequest {
                request_id: 1,
                caller,
                method: "echo".to_string(),
                payload: b"a".to_vec(),
                direction: RpcDirection::ClientToServer,
            },
            RpcRequest {
                request_id: 2,
                caller,
                method: "missing".to_string(),
                payload: vec![],
                direction: RpcDirection::ClientToServer,
            },
            RpcRequest {
                request_id: 3,
                caller,
                method: "echo".to_string(),
                payload: b"b".to_vec(),
                direction: RpcDirection::ClientToServer,
            },
        ];

        let results = dispatcher.dispatch_batch(&requests);
        assert_eq!(results.len(), 3);
        assert!(results[0].is_ok());
        assert!(results[1].is_err());
        assert!(results[2].is_ok());
    }

    #[test]
    fn test_register_overwrites_existing() {
        let mut dispatcher = RpcDispatcher::new();
        dispatcher.register("method", |_| Ok(b"v1".to_vec()));
        dispatcher.register("method", |_| Ok(b"v2".to_vec()));

        assert_eq!(dispatcher.handler_count(), 1);

        let request = RpcRequest {
            request_id: 1,
            caller: Uuid::new_v4(),
            method: "method".to_string(),
            payload: vec![],
            direction: RpcDirection::ClientToServer,
        };

        let response = dispatcher.dispatch(&request).unwrap();
        assert_eq!(response.payload, b"v2");
    }

    #[test]
    fn test_handler_accesses_request_fields() {
        let mut dispatcher = RpcDispatcher::new();
        dispatcher.register("check_caller", |req| {
            // Handler can inspect request fields
            Ok(req.caller.as_bytes().to_vec())
        });

        let caller = Uuid::new_v4();
        let request = RpcRequest {
            request_id: 1,
            caller,
            method: "check_caller".to_string(),
            payload: vec![],
            direction: RpcDirection::ClientToServer,
        };

        let response = dispatcher.dispatch(&request).unwrap();
        assert_eq!(response.payload, caller.as_bytes().to_vec());
    }

    #[test]
    fn test_debug_format() {
        let dispatcher = RpcDispatcher::new();
        let debug_str = format!("{:?}", dispatcher);
        assert!(debug_str.contains("RpcDispatcher"));
        assert!(debug_str.contains("handler_count"));
    }
}
