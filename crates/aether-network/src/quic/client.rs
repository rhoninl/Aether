use std::net::SocketAddr;

use quinn::Endpoint;

use super::config::QuicConfig;
use super::connection::{ConnectionError, ConnectionState, QuicConnection};
use super::tls;

/// Errors specific to client operations.
#[derive(Debug)]
pub enum ClientError {
    /// Failed to bind the local endpoint.
    Bind(String),
    /// Failed to connect to the server.
    Connect(String),
    /// Connection-level error.
    Connection(ConnectionError),
    /// Client is not connected.
    NotConnected,
    /// Connection timeout.
    Timeout,
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Bind(msg) => write!(f, "client bind failed: {msg}"),
            ClientError::Connect(msg) => write!(f, "client connect failed: {msg}"),
            ClientError::Connection(e) => write!(f, "client connection error: {e}"),
            ClientError::NotConnected => write!(f, "client is not connected"),
            ClientError::Timeout => write!(f, "connection timed out"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<ConnectionError> for ClientError {
    fn from(e: ConnectionError) -> Self {
        ClientError::Connection(e)
    }
}

/// A QUIC client that connects to a server.
pub struct QuicClient {
    endpoint: Endpoint,
    connection: Option<QuicConnection>,
    config: QuicConfig,
}

impl QuicClient {
    /// Create a new QUIC client with the given configuration.
    ///
    /// Binds a local UDP socket but does not connect yet.
    pub fn new(config: QuicConfig) -> Result<Self, ClientError> {
        let client_config = tls::build_client_config_dev();

        let mut endpoint = Endpoint::client("0.0.0.0:0".parse::<SocketAddr>().unwrap())
            .map_err(|e| ClientError::Bind(e.to_string()))?;
        endpoint.set_default_client_config(client_config);

        Ok(Self {
            endpoint,
            connection: None,
            config,
        })
    }

    /// Connect to the server and perform the handshake.
    ///
    /// Returns the server tick from the handshake response.
    pub async fn connect(
        &mut self,
        client_id: u64,
        token: &[u8],
    ) -> Result<u64, ClientError> {
        let server_name = server_name_from_addr(&self.config.server_addr);

        let connecting = self
            .endpoint
            .connect(self.config.server_addr, &server_name)
            .map_err(|e| ClientError::Connect(e.to_string()))?;

        let quinn_conn = tokio::time::timeout(self.config.connect_timeout, connecting)
            .await
            .map_err(|_| ClientError::Timeout)?
            .map_err(|e| ClientError::Connect(e.to_string()))?;

        let mut conn = QuicConnection::new(quinn_conn, client_id);
        let server_tick = conn.client_handshake(client_id, token).await?;

        self.connection = Some(conn);
        Ok(server_tick)
    }

    /// Check if the client is currently connected.
    pub fn is_connected(&self) -> bool {
        self.connection
            .as_ref()
            .map(|c| c.state() == ConnectionState::Connected)
            .unwrap_or(false)
    }

    /// Get the connection state.
    pub fn state(&self) -> ConnectionState {
        self.connection
            .as_ref()
            .map(|c| c.state())
            .unwrap_or(ConnectionState::Disconnected)
    }

    /// Send a reliable message to the server.
    pub async fn send_reliable(&self, data: &[u8]) -> Result<(), ClientError> {
        let conn = self.connection.as_ref().ok_or(ClientError::NotConnected)?;
        conn.send_reliable(data).await.map_err(ClientError::from)
    }

    /// Send an unreliable datagram to the server.
    pub fn send_datagram(&self, data: &[u8]) -> Result<(), ClientError> {
        let conn = self.connection.as_ref().ok_or(ClientError::NotConnected)?;
        conn.send_datagram(data).map_err(ClientError::from)
    }

    /// Accept a bi-directional stream from the server and read one message.
    pub async fn recv_reliable(&self) -> Result<Vec<u8>, ClientError> {
        let conn = self.connection.as_ref().ok_or(ClientError::NotConnected)?;
        let (data, _send) = conn.accept_bi_stream().await?;
        Ok(data)
    }

    /// Get a reference to the underlying connection (if connected).
    pub fn connection(&self) -> Option<&QuicConnection> {
        self.connection.as_ref()
    }

    /// Get a mutable reference to the underlying connection (if connected).
    pub fn connection_mut(&mut self) -> Option<&mut QuicConnection> {
        self.connection.as_mut()
    }

    /// Disconnect from the server.
    pub fn disconnect(&mut self, reason: &str) {
        if let Some(mut conn) = self.connection.take() {
            conn.close(reason);
        }
    }

    /// Shut down the client endpoint.
    pub fn shutdown(&mut self) {
        self.disconnect("client shutdown");
        self.endpoint.close(0u32.into(), b"client shutdown");
    }
}

/// Extract a suitable server name for TLS from a socket address.
fn server_name_from_addr(addr: &SocketAddr) -> String {
    match addr.ip() {
        std::net::IpAddr::V4(ip) if ip.is_loopback() => "localhost".to_string(),
        std::net::IpAddr::V6(ip) if ip.is_loopback() => "localhost".to_string(),
        ip => ip.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_error_displays() {
        let err = ClientError::NotConnected;
        assert!(format!("{err}").contains("not connected"));
    }

    #[test]
    fn client_error_from_connection_error() {
        let conn_err = ConnectionError::ConnectionLost("gone".to_string());
        let err: ClientError = conn_err.into();
        assert!(format!("{err}").contains("gone"));
    }

    #[test]
    fn server_name_from_loopback() {
        let addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
        assert_eq!(server_name_from_addr(&addr), "localhost");

        let addr_v6: SocketAddr = "[::1]:4433".parse().unwrap();
        assert_eq!(server_name_from_addr(&addr_v6), "localhost");
    }

    #[test]
    fn server_name_from_non_loopback() {
        let addr: SocketAddr = "192.168.1.1:4433".parse().unwrap();
        assert_eq!(server_name_from_addr(&addr), "192.168.1.1");
    }
}
