//! JWT token creation and validation.

use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::env;
use uuid::Uuid;

use crate::user::UserRole;

/// Default JWT secret for development only.
const DEFAULT_JWT_SECRET: &str = "dev-secret-do-not-use-in-production";

/// Default access token expiry in seconds (1 hour).
const DEFAULT_JWT_EXPIRY_SECS: u64 = 3600;

/// Default refresh token expiry in seconds (7 days).
const DEFAULT_JWT_REFRESH_EXPIRY_SECS: u64 = 604_800;

/// Environment variable names for JWT configuration.
const ENV_JWT_SECRET: &str = "JWT_SECRET";
const ENV_JWT_EXPIRY_SECS: &str = "JWT_EXPIRY_SECS";
const ENV_JWT_REFRESH_EXPIRY_SECS: &str = "JWT_REFRESH_EXPIRY_SECS";

/// JWT claims embedded in access and refresh tokens.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Claims {
    /// Subject - the user ID.
    pub sub: String,
    /// User role.
    pub role: String,
    /// Expiration time (UTC timestamp).
    pub exp: usize,
    /// Issued-at time (UTC timestamp).
    pub iat: usize,
    /// Unique token identifier for revocation tracking.
    pub jti: String,
}

/// Configuration for JWT operations.
#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub access_expiry_secs: u64,
    pub refresh_expiry_secs: u64,
}

impl JwtConfig {
    /// Loads JWT configuration from environment variables with defaults.
    pub fn from_env() -> Self {
        let secret = env::var(ENV_JWT_SECRET).unwrap_or_else(|_| DEFAULT_JWT_SECRET.to_string());
        let access_expiry_secs = env::var(ENV_JWT_EXPIRY_SECS)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_JWT_EXPIRY_SECS);
        let refresh_expiry_secs = env::var(ENV_JWT_REFRESH_EXPIRY_SECS)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_JWT_REFRESH_EXPIRY_SECS);

        Self {
            secret,
            access_expiry_secs,
            refresh_expiry_secs,
        }
    }
}

/// Errors that can occur during JWT operations.
#[derive(Debug)]
pub enum JwtError {
    /// Token encoding failed.
    EncodingFailed(String),
    /// Token decoding/validation failed.
    ValidationFailed(String),
    /// Token has expired.
    Expired,
    /// Token signature is invalid.
    InvalidSignature,
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwtError::EncodingFailed(msg) => write!(f, "JWT encoding failed: {}", msg),
            JwtError::ValidationFailed(msg) => write!(f, "JWT validation failed: {}", msg),
            JwtError::Expired => write!(f, "JWT has expired"),
            JwtError::InvalidSignature => write!(f, "JWT signature is invalid"),
        }
    }
}

impl std::error::Error for JwtError {}

/// An access/refresh token pair returned after successful authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    /// Access token TTL in seconds.
    pub expires_in: u64,
}

/// Handles JWT token creation and validation.
#[derive(Clone)]
pub struct JwtProvider {
    config: JwtConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl std::fmt::Debug for JwtProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtProvider")
            .field("config", &self.config)
            .finish()
    }
}

impl JwtProvider {
    /// Creates a new JwtProvider with the given configuration.
    pub fn new(config: JwtConfig) -> Self {
        let encoding_key = EncodingKey::from_secret(config.secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(config.secret.as_bytes());
        Self {
            config,
            encoding_key,
            decoding_key,
        }
    }

    /// Creates a JwtProvider using configuration from environment variables.
    pub fn from_env() -> Self {
        Self::new(JwtConfig::from_env())
    }

    /// Creates an access token for the given user.
    pub fn create_access_token(&self, user_id: &Uuid, role: &UserRole) -> Result<String, JwtError> {
        self.create_token(user_id, role, self.config.access_expiry_secs)
    }

    /// Creates a refresh token for the given user.
    pub fn create_refresh_token(
        &self,
        user_id: &Uuid,
        role: &UserRole,
    ) -> Result<String, JwtError> {
        self.create_token(user_id, role, self.config.refresh_expiry_secs)
    }

    /// Creates a token pair (access + refresh) for the given user.
    pub fn create_token_pair(
        &self,
        user_id: &Uuid,
        role: &UserRole,
    ) -> Result<TokenPair, JwtError> {
        let access_token = self.create_access_token(user_id, role)?;
        let refresh_token = self.create_refresh_token(user_id, role)?;
        Ok(TokenPair {
            access_token,
            refresh_token,
            expires_in: self.config.access_expiry_secs,
        })
    }

    /// Validates a token and returns the decoded claims.
    pub fn validate_token(&self, token: &str) -> Result<Claims, JwtError> {
        let validation = Validation::new(jsonwebtoken::Algorithm::HS256);
        let token_data = decode::<Claims>(token, &self.decoding_key, &validation).map_err(|e| {
            match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::Expired,
                jsonwebtoken::errors::ErrorKind::InvalidSignature => JwtError::InvalidSignature,
                _ => JwtError::ValidationFailed(e.to_string()),
            }
        })?;
        Ok(token_data.claims)
    }

