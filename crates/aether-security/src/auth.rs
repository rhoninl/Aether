//! Auth service orchestration tying together JWT, password, and session management.

use uuid::Uuid;

use crate::jwt::{Claims, JwtConfig, JwtError, JwtProvider, TokenPair};
use crate::password::{PasswordError, PasswordHasher};
use crate::session::{InMemorySessionStore, SessionError, SessionStore};
use crate::user::User;

/// Errors from the auth service.
#[derive(Debug)]
pub enum AuthError {
    /// Invalid credentials (wrong email/password).
    InvalidCredentials,
    /// User not found.
    UserNotFound,
    /// User already exists.
    UserAlreadyExists,
    /// No password set on account (OAuth-only user).
    NoPasswordSet,
    /// JWT error.
    Jwt(JwtError),
    /// Password error.
    Password(PasswordError),
    /// Session error.
    Session(SessionError),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidCredentials => write!(f, "invalid credentials"),
            AuthError::UserNotFound => write!(f, "user not found"),
            AuthError::UserAlreadyExists => write!(f, "user already exists"),
            AuthError::NoPasswordSet => write!(f, "no password set on account"),
            AuthError::Jwt(e) => write!(f, "JWT error: {}", e),
            AuthError::Password(e) => write!(f, "password error: {}", e),
            AuthError::Session(e) => write!(f, "session error: {}", e),
        }
    }
}

impl std::error::Error for AuthError {}

impl From<JwtError> for AuthError {
    fn from(e: JwtError) -> Self {
        AuthError::Jwt(e)
    }
}

impl From<PasswordError> for AuthError {
    fn from(e: PasswordError) -> Self {
        AuthError::Password(e)
    }
}

impl From<SessionError> for AuthError {
    fn from(e: SessionError) -> Self {
        AuthError::Session(e)
    }
}

/// Result of a successful registration.
#[derive(Debug)]
pub struct RegisterResult {
    pub user: User,
    pub tokens: TokenPair,
}

/// Result of a successful login.
#[derive(Debug)]
pub struct LoginResult {
    pub user_id: Uuid,
    pub tokens: TokenPair,
}

/// Trait for user storage backends used by the auth service.
pub trait UserStore: Send + Sync {
    /// Finds a user by email. Returns `None` if not found.
    fn find_by_email(&self, email: &str) -> Option<User>;

    /// Finds a user by ID. Returns `None` if not found.
    fn find_by_id(&self, id: &Uuid) -> Option<User>;

    /// Stores a new user. Returns error if email already exists.
    fn create(&self, user: &User) -> Result<(), AuthError>;
}

/// In-memory user store for testing.
#[derive(Debug)]
pub struct InMemoryUserStore {
    users: std::sync::Mutex<Vec<User>>,
}

