//! HTTP request routing engine.
//!
//! Matches incoming requests by method and path pattern against a table of
//! registered routes.  Supports exact, parameterised (`:param`), and wildcard
//! (`*`) path segments.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// HTTP methods supported by the router.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Options,
    Head,
}

/// Identifies which backend service a request should be dispatched to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceTarget {
    WorldServer { zone_id: String },
    AuthService,
    SocialService,
    EconomyService,
    UgcService,
    RegistryService,
}

/// Per-route rate-limit rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitRule {
    /// Sustained requests per second.
    pub requests_per_second: u32,
    /// Maximum burst above the sustained rate.
    pub burst: u32,
}

/// A single route definition in the routing table.
#[derive(Debug, Clone)]
pub struct Route {
    /// Pattern such as `/api/v1/worlds/:id` or `/api/v1/ugc/*`.
    pub path_pattern: String,
    pub method: HttpMethod,
    pub service: ServiceTarget,
    pub auth_required: bool,
    pub rate_limit: Option<RateLimitRule>,
}

/// Result of a successful route match.
#[derive(Debug, Clone)]
pub struct RouteMatch {
    /// Index of the matched route in the router's table.
    pub route_index: usize,
    /// Extracted path parameters (e.g. `world_id` -> `"abc123"`).
    pub params: HashMap<String, String>,
    /// Reference-copy of the matched route's service target.
    pub service: ServiceTarget,
    /// Whether auth is required for this route.
    pub auth_required: bool,
    /// Rate-limit rule for this route, if any.
    pub rate_limit: Option<RateLimitRule>,
    /// The pattern that was matched.
    pub matched_pattern: String,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Request router that matches (method, path) pairs against registered routes.
///
/// Routes are evaluated in registration order; the first match wins.
pub struct Router {
    routes: Vec<Route>,
}

impl Router {
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    /// Register a route.  Returns `&mut Self` for chaining.
    pub fn add_route(&mut self, route: Route) -> &mut Self {
        self.routes.push(route);
        self
    }

    /// Return the list of registered routes.
    pub fn routes(&self) -> &[Route] {
        &self.routes
    }

