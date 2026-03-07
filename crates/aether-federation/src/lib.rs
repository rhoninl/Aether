//! Federation and self-hosted interoperability contracts.

pub mod asset;
pub mod auth;
pub mod registration;
pub mod trust;

pub use asset::{AssetIntegrityPolicy, FederationAssetReference, HashMismatchAction};
pub use auth::{AuthCheckMode, FederationAuthRequest, FederationAuthResult};
pub use registration::{RegistrationState, SelfHostedWorld};
pub use trust::{CentralServiceGate, ModerationResult, ModifiedSinceApproval};