impl InMemoryUserStore {
    pub fn new() -> Self {
        Self {
            users: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl Default for InMemoryUserStore {
    fn default() -> Self {
        Self::new()
    }
}

impl UserStore for InMemoryUserStore {
    fn find_by_email(&self, email: &str) -> Option<User> {
        let users = self.users.lock().ok()?;
        users.iter().find(|u| u.email == email).cloned()
    }

    fn find_by_id(&self, id: &Uuid) -> Option<User> {
        let users = self.users.lock().ok()?;
        users.iter().find(|u| u.id == *id).cloned()
    }

    fn create(&self, user: &User) -> Result<(), AuthError> {
        let mut users = self
            .users
            .lock()
            .map_err(|_| AuthError::UserAlreadyExists)?;
        if users.iter().any(|u| u.email == user.email) {
            return Err(AuthError::UserAlreadyExists);
        }
        users.push(user.clone());
        Ok(())
    }
}

/// The main authentication service orchestrator.
pub struct AuthService<U: UserStore, S: SessionStore> {
    jwt_provider: JwtProvider,
    password_hasher: PasswordHasher,
    user_store: U,
    session_store: S,
}

impl<U: UserStore, S: SessionStore> AuthService<U, S> {
    /// Creates a new AuthService with all its dependencies.
    pub fn new(
        jwt_provider: JwtProvider,
        password_hasher: PasswordHasher,
        user_store: U,
        session_store: S,
    ) -> Self {
        Self {
            jwt_provider,
            password_hasher,
            user_store,
            session_store,
        }
    }

    /// Registers a new user with email and password.
    pub fn register(
        &self,
        email: String,
        display_name: String,
        password: &str,
    ) -> Result<RegisterResult, AuthError> {
        // Check if user already exists
        if self.user_store.find_by_email(&email).is_some() {
            return Err(AuthError::UserAlreadyExists);
        }

        // Hash password
        let password_hash = self.password_hasher.hash_password(password)?;

        // Create user
        let mut user = User::new(email, display_name);
        user.password_hash = Some(password_hash);
        self.user_store.create(&user)?;

        // Issue tokens
        let tokens = self.create_session_and_tokens(&user)?;

        Ok(RegisterResult { user, tokens })
    }

    /// Authenticates a user with email and password.
    pub fn login(&self, email: &str, password: &str) -> Result<LoginResult, AuthError> {
        let user = self
            .user_store
            .find_by_email(email)
            .ok_or(AuthError::InvalidCredentials)?;

        let stored_hash = user
            .password_hash
            .as_ref()
            .ok_or(AuthError::NoPasswordSet)?;

        self.password_hasher
            .verify_password(password, stored_hash)
            .map_err(|_| AuthError::InvalidCredentials)?;

        let tokens = self.create_session_and_tokens(&user)?;

        Ok(LoginResult {
            user_id: user.id,
            tokens,
        })
    }

    /// Refreshes tokens using a valid refresh token.
    /// Implements refresh token rotation: old session is revoked, new one created.
    pub fn refresh(&self, refresh_token: &str) -> Result<TokenPair, AuthError> {
        // Find and validate the session
        let session = self.session_store.find_by_refresh_token(refresh_token)?;

        // Revoke old session (rotation)
        self.session_store.revoke(session.id)?;

        // Look up user to get current role
        let user = self
            .user_store
            .find_by_id(&session.user_id)
            .ok_or(AuthError::UserNotFound)?;

        // Create new session and tokens
        self.create_session_and_tokens(&user)
    }

    /// Logs out by revoking the session associated with the given refresh token.
    pub fn logout(&self, refresh_token: &str) -> Result<(), AuthError> {
        let session = self.session_store.find_by_refresh_token(refresh_token)?;
        self.session_store.revoke(session.id)?;
        Ok(())
    }

    /// Logs out all sessions for a user.
    pub fn logout_all(&self, user_id: Uuid) -> Result<u64, AuthError> {
        let count = self.session_store.revoke_all_for_user(user_id)?;
        Ok(count)
    }

    /// Validates an access token and returns the claims.
    pub fn validate(&self, access_token: &str) -> Result<Claims, AuthError> {
        let claims = self.jwt_provider.validate_token(access_token)?;
        Ok(claims)
    }

    fn create_session_and_tokens(&self, user: &User) -> Result<TokenPair, AuthError> {
        let token_pair = self.jwt_provider.create_token_pair(&user.id, &user.role)?;

        // Store session with the refresh token
        self.session_store.create(
            user.id,
            &token_pair.refresh_token,
            self.jwt_provider.access_expiry_secs() * 7, // refresh token lives longer
        )?;

        Ok(token_pair)
    }
}

/// Creates a test-friendly AuthService with in-memory stores.
pub fn create_test_auth_service() -> AuthService<InMemoryUserStore, InMemorySessionStore> {
    let jwt_config = JwtConfig {
        secret: "test-auth-service-secret".to_string(),
        access_expiry_secs: 3600,
        refresh_expiry_secs: 604_800,
    };

    AuthService::new(
        JwtProvider::new(jwt_config),
        PasswordHasher::for_testing(),
        InMemoryUserStore::new(),
        InMemorySessionStore::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user::UserRole;

    #[test]
    fn test_register_new_user() {
        let service = create_test_auth_service();
        let result = service
            .register(
                "alice@example.com".to_string(),
                "Alice".to_string(),
                "strong-password-123",
            )
            .unwrap();

        assert_eq!(result.user.email, "alice@example.com");
        assert_eq!(result.user.display_name, "Alice");
        assert_eq!(result.user.role, UserRole::User);
        assert!(result.user.password_hash.is_some());
        assert!(!result.tokens.access_token.is_empty());
        assert!(!result.tokens.refresh_token.is_empty());
    }

    #[test]
    fn test_register_duplicate_email() {
        let service = create_test_auth_service();
        service
            .register(
                "alice@example.com".to_string(),
                "Alice".to_string(),
                "pass1",
            )
            .unwrap();

        let err = service
            .register(
                "alice@example.com".to_string(),
                "Alice2".to_string(),
                "pass2",
            )
            .unwrap_err();

        assert!(matches!(err, AuthError::UserAlreadyExists));
    }

    #[test]
    fn test_login_success() {
        let service = create_test_auth_service();
        service
            .register(
                "bob@example.com".to_string(),
                "Bob".to_string(),
                "bobs-password",
            )
            .unwrap();

        let result = service.login("bob@example.com", "bobs-password").unwrap();
        assert!(!result.tokens.access_token.is_empty());
        assert!(!result.tokens.refresh_token.is_empty());
    }

    #[test]
    fn test_login_wrong_password() {
        let service = create_test_auth_service();
        service
            .register(
                "carol@example.com".to_string(),
                "Carol".to_string(),
                "correct-password",
            )
            .unwrap();

        let err = service
            .login("carol@example.com", "wrong-password")
            .unwrap_err();
        assert!(matches!(err, AuthError::InvalidCredentials));
    }

    #[test]
    fn test_login_nonexistent_user() {
        let service = create_test_auth_service();
        let err = service
            .login("nobody@example.com", "password")
            .unwrap_err();
        assert!(matches!(err, AuthError::InvalidCredentials));
    }

    #[test]
    fn test_refresh_tokens() {
        let service = create_test_auth_service();
        let reg = service
            .register(
                "dave@example.com".to_string(),
                "Dave".to_string(),
                "daves-password",
            )
            .unwrap();

        let new_tokens = service.refresh(&reg.tokens.refresh_token).unwrap();
        assert!(!new_tokens.access_token.is_empty());
        assert!(!new_tokens.refresh_token.is_empty());
        // Old and new tokens should be different
        assert_ne!(new_tokens.access_token, reg.tokens.access_token);
        assert_ne!(new_tokens.refresh_token, reg.tokens.refresh_token);
    }

    #[test]
    fn test_refresh_revokes_old_session() {
        let service = create_test_auth_service();
        let reg = service
            .register(
                "eve@example.com".to_string(),
                "Eve".to_string(),
                "eves-password",
            )
            .unwrap();

        // First refresh succeeds
        let _new_tokens = service.refresh(&reg.tokens.refresh_token).unwrap();

        // Using the old refresh token again should fail (session was revoked)
        let err = service.refresh(&reg.tokens.refresh_token).unwrap_err();
        assert!(matches!(err, AuthError::Session(SessionError::Revoked)));
    }

    #[test]
    fn test_refresh_with_invalid_token() {
        let service = create_test_auth_service();
        let err = service.refresh("totally-invalid-token").unwrap_err();
        assert!(matches!(err, AuthError::Session(SessionError::NotFound)));
    }

    #[test]
    fn test_logout() {
        let service = create_test_auth_service();
        let reg = service
            .register(
                "frank@example.com".to_string(),
                "Frank".to_string(),
                "franks-password",
            )
            .unwrap();

        service.logout(&reg.tokens.refresh_token).unwrap();

        // Refresh should fail after logout
        let err = service.refresh(&reg.tokens.refresh_token).unwrap_err();
        assert!(matches!(err, AuthError::Session(SessionError::Revoked)));
    }

    #[test]
    fn test_logout_all() {
        let service = create_test_auth_service();
        let reg = service
            .register(
                "grace@example.com".to_string(),
                "Grace".to_string(),
                "graces-password",
            )
            .unwrap();

        // Login again to create a second session
        let login = service.login("grace@example.com", "graces-password").unwrap();

        let count = service.logout_all(reg.user.id).unwrap();
        assert_eq!(count, 2);

        // Both sessions should be revoked
        assert!(service.refresh(&reg.tokens.refresh_token).is_err());
        assert!(service.refresh(&login.tokens.refresh_token).is_err());
    }

    #[test]
    fn test_validate_access_token() {
        let service = create_test_auth_service();
        let reg = service
            .register(
                "heidi@example.com".to_string(),
                "Heidi".to_string(),
                "heidis-password",
            )
            .unwrap();

        let claims = service.validate(&reg.tokens.access_token).unwrap();
        assert_eq!(claims.sub, reg.user.id.to_string());
        assert_eq!(claims.role, "user");
    }

    #[test]
    fn test_validate_invalid_access_token() {
        let service = create_test_auth_service();
        let err = service.validate("bad-token").unwrap_err();
        assert!(matches!(err, AuthError::Jwt(_)));
    }

    #[test]
    fn test_register_returns_valid_tokens() {
        let service = create_test_auth_service();
        let reg = service
            .register(
                "ivan@example.com".to_string(),
                "Ivan".to_string(),
                "ivans-password",
            )
            .unwrap();

        // Access token should be valid
        let claims = service.validate(&reg.tokens.access_token).unwrap();
        assert_eq!(claims.sub, reg.user.id.to_string());

        // Refresh token should work
        let refreshed = service.refresh(&reg.tokens.refresh_token).unwrap();
        assert!(!refreshed.access_token.is_empty());
    }

    #[test]
    fn test_full_auth_flow() {
        let service = create_test_auth_service();

        // 1. Register
        let reg = service
            .register(
                "judy@example.com".to_string(),
                "Judy".to_string(),
                "judys-password",
            )
            .unwrap();

        // 2. Validate access token
        let claims = service.validate(&reg.tokens.access_token).unwrap();
        assert_eq!(claims.role, "user");

        // 3. Refresh tokens
        let refreshed = service.refresh(&reg.tokens.refresh_token).unwrap();

        // 4. Validate new access token
        let new_claims = service.validate(&refreshed.access_token).unwrap();
        assert_eq!(new_claims.sub, reg.user.id.to_string());

        // 5. Logout
        service.logout(&refreshed.refresh_token).unwrap();

        // 6. Can still login again
        let login = service.login("judy@example.com", "judys-password").unwrap();
        assert!(!login.tokens.access_token.is_empty());

        // 7. Logout all
        service.logout_all(reg.user.id).unwrap();
    }

    #[test]
    fn test_auth_error_display() {
        assert!(AuthError::InvalidCredentials
            .to_string()
            .contains("invalid credentials"));
        assert!(AuthError::UserNotFound
            .to_string()
            .contains("user not found"));
        assert!(AuthError::UserAlreadyExists
            .to_string()
            .contains("already exists"));
        assert!(AuthError::NoPasswordSet
            .to_string()
            .contains("no password"));
    }

    #[test]
    fn test_in_memory_user_store_find_by_id() {
        let store = InMemoryUserStore::new();
        let user = User::new("test@example.com".to_string(), "Test".to_string());
        let user_id = user.id;
        store.create(&user).unwrap();

        let found = store.find_by_id(&user_id).unwrap();
        assert_eq!(found.email, "test@example.com");
    }

    #[test]
    fn test_in_memory_user_store_find_by_id_not_found() {
        let store = InMemoryUserStore::new();
        assert!(store.find_by_id(&Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_in_memory_user_store_find_by_email_not_found() {
        let store = InMemoryUserStore::new();
        assert!(store.find_by_email("none@example.com").is_none());
    }
}
