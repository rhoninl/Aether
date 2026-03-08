//! Federation and self-hosted interoperability contracts.

pub mod asset;
pub mod auth;
pub mod handshake;
pub mod health;
pub mod registration;
pub mod routing;
pub mod runtime;
pub mod server_registry;
pub mod trust;
pub mod verification;

pub use asset::{AssetIntegrityPolicy, FederationAssetReference, HashMismatchAction};
pub use auth::{AuthCheckMode, FederationAuthRequest, FederationAuthResult};
pub use handshake::{
    HandshakeChallenge, HandshakeComplete, HandshakeError, HandshakeManager, HandshakeResponse,
    HandshakeSession, HandshakeState,
};
pub use health::{HealthMonitor, HealthRecord, HealthStatus};
pub use registration::{RegistrationState, SelfHostedWorld};
pub use routing::{PortalRoute, ResolvedDestination, RoutingError, RoutingTable};
pub use runtime::{
    FederationRuntime, FederationRuntimeConfig, FederationRuntimeInput, FederationRuntimeOutput,
    FederationTransactionRequest, FederationTransactionResult,
};
pub use server_registry::{FederatedServer, RegistryError, ServerRegistry, ServerStatus};
pub use trust::{CentralServiceGate, ModerationResult, ModifiedSinceApproval};
pub use verification::{AssetVerification, VerificationResult};
