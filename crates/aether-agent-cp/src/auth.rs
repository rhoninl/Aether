//! Agent service-account authentication.
//!
//! Every MCP / gRPC request carries an `Authorization: Bearer <jwt>` header
//! (or an `auth` field inside the JSON-RPC envelope for transports where a
//! transport-level header is awkward, e.g. stdio).
//!
//! This module owns the minimal JWT validation surface this crate needs. Once
//! the shared `aether-security` JWT primitives land in this worktree it can be
//! swapped to re-export them; for now it wraps `jsonwebtoken` directly so the
//! crate builds standalone. The claim layout (`sub`, `role`, `exp`, `iat`,
//! `jti`) matches `services/identity` exactly.

use std::env;
use std::sync::Arc;

use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

use crate::error::{codes, ToolError};

/// Default identity service JWKS URL. Matches
/// `services/identity/cmd/server/main.go`'s `/auth/.well-known/jwks.json`.
pub const DEFAULT_IDENTITY_JWKS_URL: &str = "http://identity:8080/auth/.well-known/jwks.json";

/// Default HS256 secret for dev. Production operators MUST override
/// `AETHER_AGENT_CP_JWT_SECRET` (falls back to `JWT_SECRET` to match
/// `services/identity`).
const DEFAULT_JWT_SECRET: &str = "dev-secret-do-not-use-in-production";

/// Environment variable names consumed by the auth verifier.
pub const ENV_IDENTITY_JWKS_URL: &str = "AETHER_AGENT_CP_IDENTITY_JWKS_URL";
/// Optional override for the bearer-token role check. When set, the
/// verifier will require the token's `role` claim to equal this string.
pub const ENV_REQUIRED_ROLE: &str = "AETHER_AGENT_CP_REQUIRED_ROLE";
/// Primary HS256 secret env var. `JWT_SECRET` is the fallback (shared with
/// `services/identity`).
pub const ENV_JWT_SECRET: &str = "AETHER_AGENT_CP_JWT_SECRET";
pub const ENV_JWT_SECRET_FALLBACK: &str = "JWT_SECRET";

/// Claims layout. Must match what the identity service issues.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Claims {
    pub sub: String,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
    pub jti: String,
}

/// Auth verifier configuration.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// URL of the identity service's JWKS endpoint. Stored for future asymmetric-key flows.
    pub identity_jwks_url: String,
    /// Optional role the caller must carry (e.g. `agent`). When `None`, any valid token is accepted.
    pub required_role: Option<String>,
    /// Shared HS256 secret used to validate bearer tokens.
    pub hs256_secret: String,
}

impl AuthConfig {
    pub fn from_env() -> Self {
        let identity_jwks_url =
            env::var(ENV_IDENTITY_JWKS_URL).unwrap_or_else(|_| DEFAULT_IDENTITY_JWKS_URL.into());
        let required_role = env::var(ENV_REQUIRED_ROLE).ok().filter(|s| !s.is_empty());
        let hs256_secret = env::var(ENV_JWT_SECRET)
            .or_else(|_| env::var(ENV_JWT_SECRET_FALLBACK))
            .unwrap_or_else(|_| DEFAULT_JWT_SECRET.into());
        Self {
            identity_jwks_url,
            required_role,
            hs256_secret,
        }
    }
}

/// Validates bearer tokens issued by the identity service. Clone-safe / cheap.
#[derive(Clone)]
pub struct AuthVerifier {
    inner: Arc<AuthVerifierInner>,
}

struct AuthVerifierInner {
    config: AuthConfig,
    key: DecodingKey,
    validation: Validation,
}

impl AuthVerifier {
    pub fn new(config: AuthConfig) -> Self {
        let key = DecodingKey::from_secret(config.hs256_secret.as_bytes());
        let validation = Validation::new(jsonwebtoken::Algorithm::HS256);
        Self {
            inner: Arc::new(AuthVerifierInner {
                config,
                key,
                validation,
            }),
        }
    }

    /// Build a verifier using configuration from environment variables.
    pub fn from_env() -> Self {
        Self::new(AuthConfig::from_env())
    }

    /// The configured JWKS URL (for introspection + banner logging).
    pub fn jwks_url(&self) -> &str {
        &self.inner.config.identity_jwks_url
    }

    /// Extract a bearer token from an `Authorization` header value.
    pub fn parse_bearer(header: &str) -> Option<&str> {
        let header = header.trim();
        let prefix = "Bearer ";
        if header.len() > prefix.len() && header[..prefix.len()].eq_ignore_ascii_case(prefix) {
            Some(header[prefix.len()..].trim())
        } else {
            None
        }
    }

