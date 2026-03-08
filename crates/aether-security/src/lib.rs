//! Security and anti-cheat helper primitives.

pub mod action_rate_limiter;
pub mod anti_cheat;
pub mod auth;
pub mod encryption;
pub mod hit_validation;
pub mod interest_management;
pub mod jwt;
pub mod movement_validator;
pub mod oauth;
pub mod password;
pub mod ratelimit;
pub mod session;
pub mod teleport_detection;
pub mod transport;
pub mod user;
pub mod wasm;

pub use action_rate_limiter::{ActionRateConfig, ActionRateLimiter, RateLimitResult};
pub use anti_cheat::{CheatSignal, CheatVerdict, InputPlausibility};
pub use auth::{AuthError, AuthService, LoginResult, RegisterResult, UserStore};
pub use encryption::{TlsMode, TransportSecurityPolicy};
pub use hit_validation::{
    HitClaim, HitResult, HitValidationConfig, ServerHitState, validate_hit,
};
pub use interest_management::{
    FilterReason, InterestConfig, InterestManager, TrackedEntity, Visibility, VisibilityResult,
};
pub use jwt::{Claims, JwtConfig, JwtError, JwtProvider, TokenPair};
pub use movement_validator::{
    MovementConfig, MovementResult, Vec3, validate_movement, validate_movement_with_acceleration,
};
pub use oauth::{
    DiscordOAuthProvider, GoogleOAuthProvider, OAuthConfig, OAuthProvider, OAuthUserInfo,
};
pub use password::{PasswordConfig, PasswordError, PasswordHasher};
pub use ratelimit::{ActionKey, RateLimit, RateLimitBucket};
pub use session::{InMemorySessionStore, Session, SessionError, SessionStore};
pub use teleport_detection::{TeleportConfig, TeleportDetector, TeleportResult};
pub use transport::{AttackSignal, ClientAddress, DdosDefenseState, FloodSignal, NetworkAction};
pub use user::{User, UserRole};
pub use wasm::{SandboxCapability, ScriptSandboxPolicy, WasmSandboxCapability, WasmSurfaceError};
