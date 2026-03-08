//! Service dispatch logic.
//!
//! Given a [`RouteMatch`](crate::router::RouteMatch) and an optional
//! authenticated user, builds a [`DispatchContext`] that downstream code can
//! use to forward the request to the correct backend service.

use std::collections::HashMap;

use crate::router::{RouteMatch, ServiceTarget};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// All the information needed to forward a request to a backend service.
#[derive(Debug, Clone)]
pub struct DispatchContext {
    /// Which service to send the request to.
    pub service: ServiceTarget,
    /// Extracted path parameters.
    pub params: HashMap<String, String>,
    /// Authenticated user id, if applicable.
    pub user_id: Option<u64>,
    /// The original matched pattern (useful for logging / metrics keys).
    pub matched_pattern: String,
}

/// Errors that can occur during dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchError {
    /// The matched route requires a `zone_id` param that was not present.
    MissingRequiredParam(String),
}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

/// Builds [`DispatchContext`] values from route matches.
pub struct Dispatcher;

impl Dispatcher {
    pub fn new() -> Self {
        Self
    }

    /// Build a dispatch context from a route match.
    ///
    /// For `ServiceTarget::WorldServer` the dispatcher will look up the
    /// `world_id` or `zone_id` param and populate the target's `zone_id`
    /// field accordingly.  If neither param exists the original route
    /// definition's `zone_id` is kept.
    pub fn dispatch(
        &self,
        route_match: &RouteMatch,
        user_id: Option<u64>,
    ) -> Result<DispatchContext, DispatchError> {
        let service = self.resolve_service(route_match);

        Ok(DispatchContext {
            service,
            params: route_match.params.clone(),
            user_id,
            matched_pattern: route_match.matched_pattern.clone(),
        })
    }

    /// Resolve the final `ServiceTarget`, enriching dynamic fields from path
    /// params when applicable.
    fn resolve_service(&self, route_match: &RouteMatch) -> ServiceTarget {
        match &route_match.service {
            ServiceTarget::WorldServer { zone_id } => {
                // Prefer extracted params over the static route definition.
                let resolved_zone = route_match
                    .params
                    .get("zone_id")
                    .or_else(|| route_match.params.get("world_id"))
                    .cloned()
                    .unwrap_or_else(|| zone_id.clone());

                ServiceTarget::WorldServer {
                    zone_id: resolved_zone,
                }
            }
            other => other.clone(),
        }
    }
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::{HttpMethod, Route, Router, ServiceTarget};

    fn make_route_match(
        service: ServiceTarget,
        params: HashMap<String, String>,
        pattern: &str,
    ) -> RouteMatch {
        RouteMatch {
            route_index: 0,
            params,
            service,
            auth_required: true,
            rate_limit: None,
            matched_pattern: pattern.to_string(),
        }
    }

    #[test]
    fn dispatch_world_server_with_zone_param() {
        let dispatcher = Dispatcher::new();
        let mut params = HashMap::new();
        params.insert("zone_id".to_string(), "zone_42".to_string());

        let rm = make_route_match(
            ServiceTarget::WorldServer {
                zone_id: "default".to_string(),
            },
            params,
            "/api/v1/worlds/:zone_id",
        );

        let ctx = dispatcher.dispatch(&rm, Some(1)).unwrap();
        assert_eq!(
            ctx.service,
            ServiceTarget::WorldServer {
                zone_id: "zone_42".to_string()
            }
        );
        assert_eq!(ctx.user_id, Some(1));
    }

    #[test]
    fn dispatch_world_server_with_world_id_param() {
        let dispatcher = Dispatcher::new();
        let mut params = HashMap::new();
        params.insert("world_id".to_string(), "w_abc".to_string());

        let rm = make_route_match(
            ServiceTarget::WorldServer {
                zone_id: "default".to_string(),
            },
            params,
            "/api/v1/worlds/:world_id",
        );

        let ctx = dispatcher.dispatch(&rm, Some(5)).unwrap();
        assert_eq!(
            ctx.service,
            ServiceTarget::WorldServer {
                zone_id: "w_abc".to_string()
            }
        );
    }

