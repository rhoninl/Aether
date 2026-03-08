//! OAuth2 provider traits and configurations for Google and Discord.

use serde::{Deserialize, Serialize};
use std::env;

/// Environment variable names for OAuth configuration.
const ENV_OAUTH_GOOGLE_CLIENT_ID: &str = "OAUTH_GOOGLE_CLIENT_ID";
const ENV_OAUTH_GOOGLE_CLIENT_SECRET: &str = "OAUTH_GOOGLE_CLIENT_SECRET";
const ENV_OAUTH_DISCORD_CLIENT_ID: &str = "OAUTH_DISCORD_CLIENT_ID";
const ENV_OAUTH_DISCORD_CLIENT_SECRET: &str = "OAUTH_DISCORD_CLIENT_SECRET";

/// Well-known OAuth2 endpoint URLs.
const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";

const DISCORD_AUTH_URL: &str = "https://discord.com/api/oauth2/authorize";
const DISCORD_TOKEN_URL: &str = "https://discord.com/api/oauth2/token";
const DISCORD_USERINFO_URL: &str = "https://discord.com/api/users/@me";

/// Configuration for an OAuth2 provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

/// User info retrieved from an OAuth2 provider after authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthUserInfo {
    pub provider: String,
    pub provider_user_id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
}

/// Trait for OAuth2 providers. Implementations define provider-specific URLs and scopes.
pub trait OAuthProvider: Send + Sync {
    /// Returns the provider name (e.g., "google", "discord").
    fn provider_name(&self) -> &str;

    /// Returns the authorization URL.
    fn auth_url(&self) -> &str;

    /// Returns the token exchange URL.
    fn token_url(&self) -> &str;

    /// Returns the user info URL.
    fn userinfo_url(&self) -> &str;

    /// Returns the OAuth2 scopes required.
    fn scopes(&self) -> Vec<String>;

    /// Returns the OAuth config (client_id, client_secret, redirect_uri).
    fn config(&self) -> &OAuthConfig;

    /// Builds the full authorization redirect URL with query parameters.
    fn build_auth_redirect_url(&self, state: &str) -> String {
        let config = self.config();
        let scopes = self.scopes().join(" ");
        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            self.auth_url(),
            config.client_id,
            config.redirect_uri,
            scopes,
            state
        )
    }
}

/// Google OAuth2 provider.
#[derive(Debug, Clone)]
pub struct GoogleOAuthProvider {
    config: OAuthConfig,
}

impl GoogleOAuthProvider {
    pub fn new(config: OAuthConfig) -> Self {
        Self { config }
    }

    /// Creates from environment variables. Returns `None` if client ID is not set.
    pub fn from_env(redirect_uri: String) -> Option<Self> {
        let client_id = env::var(ENV_OAUTH_GOOGLE_CLIENT_ID).ok()?;
        let client_secret =
            env::var(ENV_OAUTH_GOOGLE_CLIENT_SECRET).unwrap_or_default();
        Some(Self::new(OAuthConfig {
            client_id,
            client_secret,
            redirect_uri,
        }))
    }
}

impl OAuthProvider for GoogleOAuthProvider {
    fn provider_name(&self) -> &str {
        "google"
    }

    fn auth_url(&self) -> &str {
        GOOGLE_AUTH_URL
    }

    fn token_url(&self) -> &str {
        GOOGLE_TOKEN_URL
    }

    fn userinfo_url(&self) -> &str {
        GOOGLE_USERINFO_URL
    }

    fn scopes(&self) -> Vec<String> {
        vec![
            "openid".to_string(),
            "email".to_string(),
            "profile".to_string(),
        ]
    }

    fn config(&self) -> &OAuthConfig {
        &self.config
    }
}

/// Discord OAuth2 provider.
#[derive(Debug, Clone)]
pub struct DiscordOAuthProvider {
    config: OAuthConfig,
}

impl DiscordOAuthProvider {
    pub fn new(config: OAuthConfig) -> Self {
        Self { config }
    }

    /// Creates from environment variables. Returns `None` if client ID is not set.
    pub fn from_env(redirect_uri: String) -> Option<Self> {
        let client_id = env::var(ENV_OAUTH_DISCORD_CLIENT_ID).ok()?;
        let client_secret =
            env::var(ENV_OAUTH_DISCORD_CLIENT_SECRET).unwrap_or_default();
        Some(Self::new(OAuthConfig {
            client_id,
            client_secret,
            redirect_uri,
        }))
    }
}

impl OAuthProvider for DiscordOAuthProvider {
    fn provider_name(&self) -> &str {
        "discord"
    }

    fn auth_url(&self) -> &str {
        DISCORD_AUTH_URL
    }

    fn token_url(&self) -> &str {
        DISCORD_TOKEN_URL
    }

    fn userinfo_url(&self) -> &str {
        DISCORD_USERINFO_URL
    }

    fn scopes(&self) -> Vec<String> {
        vec!["identify".to_string(), "email".to_string()]
    }

    fn config(&self) -> &OAuthConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn google_test_config() -> OAuthConfig {
        OAuthConfig {
            client_id: "google-test-client-id".to_string(),
            client_secret: "google-test-secret".to_string(),
            redirect_uri: "http://localhost:8080/callback/google".to_string(),
        }
    }

