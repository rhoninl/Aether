//! Security and anti-cheat helper primitives.

pub mod anti_cheat;
pub mod auth;
pub mod encryption;
pub mod jwt;
pub mod oauth;
pub mod password;
pub mod ratelimit;
pub mod session;
pub mod transport;
pub mod user;
pub mod wasm;

pub use anti_cheat::{CheatSignal, CheatVerdict, InputPlausibility};
pub use auth::{AuthError, AuthService, LoginResult, RegisterResult, UserStore};
pub use encryption::{TlsMode, TransportSecurityPolicy};
pub use jwt::{Claims, JwtConfig, JwtError, JwtProvider, TokenPair};
pub use oauth::{
    DiscordOAuthProvider, GoogleOAuthProvider, OAuthConfig, OAuthProvider, OAuthUserInfo,
};
pub use password::{PasswordConfig, PasswordError, PasswordHasher};
pub use ratelimit::{ActionKey, RateLimit, RateLimitBucket};
pub use session::{InMemorySessionStore, Session, SessionError, SessionStore};
pub use transport::{AttackSignal, ClientAddress, DdosDefenseState, FloodSignal, NetworkAction};
pub use user::{User, UserRole};
pub use wasm::{SandboxCapability, ScriptSandboxPolicy, WasmSandboxCapability, WasmSurfaceError};
