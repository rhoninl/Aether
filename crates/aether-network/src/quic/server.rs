use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use quinn::Endpoint;
use tokio::sync::Mutex;

use super::config::QuicConfig;
use super::connection::{ConnectionError, ConnectionState, QuicConnection};
use super::tls;

/// Errors specific to server operations.
#[derive(Debug)]
pub enum ServerError {
    /// Failed to bind to the specified address.
    Bind(String),
    /// TLS setup failed.
    Tls(tls::TlsError),
    /// Connection-level error.
    Connection(ConnectionError),
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerError::Bind(msg) => write!(f, "server bind failed: {msg}"),
            ServerError::Tls(e) => write!(f, "server TLS error: {e}"),
            ServerError::Connection(e) => write!(f, "server connection error: {e}"),
        }
    }
}

impl std::error::Error for ServerError {}

impl From<tls::TlsError> for ServerError {
    fn from(e: tls::TlsError) -> Self {
        ServerError::Tls(e)
    }
}

impl From<ConnectionError> for ServerError {
    fn from(e: ConnectionError) -> Self {
        ServerError::Connection(e)
    }
}

/// A QUIC server that accepts client connections.
pub struct QuicServer {
    endpoint: Endpoint,
    connections: Arc<Mutex<HashMap<u64, QuicConnection>>>,
    server_tick: Arc<Mutex<u64>>,
}

impl QuicServer {
    /// Create and bind a new QUIC server.
    ///
    /// If the config has cert/key paths, they are loaded from disk.
    /// Otherwise, a self-signed certificate is generated for development.
    pub fn bind(config: &QuicConfig) -> Result<Self, ServerError> {
        let cert_pair = match (&config.cert_path, &config.key_path) {
            (Some(cert), Some(key)) => tls::load_from_files(cert, key)?,
            _ => tls::generate_self_signed()?,
        };

        let mut server_config = tls::build_server_config(&cert_pair)?;

        let mut transport_config = quinn::TransportConfig::default();
        transport_config.max_idle_timeout(Some(
            config
                .idle_timeout
                .try_into()
                .expect("idle timeout should fit in quinn::IdleTimeout"),
        ));
        transport_config.max_concurrent_bidi_streams(
            super::config::MAX_CONCURRENT_BI_STREAMS.into(),
        );
        transport_config.max_concurrent_uni_streams(
            super::config::MAX_CONCURRENT_UNI_STREAMS.into(),
        );
        transport_config.datagram_receive_buffer_size(Some(65536));
        server_config.transport_config(Arc::new(transport_config));

        let endpoint = Endpoint::server(server_config, config.bind_addr)
            .map_err(|e| ServerError::Bind(e.to_string()))?;

        Ok(Self {
            endpoint,
            connections: Arc::new(Mutex::new(HashMap::new())),
            server_tick: Arc::new(Mutex::new(0)),
        })
    }

    /// Get the local address the server is bound to.
    pub fn local_addr(&self) -> Result<SocketAddr, ServerError> {
        self.endpoint
            .local_addr()
            .map_err(|e| ServerError::Bind(e.to_string()))
    }

    /// Set the current server tick (used in handshake responses).
    pub async fn set_server_tick(&self, tick: u64) {
        let mut t = self.server_tick.lock().await;
        *t = tick;
    }

    /// Accept one incoming connection, perform handshake, and store it.
    ///
    /// Returns the client_id and token on success.
    pub async fn accept(&self) -> Result<(u64, Vec<u8>), ServerError> {
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or_else(|| ServerError::Bind("endpoint closed".to_string()))?;

        let quinn_conn = incoming
            .await
            .map_err(|e| ServerError::Connection(ConnectionError::ConnectionLost(e.to_string())))?;

        let mut conn = QuicConnection::new(quinn_conn, 0);
        let server_tick = { *self.server_tick.lock().await };
        let (client_id, token) = conn.server_handshake(server_tick).await?;

        let mut conns = self.connections.lock().await;
        conns.insert(client_id, conn);

        Ok((client_id, token))
    }

