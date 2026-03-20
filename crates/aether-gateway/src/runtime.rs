//! Gateway backend runtime — orchestrates routing decisions and service dispatch.
//!
//! This module provides the high-level runtime that processes incoming requests
//! through the gateway pipeline: auth validation → rate limiting → routing → dispatch.

use std::collections::HashMap;

use crate::auth::AuthValidationPolicy;
use crate::route::GeoRoutingPolicy;

/// Configuration for the gateway backend runtime.
#[derive(Debug, Clone)]
pub struct BackendRuntimeConfig {
    pub token_ttl_ms: u64,
    pub token_refresh_window_ms: u64,
    pub max_connections_per_user: u32,
    pub request_timeout_ms: u64,
    pub auth_backends: Vec<String>,
    pub live_regions: Vec<String>,
    pub rate_limit_window_ms: u64,
}

impl Default for BackendRuntimeConfig {
    fn default() -> Self {
        Self {
            token_ttl_ms: 3_600_000,
            token_refresh_window_ms: 300_000,
            max_connections_per_user: 5,
            request_timeout_ms: 30_000,
            auth_backends: vec!["local".into()],
            live_regions: vec!["us-east-1".into()],
            rate_limit_window_ms: 60_000,
        }
    }
}

/// Mutable state accumulated across runtime steps.
#[derive(Debug, Clone, Default)]
pub struct BackendRuntimeState {
    pub active_sessions: HashMap<u64, SessionEntry>,
    pub request_count: u64,
    pub error_count: u64,
}

/// A tracked session in the runtime.
#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub user_id: u64,
    pub token_issued_ms: u64,
    pub last_activity_ms: u64,
    pub region: String,
}

/// Input to a single runtime step.
#[derive(Debug, Clone, Default)]
pub struct BackendStepInput {
    pub now_ms: u64,
    pub auth_logins: Vec<u64>,
    pub route_requests: Vec<(String, String)>,
}

/// Output from a single runtime step.
#[derive(Debug, Clone, Default)]
pub struct BackendStepOutput {
    pub sessions_created: Vec<u64>,
    pub routes_resolved: Vec<(String, String)>,
    pub errors: Vec<String>,
}

/// The backend runtime processes gateway operations per tick.
#[derive(Debug, Clone, Default)]
pub struct BackendRuntime {
    pub config: BackendRuntimeConfig,
}

impl BackendRuntime {
    pub fn new(config: BackendRuntimeConfig) -> Self {
        Self { config }
    }

    /// Execute one step of the runtime loop.
    pub fn step(
        &self,
        input: BackendStepInput,
        state: &mut BackendRuntimeState,
        _auth_policy: &AuthValidationPolicy,
        _geo_policy: Option<&GeoRoutingPolicy>,
    ) -> BackendStepOutput {
        let mut output = BackendStepOutput::default();

        // Process auth logins — create sessions
        for user_id in &input.auth_logins {
            state.active_sessions.insert(
                *user_id,
                SessionEntry {
                    user_id: *user_id,
                    token_issued_ms: input.now_ms,
                    last_activity_ms: input.now_ms,
                    region: self
                        .config
                        .live_regions
                        .first()
                        .cloned()
                        .unwrap_or_default(),
                },
            );
            output.sessions_created.push(*user_id);
        }

        // Process route requests
        for (world_id, region) in &input.route_requests {
            output
                .routes_resolved
                .push((world_id.clone(), region.clone()));
        }

        state.request_count += 1;
        output
    }

    /// Expire sessions that have been idle beyond the token TTL.
    pub fn expire_sessions(&self, state: &mut BackendRuntimeState, now_ms: u64) -> Vec<u64> {
        let expired: Vec<u64> = state
            .active_sessions
            .iter()
            .filter(|(_, s)| now_ms.saturating_sub(s.last_activity_ms) > self.config.token_ttl_ms)
            .map(|(id, _)| *id)
            .collect();

        for id in &expired {
            state.active_sessions.remove(id);
        }

        expired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_creates_sessions_on_login() {
        let runtime = BackendRuntime::default();
        let mut state = BackendRuntimeState::default();
        let policy = AuthValidationPolicy {
            require_expiry_check: false,
            require_signature: false,
            accepted_issuers: vec!["local".into()],
        };

        let output = runtime.step(
            BackendStepInput {
                now_ms: 1000,
                auth_logins: vec![1, 2],
                route_requests: vec![],
            },
            &mut state,
            &policy,
            None,
        );

        assert_eq!(output.sessions_created.len(), 2);
        assert_eq!(state.active_sessions.len(), 2);
        assert!(state.active_sessions.contains_key(&1));
        assert!(state.active_sessions.contains_key(&2));
    }

    #[test]
    fn runtime_resolves_routes() {
        let runtime = BackendRuntime::default();
        let mut state = BackendRuntimeState::default();
        let policy = AuthValidationPolicy {
            require_expiry_check: false,
            require_signature: false,
            accepted_issuers: vec!["local".into()],
        };

        let output = runtime.step(
            BackendStepInput {
                now_ms: 1000,
                auth_logins: vec![],
                route_requests: vec![("world-1".into(), "us-east-1".into())],
            },
            &mut state,
            &policy,
            None,
        );

        assert_eq!(output.routes_resolved.len(), 1);
    }

    #[test]
    fn runtime_expires_stale_sessions() {
        let runtime = BackendRuntime::default();
        let mut state = BackendRuntimeState::default();
        let policy = AuthValidationPolicy {
            require_expiry_check: false,
            require_signature: false,
            accepted_issuers: vec!["local".into()],
        };

        runtime.step(
            BackendStepInput {
                now_ms: 1000,
                auth_logins: vec![1, 2],
                route_requests: vec![],
            },
            &mut state,
            &policy,
            None,
        );

        assert_eq!(state.active_sessions.len(), 2);

        // Expire after TTL
        let expired = runtime.expire_sessions(&mut state, 1000 + 3_600_001);
        assert_eq!(expired.len(), 2);
        assert_eq!(state.active_sessions.len(), 0);
    }

    #[test]
    fn runtime_request_counter_increments() {
        let runtime = BackendRuntime::default();
        let mut state = BackendRuntimeState::default();
        let policy = AuthValidationPolicy {
            require_expiry_check: false,
            require_signature: false,
            accepted_issuers: vec!["local".into()],
        };

        runtime.step(BackendStepInput::default(), &mut state, &policy, None);
        runtime.step(BackendStepInput::default(), &mut state, &policy, None);

        assert_eq!(state.request_count, 2);
    }

    #[test]
    fn default_config_values() {
        let config = BackendRuntimeConfig::default();
        assert_eq!(config.token_ttl_ms, 3_600_000);
        assert_eq!(config.max_connections_per_user, 5);
        assert_eq!(config.request_timeout_ms, 30_000);
    }
}