    /// Validate a raw JWT and, on success, return its claims.
    pub fn validate(&self, token: &str) -> Result<Claims, ToolError> {
        let token_data =
            decode::<Claims>(token, &self.inner.key, &self.inner.validation).map_err(|e| {
                let msg = match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                        "bearer token has expired"
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidSignature => {
                        "bearer token signature is invalid"
                    }
                    _ => "bearer token failed validation",
                };
                ToolError::new(codes::UNAUTHORIZED, msg).suggest(format!(
                    "re-issue via the identity service at {}",
                    self.inner.config.identity_jwks_url
                ))
            })?;
        if let Some(required) = &self.inner.config.required_role {
            if &token_data.claims.role != required {
                return Err(ToolError::new(
                    codes::UNAUTHORIZED,
                    format!(
                        "bearer token role `{}` does not match required `{}`",
                        token_data.claims.role, required
                    ),
                ));
            }
        }
        Ok(token_data.claims)
    }
}

// ---------------------------------------------------------------------------
// Test-only helper: issue tokens with the same algorithm / secret.
// ---------------------------------------------------------------------------

#[cfg(any(test, feature = "test-tokens"))]
pub mod test_support {
    use super::Claims;
    use chrono::Utc;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use uuid::Uuid;

    /// Issue an HS256-signed token for the given secret + role. Only built in
    /// tests (or when the `test-tokens` feature is enabled).
    pub fn mint_token(secret: &str, role: &str, ttl_secs: i64) -> String {
        let now = Utc::now().timestamp() as usize;
        let claims = Claims {
            sub: Uuid::new_v4().to_string(),
            role: role.to_string(),
            exp: now + ttl_secs as usize,
            iat: now,
            jti: Uuid::new_v4().to_string(),
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("sign test token")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Tests that mutate process-wide env vars must not overlap.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn cfg(role: Option<&str>) -> AuthConfig {
        AuthConfig {
            identity_jwks_url: "http://identity/.well-known/jwks.json".into(),
            required_role: role.map(str::to_owned),
            hs256_secret: "agent-cp-unit-test-secret".into(),
        }
    }

    #[test]
    fn parse_bearer_accepts_canonical_form() {
        assert_eq!(
            AuthVerifier::parse_bearer("Bearer abc.def.ghi"),
            Some("abc.def.ghi")
        );
    }

    #[test]
    fn parse_bearer_is_case_insensitive() {
        assert_eq!(AuthVerifier::parse_bearer("bearer abc"), Some("abc"));
        assert_eq!(AuthVerifier::parse_bearer("BEARER xyz"), Some("xyz"));
    }

    #[test]
    fn parse_bearer_rejects_malformed() {
        assert!(AuthVerifier::parse_bearer("Token abc").is_none());
        assert!(AuthVerifier::parse_bearer("").is_none());
        assert!(AuthVerifier::parse_bearer("Bearer").is_none());
    }

    #[test]
    fn validate_accepts_valid_token() {
        let c = cfg(None);
        let token = test_support::mint_token(&c.hs256_secret, "user", 60);
        let v = AuthVerifier::new(c);
        let claims = v.validate(&token).unwrap();
        assert_eq!(claims.role, "user");
    }

    #[test]
    fn validate_rejects_garbage() {
        let v = AuthVerifier::new(cfg(None));
        let err = v.validate("not-a-jwt").unwrap_err();
        assert_eq!(err.code, codes::UNAUTHORIZED);
    }

    #[test]
    fn validate_rejects_wrong_role() {
        let c = cfg(Some("agent"));
        let token = test_support::mint_token(&c.hs256_secret, "user", 60);
        let v = AuthVerifier::new(c);
        let err = v.validate(&token).unwrap_err();
        assert_eq!(err.code, codes::UNAUTHORIZED);
        assert!(err.message.contains("role"));
    }

    #[test]
    fn auth_config_reads_env() {
        let _g = ENV_LOCK.lock().unwrap();
        env::set_var(ENV_IDENTITY_JWKS_URL, "http://custom/.well-known/jwks.json");
        env::set_var(ENV_REQUIRED_ROLE, "agent");
        env::set_var(ENV_JWT_SECRET, "explicit-secret");
        let cfg = AuthConfig::from_env();
        assert_eq!(cfg.identity_jwks_url, "http://custom/.well-known/jwks.json");
        assert_eq!(cfg.required_role.as_deref(), Some("agent"));
        assert_eq!(cfg.hs256_secret, "explicit-secret");
        env::remove_var(ENV_IDENTITY_JWKS_URL);
        env::remove_var(ENV_REQUIRED_ROLE);
        env::remove_var(ENV_JWT_SECRET);
    }

    #[test]
    fn auth_config_default_jwks_url() {
        let _g = ENV_LOCK.lock().unwrap();
        env::remove_var(ENV_IDENTITY_JWKS_URL);
        env::remove_var(ENV_REQUIRED_ROLE);
        env::remove_var(ENV_JWT_SECRET);
        env::remove_var(ENV_JWT_SECRET_FALLBACK);
        let cfg = AuthConfig::from_env();
        assert_eq!(cfg.identity_jwks_url, DEFAULT_IDENTITY_JWKS_URL);
        assert!(cfg.required_role.is_none());
    }
}