    /// Send a reliable message to a specific client.
    pub async fn send_reliable(&self, client_id: u64, data: &[u8]) -> Result<(), ServerError> {
        let conns = self.connections.lock().await;
        let conn = conns
            .get(&client_id)
            .ok_or_else(|| {
                ServerError::Connection(ConnectionError::ConnectionLost(format!(
                    "no connection for client {client_id}"
                )))
            })?;

        conn.send_reliable(data).await.map_err(ServerError::from)
    }

    /// Send an unreliable datagram to a specific client.
    pub fn send_datagram_blocking(&self, client_id: u64, data: &[u8]) -> Result<(), ServerError> {
        let conns = self.connections.blocking_lock();
        let conn = conns
            .get(&client_id)
            .ok_or_else(|| {
                ServerError::Connection(ConnectionError::ConnectionLost(format!(
                    "no connection for client {client_id}"
                )))
            })?;

        conn.send_datagram(data).map_err(ServerError::from)
    }

    /// Send an unreliable datagram to a specific client (async).
    pub async fn send_datagram(&self, client_id: u64, data: &[u8]) -> Result<(), ServerError> {
        let conns = self.connections.lock().await;
        let conn = conns
            .get(&client_id)
            .ok_or_else(|| {
                ServerError::Connection(ConnectionError::ConnectionLost(format!(
                    "no connection for client {client_id}"
                )))
            })?;

        conn.send_datagram(data).map_err(ServerError::from)
    }

    /// Poll all connected clients for incoming datagrams.
    ///
    /// Returns a vec of `(client_id, data)` for each datagram received.
    pub async fn recv_datagrams(&self) -> Vec<(u64, Vec<u8>)> {
        let mut conns = self.connections.lock().await;
        let mut result = Vec::new();
        for (&client_id, conn) in conns.iter_mut() {
            while let Some(data) = conn.try_recv_datagram() {
                result.push((client_id, data));
            }
        }
        result
    }

    /// Accept a bi-directional stream from a specific client and read one message.
    pub async fn recv_reliable(&self, client_id: u64) -> Result<Vec<u8>, ServerError> {
        let conns = self.connections.lock().await;
        let conn = conns
            .get(&client_id)
            .ok_or_else(|| {
                ServerError::Connection(ConnectionError::ConnectionLost(format!(
                    "no connection for client {client_id}"
                )))
            })?;

        let (data, _send) = conn.accept_bi_stream().await?;
        Ok(data)
    }

    /// Disconnect a specific client.
    pub async fn disconnect(&self, client_id: u64, reason: &str) {
        let mut conns = self.connections.lock().await;
        if let Some(mut conn) = conns.remove(&client_id) {
            conn.close(reason);
        }
    }

    /// Get the number of connected clients.
    pub async fn connection_count(&self) -> usize {
        self.connections.lock().await.len()
    }

    /// Check if a specific client is connected.
    pub async fn is_connected(&self, client_id: u64) -> bool {
        let conns = self.connections.lock().await;
        conns
            .get(&client_id)
            .map(|c| c.state() == ConnectionState::Connected)
            .unwrap_or(false)
    }

    /// Shut down the server, closing all connections.
    pub async fn shutdown(&self) {
        let mut conns = self.connections.lock().await;
        for (_, mut conn) in conns.drain() {
            conn.close("server shutdown");
        }
        self.endpoint.close(0u32.into(), b"server shutdown");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_error_displays() {
        let err = ServerError::Bind("port in use".to_string());
        assert!(format!("{err}").contains("port in use"));
    }

    #[test]
    fn server_error_from_tls() {
        let tls_err = tls::TlsError::CertGeneration("test".to_string());
        let err: ServerError = tls_err.into();
        assert!(format!("{err}").contains("test"));
    }
}