    fn discord_test_config() -> OAuthConfig {
        OAuthConfig {
            client_id: "discord-test-client-id".to_string(),
            client_secret: "discord-test-secret".to_string(),
            redirect_uri: "http://localhost:8080/callback/discord".to_string(),
        }
    }

    #[test]
    fn test_google_provider_name() {
        let provider = GoogleOAuthProvider::new(google_test_config());
        assert_eq!(provider.provider_name(), "google");
    }

    #[test]
    fn test_google_auth_url() {
        let provider = GoogleOAuthProvider::new(google_test_config());
        assert_eq!(provider.auth_url(), GOOGLE_AUTH_URL);
    }

    #[test]
    fn test_google_token_url() {
        let provider = GoogleOAuthProvider::new(google_test_config());
        assert_eq!(provider.token_url(), GOOGLE_TOKEN_URL);
    }

    #[test]
    fn test_google_userinfo_url() {
        let provider = GoogleOAuthProvider::new(google_test_config());
        assert_eq!(provider.userinfo_url(), GOOGLE_USERINFO_URL);
    }

    #[test]
    fn test_google_scopes() {
        let provider = GoogleOAuthProvider::new(google_test_config());
        let scopes = provider.scopes();
        assert!(scopes.contains(&"openid".to_string()));
        assert!(scopes.contains(&"email".to_string()));
        assert!(scopes.contains(&"profile".to_string()));
    }

    #[test]
    fn test_google_build_auth_redirect_url() {
        let provider = GoogleOAuthProvider::new(google_test_config());
        let url = provider.build_auth_redirect_url("random-state-123");

        assert!(url.starts_with(GOOGLE_AUTH_URL));
        assert!(url.contains("client_id=google-test-client-id"));
        assert!(url.contains("redirect_uri=http://localhost:8080/callback/google"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("state=random-state-123"));
        assert!(url.contains("scope=openid"));
    }

    #[test]
    fn test_discord_provider_name() {
        let provider = DiscordOAuthProvider::new(discord_test_config());
        assert_eq!(provider.provider_name(), "discord");
    }

    #[test]
    fn test_discord_auth_url() {
        let provider = DiscordOAuthProvider::new(discord_test_config());
        assert_eq!(provider.auth_url(), DISCORD_AUTH_URL);
    }

    #[test]
    fn test_discord_token_url() {
        let provider = DiscordOAuthProvider::new(discord_test_config());
        assert_eq!(provider.token_url(), DISCORD_TOKEN_URL);
    }

    #[test]
    fn test_discord_userinfo_url() {
        let provider = DiscordOAuthProvider::new(discord_test_config());
        assert_eq!(provider.userinfo_url(), DISCORD_USERINFO_URL);
    }

    #[test]
    fn test_discord_scopes() {
        let provider = DiscordOAuthProvider::new(discord_test_config());
        let scopes = provider.scopes();
        assert!(scopes.contains(&"identify".to_string()));
        assert!(scopes.contains(&"email".to_string()));
    }

    #[test]
    fn test_discord_build_auth_redirect_url() {
        let provider = DiscordOAuthProvider::new(discord_test_config());
        let url = provider.build_auth_redirect_url("state-xyz");

        assert!(url.starts_with(DISCORD_AUTH_URL));
        assert!(url.contains("client_id=discord-test-client-id"));
        assert!(url.contains("state=state-xyz"));
    }

    #[test]
    fn test_oauth_config_serde_roundtrip() {
        let config = google_test_config();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: OAuthConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.client_id, config.client_id);
        assert_eq!(parsed.client_secret, config.client_secret);
        assert_eq!(parsed.redirect_uri, config.redirect_uri);
    }

    #[test]
    fn test_oauth_user_info_serde() {
        let info = OAuthUserInfo {
            provider: "google".to_string(),
            provider_user_id: "12345".to_string(),
            email: Some("user@example.com".to_string()),
            display_name: Some("Test User".to_string()),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: OAuthUserInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.provider, "google");
        assert_eq!(parsed.provider_user_id, "12345");
        assert_eq!(parsed.email.unwrap(), "user@example.com");
    }

    #[test]
    fn test_google_from_env_returns_none_when_not_set() {
        env::remove_var(ENV_OAUTH_GOOGLE_CLIENT_ID);
        let provider = GoogleOAuthProvider::from_env("http://localhost/cb".to_string());
        assert!(provider.is_none());
    }

    #[test]
    fn test_discord_from_env_returns_none_when_not_set() {
        env::remove_var(ENV_OAUTH_DISCORD_CLIENT_ID);
        let provider = DiscordOAuthProvider::from_env("http://localhost/cb".to_string());
        assert!(provider.is_none());
    }

    #[test]
    fn test_provider_config_returns_correct_values() {
        let config = google_test_config();
        let provider = GoogleOAuthProvider::new(config.clone());
        assert_eq!(provider.config().client_id, "google-test-client-id");
        assert_eq!(provider.config().client_secret, "google-test-secret");
    }
}