    #[test]
    fn dispatch_world_server_falls_back_to_route_zone() {
        let dispatcher = Dispatcher::new();
        let rm = make_route_match(
            ServiceTarget::WorldServer {
                zone_id: "static_zone".to_string(),
            },
            HashMap::new(),
            "/api/v1/worlds",
        );

        let ctx = dispatcher.dispatch(&rm, None).unwrap();
        assert_eq!(
            ctx.service,
            ServiceTarget::WorldServer {
                zone_id: "static_zone".to_string()
            }
        );
    }

    #[test]
    fn dispatch_auth_service() {
        let dispatcher = Dispatcher::new();
        let rm = make_route_match(
            ServiceTarget::AuthService,
            HashMap::new(),
            "/api/v1/auth/login",
        );
        let ctx = dispatcher.dispatch(&rm, None).unwrap();
        assert_eq!(ctx.service, ServiceTarget::AuthService);
        assert!(ctx.user_id.is_none());
    }

    #[test]
    fn dispatch_social_service() {
        let dispatcher = Dispatcher::new();
        let mut params = HashMap::new();
        params.insert("user_id".to_string(), "99".to_string());
        let rm = make_route_match(
            ServiceTarget::SocialService,
            params,
            "/api/v1/social/friends/:user_id",
        );
        let ctx = dispatcher.dispatch(&rm, Some(10)).unwrap();
        assert_eq!(ctx.service, ServiceTarget::SocialService);
        assert_eq!(ctx.params.get("user_id").unwrap(), "99");
        assert_eq!(ctx.user_id, Some(10));
    }

    #[test]
    fn dispatch_economy_service() {
        let dispatcher = Dispatcher::new();
        let rm = make_route_match(
            ServiceTarget::EconomyService,
            HashMap::new(),
            "/api/v1/economy/transfer",
        );
        let ctx = dispatcher.dispatch(&rm, Some(7)).unwrap();
        assert_eq!(ctx.service, ServiceTarget::EconomyService);
    }

    #[test]
    fn dispatch_ugc_service() {
        let dispatcher = Dispatcher::new();
        let mut params = HashMap::new();
        params.insert("*".to_string(), "uploads/image.png".to_string());
        let rm = make_route_match(
            ServiceTarget::UgcService,
            params,
            "/api/v1/ugc/*",
        );
        let ctx = dispatcher.dispatch(&rm, Some(3)).unwrap();
        assert_eq!(ctx.service, ServiceTarget::UgcService);
        assert_eq!(ctx.params.get("*").unwrap(), "uploads/image.png");
    }

    #[test]
    fn dispatch_registry_service() {
        let dispatcher = Dispatcher::new();
        let rm = make_route_match(
            ServiceTarget::RegistryService,
            HashMap::new(),
            "/api/v1/registry/search",
        );
        let ctx = dispatcher.dispatch(&rm, Some(11)).unwrap();
        assert_eq!(ctx.service, ServiceTarget::RegistryService);
    }

    #[test]
    fn dispatch_preserves_matched_pattern() {
        let dispatcher = Dispatcher::new();
        let rm = make_route_match(
            ServiceTarget::AuthService,
            HashMap::new(),
            "/api/v1/auth/token",
        );
        let ctx = dispatcher.dispatch(&rm, None).unwrap();
        assert_eq!(ctx.matched_pattern, "/api/v1/auth/token");
    }

    #[test]
    fn dispatch_via_router_integration() {
        // End-to-end: register route, match, dispatch.
        let mut router = Router::new();
        router.add_route(Route {
            path_pattern: "/api/v1/worlds/:world_id".to_string(),
            method: HttpMethod::Get,
            service: ServiceTarget::WorldServer {
                zone_id: "default".to_string(),
            },
            auth_required: true,
            rate_limit: None,
        });

        let rm = router
            .match_request(HttpMethod::Get, "/api/v1/worlds/my_world")
            .unwrap();

        let dispatcher = Dispatcher::new();
        let ctx = dispatcher.dispatch(&rm, Some(42)).unwrap();
        assert_eq!(
            ctx.service,
            ServiceTarget::WorldServer {
                zone_id: "my_world".to_string()
            }
        );
        assert_eq!(ctx.user_id, Some(42));
    }

    #[test]
    fn dispatcher_default_trait() {
        let _d = Dispatcher::default();
    }
}
