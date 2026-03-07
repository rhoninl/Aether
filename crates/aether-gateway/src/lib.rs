//! Edge gateway and voice relay contracts.

pub mod auth;
pub mod rate;
pub mod relay;
pub mod route;

pub use auth::{AuthValidationPolicy, AuthzResult, Token};
pub use rate::{RateLimitPolicy, RateLimitStatus, RouteId};
pub use relay::{NatMode, RelayProfile, RelayRegion, RelaySession};
pub use route::{GeoRoutingPolicy, RegionLatencyProfile, RoutedRequest};