    /// Returns the access token expiry in seconds.
    pub fn access_expiry_secs(&self) -> u64 {
        self.config.access_expiry_secs
    }

    fn create_token(
        &self,
        user_id: &Uuid,
        role: &UserRole,
        expiry_secs: u64,
    ) -> Result<String, JwtError> {
        let now = Utc::now().timestamp() as usize;
        let claims = Claims {
            sub: user_id.to_string(),
            role: role.to_string(),
            exp: now + expiry_secs as usize,
            iat: now,
            jti: Uuid::new_v4().to_string(),
        };
        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| JwtError::EncodingFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> JwtConfig {
        JwtConfig {
            secret: "test-secret-key-for-unit-tests".to_string(),
            access_expiry_secs: 3600,
            refresh_expiry_secs: 604_800,
        }
    }

    fn test_provider() -> JwtProvider {
        JwtProvider::new(test_config())
    }

    #[test]
    fn test_create_access_token() {
        let provider = test_provider();
        let user_id = Uuid::new_v4();
        let token = provider
            .create_access_token(&user_id, &UserRole::User)
            .unwrap();
        assert!(!token.is_empty());
        // JWT has 3 parts separated by dots
        assert_eq!(token.split('.').count(), 3);
    }

    #[test]
    fn test_create_refresh_token() {
        let provider = test_provider();
        let user_id = Uuid::new_v4();
        let token = provider
            .create_refresh_token(&user_id, &UserRole::Admin)
            .unwrap();
        assert!(!token.is_empty());
    }

    #[test]
    fn test_create_token_pair() {
        let provider = test_provider();
        let user_id = Uuid::new_v4();
        let pair = provider
            .create_token_pair(&user_id, &UserRole::Moderator)
            .unwrap();
        assert!(!pair.access_token.is_empty());
        assert!(!pair.refresh_token.is_empty());
        assert_ne!(pair.access_token, pair.refresh_token);
        assert_eq!(pair.expires_in, 3600);
    }

    #[test]
    fn test_validate_valid_token() {
        let provider = test_provider();
        let user_id = Uuid::new_v4();
        let token = provider
            .create_access_token(&user_id, &UserRole::User)
            .unwrap();
        let claims = provider.validate_token(&token).unwrap();
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.role, "user");
        assert!(!claims.jti.is_empty());
    }

    #[test]
    fn test_validate_extracts_correct_role() {
        let provider = test_provider();
        let user_id = Uuid::new_v4();

        let token_user = provider
            .create_access_token(&user_id, &UserRole::User)
            .unwrap();
        assert_eq!(provider.validate_token(&token_user).unwrap().role, "user");

        let token_mod = provider
            .create_access_token(&user_id, &UserRole::Moderator)
            .unwrap();
        assert_eq!(
            provider.validate_token(&token_mod).unwrap().role,
            "moderator"
        );

        let token_admin = provider
            .create_access_token(&user_id, &UserRole::Admin)
            .unwrap();
        assert_eq!(provider.validate_token(&token_admin).unwrap().role, "admin");
    }

    #[test]
    fn test_reject_expired_token() {
        let provider = test_provider();
        // Manually create a token with exp in the past
        let past = Utc::now().timestamp() as usize - 3600;
        let claims = Claims {
            sub: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            exp: past,
            iat: past - 3600,
            jti: Uuid::new_v4().to_string(),
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"test-secret-key-for-unit-tests"),
        )
        .unwrap();

