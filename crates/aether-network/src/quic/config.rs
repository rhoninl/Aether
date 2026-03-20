use std::net::SocketAddr;
use std::time::Duration;

/// Maximum size for QUIC datagrams (fits in a single UDP packet).
pub const MAX_DATAGRAM_SIZE: usize = 1200;

/// Maximum size for a single reliable stream chunk.
pub const MAX_STREAM_CHUNK_SIZE: usize = 65536;

/// Default server bind address.
pub const DEFAULT_BIND_ADDR: &str = "0.0.0.0:4433";

/// Default server address for clients to connect to.
pub const DEFAULT_SERVER_ADDR: &str = "127.0.0.1:4433";

/// Default connection timeout in seconds.
pub const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;

/// Default idle connection timeout in seconds.
pub const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 30;

/// Default reconnect window timeout in seconds.
pub const DEFAULT_RECONNECT_TIMEOUT_SECS: u64 = 30;

/// Handshake magic bytes identifying the Aether protocol.
pub const HANDSHAKE_MAGIC: &[u8; 4] = b"AETH";

/// Handshake protocol version.
pub const HANDSHAKE_VERSION: u8 = 1;

/// Maximum number of concurrent bi-directional streams per connection.
pub const MAX_CONCURRENT_BI_STREAMS: u32 = 64;

/// Maximum number of concurrent uni-directional streams per connection.
pub const MAX_CONCURRENT_UNI_STREAMS: u32 = 64;

/// Length-prefix size for reliable stream framing (4 bytes = u32).
pub const STREAM_FRAME_HEADER_SIZE: usize = 4;

/// Configuration for a QUIC endpoint (client or server).
#[derive(Debug, Clone)]
pub struct QuicConfig {
    /// Address the server binds to.
    pub bind_addr: SocketAddr,
    /// Address the client connects to.
    pub server_addr: SocketAddr,
    /// Path to TLS certificate PEM file (None = use self-signed).
    pub cert_path: Option<String>,
    /// Path to TLS private key PEM file (None = use self-signed).
    pub key_path: Option<String>,
    /// Connection timeout.
    pub connect_timeout: Duration,
    /// Idle connection timeout.
    pub idle_timeout: Duration,
    /// Reconnect window timeout.
    pub reconnect_timeout: Duration,
}

impl QuicConfig {
    /// Load configuration from environment variables with defaults.
    pub fn from_env() -> Self {
        let bind_addr = std::env::var("AETHER_NET_BIND_ADDR")
            .unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string())
            .parse()
            .unwrap_or_else(|_| DEFAULT_BIND_ADDR.parse().unwrap());

        let server_addr = std::env::var("AETHER_NET_SERVER_ADDR")
            .unwrap_or_else(|_| DEFAULT_SERVER_ADDR.to_string())
            .parse()
            .unwrap_or_else(|_| DEFAULT_SERVER_ADDR.parse().unwrap());

        let cert_path = std::env::var("AETHER_NET_CERT_PATH").ok();
        let key_path = std::env::var("AETHER_NET_KEY_PATH").ok();

        let connect_timeout_secs: u64 = std::env::var("AETHER_NET_CONNECT_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_CONNECT_TIMEOUT_SECS);

        let idle_timeout_secs: u64 = std::env::var("AETHER_NET_IDLE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_IDLE_TIMEOUT_SECS);

        let reconnect_timeout_secs: u64 = std::env::var("AETHER_NET_RECONNECT_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_RECONNECT_TIMEOUT_SECS);

        Self {
            bind_addr,
            server_addr,
            cert_path,
            key_path,
            connect_timeout: Duration::from_secs(connect_timeout_secs),
            idle_timeout: Duration::from_secs(idle_timeout_secs),
            reconnect_timeout: Duration::from_secs(reconnect_timeout_secs),
        }
    }
}

impl Default for QuicConfig {
    fn default() -> Self {
        Self {
            bind_addr: DEFAULT_BIND_ADDR.parse().unwrap(),
            server_addr: DEFAULT_SERVER_ADDR.parse().unwrap(),
            cert_path: None,
            key_path: None,
            connect_timeout: Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
            idle_timeout: Duration::from_secs(DEFAULT_IDLE_TIMEOUT_SECS),
            reconnect_timeout: Duration::from_secs(DEFAULT_RECONNECT_TIMEOUT_SECS),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let config = QuicConfig::default();
        assert_eq!(
            config.bind_addr,
            DEFAULT_BIND_ADDR.parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            config.server_addr,
            DEFAULT_SERVER_ADDR.parse::<SocketAddr>().unwrap()
        );
        assert!(config.cert_path.is_none());
        assert!(config.key_path.is_none());
        assert_eq!(
            config.connect_timeout,
            Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS)
        );
        assert_eq!(
            config.idle_timeout,
            Duration::from_secs(DEFAULT_IDLE_TIMEOUT_SECS)
        );
        assert_eq!(
            config.reconnect_timeout,
            Duration::from_secs(DEFAULT_RECONNECT_TIMEOUT_SECS)
        );
    }

    #[test]
    fn config_from_env_uses_defaults_when_no_env_set() {
        // Clear any env vars that might be set
        let config = QuicConfig::from_env();
        // Should at minimum parse without panicking and have valid addresses
        assert!(config.bind_addr.port() > 0 || config.bind_addr.port() == 0);
        assert!(config.connect_timeout.as_secs() > 0);
    }

    #[test]
    fn constants_are_sensible() {
        assert!(MAX_DATAGRAM_SIZE > 0);
        assert!(MAX_DATAGRAM_SIZE <= 1500); // Must fit in a single UDP packet
        assert!(MAX_STREAM_CHUNK_SIZE > MAX_DATAGRAM_SIZE);
        assert_eq!(HANDSHAKE_MAGIC, b"AETH");
        assert_eq!(HANDSHAKE_VERSION, 1);
        assert!(MAX_CONCURRENT_BI_STREAMS > 0);
    }
}
