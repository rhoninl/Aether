//! Password hashing and verification using Argon2id.

use argon2::{
    password_hash::{
        rand_core::OsRng, PasswordHash, PasswordHasher as ArgonHasher, PasswordVerifier, SaltString,
    },
    Algorithm, Argon2, Params, Version,
};
use std::env;

/// Default Argon2 memory cost in KiB.
const DEFAULT_MEMORY_COST_KB: u32 = 65_536;

/// Default Argon2 time cost (iterations).
const DEFAULT_TIME_COST: u32 = 3;

/// Default Argon2 parallelism degree.
const DEFAULT_PARALLELISM: u32 = 1;

/// Environment variable for Argon2 memory cost.
const ENV_PASSWORD_HASH_MEMORY_KB: &str = "PASSWORD_HASH_MEMORY_KB";

/// Environment variable for Argon2 iteration count.
const ENV_PASSWORD_HASH_ITERATIONS: &str = "PASSWORD_HASH_ITERATIONS";

/// Configuration for password hashing.
#[derive(Debug, Clone)]
pub struct PasswordConfig {
    pub memory_cost_kb: u32,
    pub time_cost: u32,
    pub parallelism: u32,
}

impl PasswordConfig {
    /// Loads password hashing configuration from environment variables.
    pub fn from_env() -> Self {
        let memory_cost_kb = env::var(ENV_PASSWORD_HASH_MEMORY_KB)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MEMORY_COST_KB);
        let time_cost = env::var(ENV_PASSWORD_HASH_ITERATIONS)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_TIME_COST);

        Self {
            memory_cost_kb,
            time_cost,
            parallelism: DEFAULT_PARALLELISM,
        }
    }

    /// Creates a test-friendly configuration with minimal resource usage.
    pub fn for_testing() -> Self {
        Self {
            memory_cost_kb: 1024,
            time_cost: 1,
            parallelism: 1,
        }
    }
}

/// Errors that can occur during password operations.
#[derive(Debug)]
pub enum PasswordError {
    /// Password hashing failed.
    HashingFailed(String),
    /// Password verification failed (wrong password).
    VerificationFailed,
    /// Stored hash is malformed.
    InvalidHash(String),
}

impl std::fmt::Display for PasswordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PasswordError::HashingFailed(msg) => write!(f, "password hashing failed: {}", msg),
            PasswordError::VerificationFailed => write!(f, "password verification failed"),
            PasswordError::InvalidHash(msg) => write!(f, "invalid password hash: {}", msg),
        }
    }
}

impl std::error::Error for PasswordError {}

/// Handles password hashing and verification using Argon2id.
#[derive(Debug, Clone)]
pub struct PasswordHasher {
    config: PasswordConfig,
}

impl PasswordHasher {
    /// Creates a new PasswordHasher with the given configuration.
    pub fn new(config: PasswordConfig) -> Self {
        Self { config }
    }

    /// Creates a PasswordHasher using configuration from environment variables.
    pub fn from_env() -> Self {
        Self::new(PasswordConfig::from_env())
    }

    /// Creates a test-friendly PasswordHasher with minimal resource usage.
    pub fn for_testing() -> Self {
        Self::new(PasswordConfig::for_testing())
    }

    /// Hashes a password using Argon2id. Returns the PHC-format hash string.
    pub fn hash_password(&self, password: &str) -> Result<String, PasswordError> {
        let salt = SaltString::generate(&mut OsRng);
        let params = Params::new(
            self.config.memory_cost_kb,
            self.config.time_cost,
            self.config.parallelism,
            None,
        )
        .map_err(|e| PasswordError::HashingFailed(e.to_string()))?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| PasswordError::HashingFailed(e.to_string()))?;

        Ok(hash.to_string())
    }

    /// Verifies a password against a stored PHC-format hash.
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<(), PasswordError> {
        let parsed_hash =
            PasswordHash::new(hash).map_err(|e| PasswordError::InvalidHash(e.to_string()))?;

        // Verification uses the parameters embedded in the hash, not the config
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| PasswordError::VerificationFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hasher() -> PasswordHasher {
        PasswordHasher::for_testing()
    }

    #[test]
    fn test_hash_password_produces_phc_string() {
        let hasher = test_hasher();
        let hash = hasher.hash_password("my-secure-password").unwrap();
        // PHC format starts with $argon2id$
        assert!(hash.starts_with("$argon2id$"));
    }

    #[test]
    fn test_verify_correct_password() {
        let hasher = test_hasher();
        let hash = hasher.hash_password("correct-password").unwrap();
        assert!(hasher.verify_password("correct-password", &hash).is_ok());
    }

    #[test]
    fn test_reject_wrong_password() {
        let hasher = test_hasher();
        let hash = hasher.hash_password("correct-password").unwrap();
        let err = hasher.verify_password("wrong-password", &hash).unwrap_err();
        assert!(matches!(err, PasswordError::VerificationFailed));
    }

    #[test]
    fn test_different_hashes_for_same_password() {
        let hasher = test_hasher();
        let hash1 = hasher.hash_password("same-password").unwrap();
        let hash2 = hasher.hash_password("same-password").unwrap();
        // Different salts produce different hashes
        assert_ne!(hash1, hash2);
        // But both verify correctly
        assert!(hasher.verify_password("same-password", &hash1).is_ok());
        assert!(hasher.verify_password("same-password", &hash2).is_ok());
    }

    #[test]
    fn test_empty_password_hashes() {
        let hasher = test_hasher();
        let hash = hasher.hash_password("").unwrap();
        assert!(hasher.verify_password("", &hash).is_ok());
        assert!(hasher.verify_password("notempty", &hash).is_err());
    }

    #[test]
    fn test_long_password_hashes() {
        let hasher = test_hasher();
        let long_pw = "a".repeat(1000);
        let hash = hasher.hash_password(&long_pw).unwrap();
        assert!(hasher.verify_password(&long_pw, &hash).is_ok());
    }

    #[test]
    fn test_invalid_hash_string() {
        let hasher = test_hasher();
        let err = hasher
            .verify_password("password", "not-a-valid-hash")
            .unwrap_err();
        assert!(matches!(err, PasswordError::InvalidHash(_)));
    }

    #[test]
    fn test_password_error_display() {
        assert!(PasswordError::VerificationFailed
            .to_string()
            .contains("verification"));
        assert!(PasswordError::HashingFailed("x".into())
            .to_string()
            .contains("hashing"));
        assert!(PasswordError::InvalidHash("x".into())
            .to_string()
            .contains("invalid"));
    }

    #[test]
    fn test_password_config_for_testing() {
        let config = PasswordConfig::for_testing();
        assert_eq!(config.memory_cost_kb, 1024);
        assert_eq!(config.time_cost, 1);
        assert_eq!(config.parallelism, 1);
    }

    #[test]
    fn test_password_config_defaults() {
        env::remove_var(ENV_PASSWORD_HASH_MEMORY_KB);
        env::remove_var(ENV_PASSWORD_HASH_ITERATIONS);
        let config = PasswordConfig::from_env();
        assert_eq!(config.memory_cost_kb, DEFAULT_MEMORY_COST_KB);
        assert_eq!(config.time_cost, DEFAULT_TIME_COST);
    }

    #[test]
    fn test_unicode_password() {
        let hasher = test_hasher();
        let hash = hasher.hash_password("p@$$w0rd-!@#").unwrap();
        assert!(hasher.verify_password("p@$$w0rd-!@#", &hash).is_ok());
        assert!(hasher.verify_password("p@$$w0rd-!@$", &hash).is_err());
    }
}