        let err = provider.validate_token(&token).unwrap_err();
        assert!(matches!(err, JwtError::Expired));
    }

    #[test]
    fn test_reject_tampered_token() {
        let provider = test_provider();
        let user_id = Uuid::new_v4();
        let token = provider
            .create_access_token(&user_id, &UserRole::User)
            .unwrap();

        // Tamper with the token by modifying a character in the signature
        let mut parts: Vec<&str> = token.split('.').collect();
        let sig = parts[2].to_string();
        let tampered_sig = if sig.starts_with('a') {
            format!("b{}", &sig[1..])
        } else {
            format!("a{}", &sig[1..])
        };
        parts[2] = &tampered_sig;
        let tampered_token = parts.join(".");

        let err = provider.validate_token(&tampered_token).unwrap_err();
        assert!(matches!(
            err,
            JwtError::InvalidSignature | JwtError::ValidationFailed(_)
        ));
    }

    #[test]
    fn test_reject_wrong_secret() {
        let provider1 = JwtProvider::new(JwtConfig {
            secret: "secret-one".to_string(),
            access_expiry_secs: 3600,
            refresh_expiry_secs: 604_800,
        });
        let provider2 = JwtProvider::new(JwtConfig {
            secret: "secret-two".to_string(),
            access_expiry_secs: 3600,
            refresh_expiry_secs: 604_800,
        });

        let user_id = Uuid::new_v4();
        let token = provider1
            .create_access_token(&user_id, &UserRole::User)
            .unwrap();

        let err = provider2.validate_token(&token).unwrap_err();
        assert!(matches!(
            err,
            JwtError::InvalidSignature | JwtError::ValidationFailed(_)
        ));
    }

    #[test]
    fn test_claims_have_unique_jti() {
        let provider = test_provider();
        let user_id = Uuid::new_v4();
        let t1 = provider
            .create_access_token(&user_id, &UserRole::User)
            .unwrap();
        let t2 = provider
            .create_access_token(&user_id, &UserRole::User)
            .unwrap();
        let c1 = provider.validate_token(&t1).unwrap();
        let c2 = provider.validate_token(&t2).unwrap();
        assert_ne!(c1.jti, c2.jti);
    }

    #[test]
    fn test_claims_iat_is_recent() {
        let provider = test_provider();
        let user_id = Uuid::new_v4();
        let token = provider
            .create_access_token(&user_id, &UserRole::User)
            .unwrap();
        let claims = provider.validate_token(&token).unwrap();
        let now = Utc::now().timestamp() as usize;
        // iat should be within 5 seconds of now
        assert!(claims.iat <= now);
        assert!(now - claims.iat < 5);
    }

    #[test]
    fn test_jwt_config_defaults() {
        // Clear env vars to test defaults
        env::remove_var(ENV_JWT_SECRET);
        env::remove_var(ENV_JWT_EXPIRY_SECS);
        env::remove_var(ENV_JWT_REFRESH_EXPIRY_SECS);

        let config = JwtConfig::from_env();
        assert_eq!(config.secret, DEFAULT_JWT_SECRET);
        assert_eq!(config.access_expiry_secs, DEFAULT_JWT_EXPIRY_SECS);
        assert_eq!(config.refresh_expiry_secs, DEFAULT_JWT_REFRESH_EXPIRY_SECS);
    }

    #[test]
    fn test_jwt_error_display() {
        assert!(JwtError::Expired.to_string().contains("expired"));
        assert!(JwtError::InvalidSignature.to_string().contains("signature"));
        assert!(JwtError::EncodingFailed("x".into())
            .to_string()
            .contains("encoding"));
        assert!(JwtError::ValidationFailed("x".into())
            .to_string()
            .contains("validation"));
    }

    #[test]
    fn test_validate_garbage_token() {
        let provider = test_provider();
        let err = provider.validate_token("not.a.jwt").unwrap_err();
        assert!(matches!(
            err,
            JwtError::ValidationFailed(_) | JwtError::InvalidSignature
        ));
    }
}
