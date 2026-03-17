//! Environment-based configuration for the multiplayer prototype.

use std::net::SocketAddr;

/// Default server port.
const DEFAULT_SERVER_PORT: u16 = 7777;

/// Default tick rate in Hz.
const DEFAULT_TICK_RATE: u32 = 60;

/// Default maximum number of players.
const DEFAULT_MAX_PLAYERS: usize = 20;

/// Default server bind address prefix.
const DEFAULT_BIND_HOST: &str = "0.0.0.0";

/// Configuration for the multiplayer server.
#[derive(Debug, Clone)]
pub struct MultiplayerConfig {
    /// Address the server binds to.
    pub bind_addr: SocketAddr,
    /// Server tick rate in Hz.
    pub tick_rate: u32,
    /// Maximum number of concurrent players.
    pub max_players: usize,
}

impl MultiplayerConfig {
    /// Load configuration from environment variables with defaults.
    pub fn from_env() -> Self {
        let port: u16 = std::env::var("AETHER_SERVER_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_SERVER_PORT);

        let tick_rate: u32 = std::env::var("AETHER_TICK_RATE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_TICK_RATE);

        let max_players: usize = std::env::var("AETHER_MAX_PLAYERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_PLAYERS);

        let bind_addr = format!("{DEFAULT_BIND_HOST}:{port}")
            .parse()
            .unwrap_or_else(|_| {
                format!("{DEFAULT_BIND_HOST}:{DEFAULT_SERVER_PORT}")
                    .parse()
                    .unwrap()
            });

        Self {
            bind_addr,
            tick_rate,
            max_players,
        }
    }
}

impl Default for MultiplayerConfig {
    fn default() -> Self {
        Self {
            bind_addr: format!("{DEFAULT_BIND_HOST}:{DEFAULT_SERVER_PORT}")
                .parse()
                .unwrap(),
            tick_rate: DEFAULT_TICK_RATE,
            max_players: DEFAULT_MAX_PLAYERS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let config = MultiplayerConfig::default();
        assert_eq!(config.bind_addr.port(), DEFAULT_SERVER_PORT);
        assert_eq!(config.tick_rate, DEFAULT_TICK_RATE);
        assert_eq!(config.max_players, DEFAULT_MAX_PLAYERS);
    }

    #[test]
    fn from_env_uses_defaults_when_no_env() {
        let config = MultiplayerConfig::from_env();
        assert!(config.bind_addr.port() > 0);
        assert!(config.tick_rate > 0);
        assert!(config.max_players > 0);
    }

    #[test]
    fn default_bind_addr_is_all_interfaces() {
        let config = MultiplayerConfig::default();
        assert_eq!(config.bind_addr.ip().to_string(), DEFAULT_BIND_HOST);
    }

    #[test]
    fn config_clone_is_equal() {
        let config = MultiplayerConfig::default();
        let cloned = config.clone();
        assert_eq!(config.bind_addr, cloned.bind_addr);
        assert_eq!(config.tick_rate, cloned.tick_rate);
        assert_eq!(config.max_players, cloned.max_players);
    }
}