    /// Try to match `(method, path)` against the routing table.
    pub fn match_request(&self, method: HttpMethod, path: &str) -> Option<RouteMatch> {
        let request_segments = split_path(path);

        for (idx, route) in self.routes.iter().enumerate() {
            if route.method != method {
                continue;
            }

            let pattern_segments = split_path(&route.path_pattern);

            if let Some(params) = match_segments(&pattern_segments, &request_segments) {
                return Some(RouteMatch {
                    route_index: idx,
                    params,
                    service: route.service.clone(),
                    auth_required: route.auth_required,
                    rate_limit: route.rate_limit.clone(),
                    matched_pattern: route.path_pattern.clone(),
                });
            }
        }

        None
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Split a path on `/`, filtering out empty segments.
fn split_path(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

/// Try to match `pattern` segments against `request` segments.
///
/// Returns extracted parameters on success.
fn match_segments<'a>(
    pattern: &[&str],
    request: &[&'a str],
) -> Option<HashMap<String, String>> {
    let mut params = HashMap::new();
    let mut p_idx = 0;

    for (i, &seg) in pattern.iter().enumerate() {
        if seg == "*" {
            // Wildcard matches the rest of the path.
            // Capture everything remaining as `*`.
            let rest: Vec<&str> = request[i..].to_vec();
            params.insert("*".to_string(), rest.join("/"));
            return Some(params);
        }

        if i >= request.len() {
            return None;
        }

        if let Some(name) = seg.strip_prefix(':') {
            params.insert(name.to_string(), request[i].to_string());
        } else if seg != request[i] {
            return None;
        }

        p_idx = i + 1;
    }

    // Pattern consumed but request has extra segments (and no wildcard).
    if p_idx != request.len() {
        return None;
    }

    Some(params)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn world_route() -> Route {
        Route {
            path_pattern: "/api/v1/worlds/:world_id".to_string(),
            method: HttpMethod::Get,
            service: ServiceTarget::WorldServer {
                zone_id: "default".to_string(),
            },
            auth_required: true,
            rate_limit: Some(RateLimitRule {
                requests_per_second: 10,
                burst: 20,
            }),
        }
    }

    fn health_route() -> Route {
        Route {
            path_pattern: "/api/v1/health".to_string(),
            method: HttpMethod::Get,
            service: ServiceTarget::AuthService,
            auth_required: false,
            rate_limit: None,
        }
    }

    fn ugc_wildcard_route() -> Route {
        Route {
            path_pattern: "/api/v1/ugc/*".to_string(),
            method: HttpMethod::Post,
            service: ServiceTarget::UgcService,
            auth_required: true,
            rate_limit: None,
        }
    }

    fn economy_route() -> Route {
        Route {
            path_pattern: "/api/v1/economy/transfer".to_string(),
            method: HttpMethod::Post,
            service: ServiceTarget::EconomyService,
            auth_required: true,
            rate_limit: Some(RateLimitRule {
                requests_per_second: 5,
                burst: 10,
            }),
        }
    }

    fn social_route() -> Route {
        Route {
            path_pattern: "/api/v1/social/friends/:user_id".to_string(),
            method: HttpMethod::Get,
            service: ServiceTarget::SocialService,
            auth_required: true,
            rate_limit: None,
        }
    }

    fn build_router() -> Router {
        let mut router = Router::new();
        router.add_route(health_route());
        router.add_route(world_route());
        router.add_route(ugc_wildcard_route());
        router.add_route(economy_route());
        router.add_route(social_route());
        router
    }

    #[test]
    fn exact_match() {
        let router = build_router();
        let m = router
            .match_request(HttpMethod::Get, "/api/v1/health")
            .expect("should match health route");
        assert_eq!(m.service, ServiceTarget::AuthService);
        assert!(!m.auth_required);
        assert!(m.params.is_empty());
    }

    #[test]
    fn param_extraction() {
        let router = build_router();
        let m = router
            .match_request(HttpMethod::Get, "/api/v1/worlds/abc123")
            .expect("should match world route");
        assert_eq!(m.params.get("world_id").unwrap(), "abc123");
        assert!(m.auth_required);
    }

    #[test]
    fn wildcard_match() {
        let router = build_router();
        let m = router
            .match_request(HttpMethod::Post, "/api/v1/ugc/uploads/images/avatar.png")
            .expect("should match ugc wildcard");
        assert_eq!(m.service, ServiceTarget::UgcService);
        assert_eq!(
            m.params.get("*").unwrap(),
            "uploads/images/avatar.png"
        );
    }

    #[test]
    fn method_mismatch() {
        let router = build_router();
        let m = router.match_request(HttpMethod::Post, "/api/v1/health");
        assert!(m.is_none(), "POST should not match GET-only route");
    }

    #[test]
    fn no_match_unknown_path() {
        let router = build_router();
        let m = router.match_request(HttpMethod::Get, "/api/v1/nonexistent");
        assert!(m.is_none());
    }

    #[test]
    fn first_match_wins() {
        let mut router = Router::new();
        // Two routes with same path but different services.
        router.add_route(Route {
            path_pattern: "/api/v1/test".to_string(),
            method: HttpMethod::Get,
            service: ServiceTarget::AuthService,
            auth_required: false,
            rate_limit: None,
        });
        router.add_route(Route {
            path_pattern: "/api/v1/test".to_string(),
            method: HttpMethod::Get,
            service: ServiceTarget::SocialService,
            auth_required: false,
            rate_limit: None,
        });
        let m = router
            .match_request(HttpMethod::Get, "/api/v1/test")
            .unwrap();
        assert_eq!(m.service, ServiceTarget::AuthService);
    }

    #[test]
    fn multiple_params() {
        let mut router = Router::new();
        router.add_route(Route {
            path_pattern: "/api/v1/worlds/:world_id/zones/:zone_id".to_string(),
            method: HttpMethod::Get,
            service: ServiceTarget::WorldServer {
                zone_id: "dynamic".to_string(),
            },
            auth_required: true,
            rate_limit: None,
        });

        let m = router
            .match_request(HttpMethod::Get, "/api/v1/worlds/w1/zones/z2")
            .unwrap();
        assert_eq!(m.params.get("world_id").unwrap(), "w1");
        assert_eq!(m.params.get("zone_id").unwrap(), "z2");
    }

    #[test]
    fn trailing_slash_ignored() {
        let router = build_router();
        let m = router.match_request(HttpMethod::Get, "/api/v1/health/");
        assert!(m.is_some(), "trailing slash should still match");
    }

    #[test]
    fn rate_limit_propagated() {
        let router = build_router();
        let m = router
            .match_request(HttpMethod::Post, "/api/v1/economy/transfer")
            .unwrap();
        let rl = m.rate_limit.as_ref().unwrap();
        assert_eq!(rl.requests_per_second, 5);
        assert_eq!(rl.burst, 10);
    }

    #[test]
    fn social_param_route() {
        let router = build_router();
        let m = router
            .match_request(HttpMethod::Get, "/api/v1/social/friends/42")
            .unwrap();
        assert_eq!(m.service, ServiceTarget::SocialService);
        assert_eq!(m.params.get("user_id").unwrap(), "42");
    }

    #[test]
    fn extra_segments_no_match() {
        let router = build_router();
        // health route is exact; extra segment should not match.
        let m = router.match_request(HttpMethod::Get, "/api/v1/health/extra");
        assert!(m.is_none());
    }

    #[test]
    fn default_trait() {
        let router = Router::default();
        assert!(router.routes().is_empty());
    }
}
