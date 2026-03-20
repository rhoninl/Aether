//! User model and role definitions for the auth system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// User roles with hierarchical ordering.
/// Higher variants have more privileges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    User,
    Moderator,
    Admin,
}

impl UserRole {
    /// Returns the privilege level (higher = more access).
    pub fn level(&self) -> u8 {
        match self {
            UserRole::User => 0,
            UserRole::Moderator => 50,
            UserRole::Admin => 100,
        }
    }

    /// Returns true if this role has at least the privileges of `other`.
    pub fn has_at_least(&self, other: &UserRole) -> bool {
        self.level() >= other.level()
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserRole::User => write!(f, "user"),
            UserRole::Moderator => write!(f, "moderator"),
            UserRole::Admin => write!(f, "admin"),
        }
    }
}

/// Error returned when parsing an unknown role string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseRoleError {
    pub invalid_value: String,
}

impl fmt::Display for ParseRoleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown role: '{}'", self.invalid_value)
    }
}

impl std::error::Error for ParseRoleError {}

impl FromStr for UserRole {
    type Err = ParseRoleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(UserRole::User),
            "moderator" => Ok(UserRole::Moderator),
            "admin" => Ok(UserRole::Admin),
            _ => Err(ParseRoleError {
                invalid_value: s.to_string(),
            }),
        }
    }
}

/// A platform user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: UserRole,
    pub password_hash: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl User {
    /// Creates a new user with the given email and display name.
    /// Assigns the default `User` role and generates a new UUID.
    pub fn new(email: String, display_name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            email,
            display_name,
            role: UserRole::User,
            password_hash: None,
            created_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_role_display() {
        assert_eq!(UserRole::User.to_string(), "user");
        assert_eq!(UserRole::Moderator.to_string(), "moderator");
        assert_eq!(UserRole::Admin.to_string(), "admin");
    }

    #[test]
    fn test_user_role_from_str() {
        assert_eq!("user".parse::<UserRole>().unwrap(), UserRole::User);
        assert_eq!(
            "moderator".parse::<UserRole>().unwrap(),
            UserRole::Moderator
        );
        assert_eq!("admin".parse::<UserRole>().unwrap(), UserRole::Admin);
    }

    #[test]
    fn test_user_role_from_str_case_insensitive() {
        assert_eq!("USER".parse::<UserRole>().unwrap(), UserRole::User);
        assert_eq!(
            "Moderator".parse::<UserRole>().unwrap(),
            UserRole::Moderator
        );
        assert_eq!("ADMIN".parse::<UserRole>().unwrap(), UserRole::Admin);
    }

    #[test]
    fn test_user_role_from_str_invalid() {
        let err = "superadmin".parse::<UserRole>().unwrap_err();
        assert_eq!(err.invalid_value, "superadmin");
        assert!(err.to_string().contains("superadmin"));
    }

    #[test]
    fn test_user_role_level() {
        assert_eq!(UserRole::User.level(), 0);
        assert_eq!(UserRole::Moderator.level(), 50);
        assert_eq!(UserRole::Admin.level(), 100);
    }

    #[test]
    fn test_user_role_has_at_least() {
        assert!(UserRole::Admin.has_at_least(&UserRole::User));
        assert!(UserRole::Admin.has_at_least(&UserRole::Moderator));
        assert!(UserRole::Admin.has_at_least(&UserRole::Admin));
        assert!(UserRole::Moderator.has_at_least(&UserRole::User));
        assert!(UserRole::Moderator.has_at_least(&UserRole::Moderator));
        assert!(!UserRole::Moderator.has_at_least(&UserRole::Admin));
        assert!(UserRole::User.has_at_least(&UserRole::User));
        assert!(!UserRole::User.has_at_least(&UserRole::Moderator));
    }

    #[test]
    fn test_user_new() {
        let user = User::new("alice@example.com".to_string(), "Alice".to_string());
        assert_eq!(user.email, "alice@example.com");
        assert_eq!(user.display_name, "Alice");
        assert_eq!(user.role, UserRole::User);
        assert!(user.password_hash.is_none());
    }

    #[test]
    fn test_user_new_generates_unique_ids() {
        let u1 = User::new("a@example.com".to_string(), "A".to_string());
        let u2 = User::new("b@example.com".to_string(), "B".to_string());
        assert_ne!(u1.id, u2.id);
    }

    #[test]
    fn test_user_role_serde_roundtrip() {
        let role = UserRole::Moderator;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"moderator\"");
        let parsed: UserRole = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, role);
    }

    #[test]
    fn test_user_serde_roundtrip() {
        let user = User::new("test@example.com".to_string(), "Test".to_string());
        let json = serde_json::to_string(&user).unwrap();
        let parsed: User = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, user.id);
        assert_eq!(parsed.email, user.email);
        assert_eq!(parsed.role, user.role);
    }
}
