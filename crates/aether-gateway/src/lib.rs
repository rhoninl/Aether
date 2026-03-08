//! Edge gateway and voice relay contracts.

pub mod auth;
pub mod dispatch;
pub mod health;
pub mod metrics;
pub mod middleware;
pub mod rate;
pub mod relay;
pub mod route;
pub mod router;
pub mod runtime;

pub use auth::{AuthValidationPolicy, AuthzResult, Token};
pub use dispatch::{DispatchContext, DispatchError, Dispatcher};
pub use health::{HealthCheckConfig, HealthChecker, ServiceHealth, ServiceHealthState};
pub use metrics::{RequestMetrics, RouteMetricsSnapshot};
pub use middleware::{AuthMiddleware, RateLimiter};
pub use rate::{RateLimitPolicy, RateLimitStatus};
pub use relay::{NatMode, RelayProfile, RelayRegion, RelaySession};
pub use route::{GeoRoutingPolicy, RegionLatencyProfile, RouteId, RoutedRequest};
pub use router::{HttpMethod, RateLimitRule, Route, RouteMatch, Router, ServiceTarget};
pub use runtime::{
    BackendRuntime, BackendRuntimeConfig, BackendRuntimeState, BackendStepInput, BackendStepOutput,
};
