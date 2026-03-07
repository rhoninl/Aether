//! Federation and self-hosted interoperability contracts.

pub mod asset;
pub mod auth;
pub mod registration;
pub mod trust;
pub mod runtime;

pub use asset::{AssetIntegrityPolicy, FederationAssetReference, HashMismatchAction};
pub use auth::{AuthCheckMode, FederationAuthRequest, FederationAuthResult};
pub use registration::{RegistrationState, SelfHostedWorld};
pub use trust::{CentralServiceGate, ModerationResult, ModifiedSinceApproval};
pub use runtime::{
    FederationRuntime, FederationRuntimeConfig, FederationRuntimeInput, FederationRuntimeOutput, FederationTransactionRequest,
    FederationTransactionResult,
};
