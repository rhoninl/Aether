use std::collections::VecDeque;

use quinn::{Connection, RecvStream, SendStream};

use super::config::{
    HANDSHAKE_MAGIC, HANDSHAKE_VERSION, MAX_DATAGRAM_SIZE, MAX_STREAM_CHUNK_SIZE,
    STREAM_FRAME_HEADER_SIZE,
};

/// Errors from QUIC connection operations.
#[derive(Debug)]
pub enum ConnectionError {
    /// The QUIC connection was lost.
    ConnectionLost(String),
    /// Failed to open a stream.
    StreamOpen(String),
    /// Failed to send data on a stream.
    StreamWrite(String),
    /// Failed to read data from a stream.
    StreamRead(String),
    /// Failed to send a datagram.
    DatagramSend(String),
    /// The message is too large for the chosen transport.
    MessageTooLarge { size: usize, max: usize },
    /// Handshake protocol error.
    HandshakeFailed(String),
}

impl std::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionError::ConnectionLost(msg) => write!(f, "connection lost: {msg}"),
            ConnectionError::StreamOpen(msg) => write!(f, "stream open: {msg}"),
            ConnectionError::StreamWrite(msg) => write!(f, "stream write: {msg}"),
            ConnectionError::StreamRead(msg) => write!(f, "stream read: {msg}"),
            ConnectionError::DatagramSend(msg) => write!(f, "datagram send: {msg}"),
            ConnectionError::MessageTooLarge { size, max } => {
                write!(f, "message too large: {size} > {max}")
            }
            ConnectionError::HandshakeFailed(msg) => write!(f, "handshake failed: {msg}"),
        }
    }
}

impl std::error::Error for ConnectionError {}

/// Connection state in the lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Handshaking,
    Connected,
    Disconnected,
}

/// Handshake status codes sent from server to client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HandshakeStatus {
    Ok = 0,
    Rejected = 1,
    VersionMismatch = 2,
}

impl HandshakeStatus {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(HandshakeStatus::Ok),
            1 => Some(HandshakeStatus::Rejected),
            2 => Some(HandshakeStatus::VersionMismatch),
            _ => None,
        }
    }
}

/// Wraps a quinn::Connection with framed send/recv for reliable and unreliable channels.
pub struct QuicConnection {
    inner: Connection,
    state: ConnectionState,
    client_id: u64,
    recv_buffer: VecDeque<Vec<u8>>,
}

impl std::fmt::Debug for QuicConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuicConnection")
            .field("state", &self.state)
            .field("client_id", &self.client_id)
            .field("recv_buffer_len", &self.recv_buffer.len())
            .finish()
    }
}

impl QuicConnection {
    /// Create a new connection wrapper.
    pub fn new(inner: Connection, client_id: u64) -> Self {
        Self {
            inner,
            state: ConnectionState::Connecting,
            client_id,
            recv_buffer: VecDeque::new(),
        }
    }

    /// Get the current connection state.
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Get the client ID associated with this connection.
    pub fn client_id(&self) -> u64 {
        self.client_id
    }

    /// Set the connection state.
    pub fn set_state(&mut self, state: ConnectionState) {
        self.state = state;
    }

    /// Get a reference to the underlying quinn connection.
    pub fn inner(&self) -> &Connection {
        &self.inner
    }

    /// Send a message over a reliable ordered bi-directional stream.
    ///
    /// The message is length-prefixed with a 4-byte (u32) header.
    pub async fn send_reliable(&self, data: &[u8]) -> Result<(), ConnectionError> {
        if data.len() > MAX_STREAM_CHUNK_SIZE {
            return Err(ConnectionError::MessageTooLarge {
                size: data.len(),
                max: MAX_STREAM_CHUNK_SIZE,
            });
        }

        let (mut send, _recv) = self
            .inner
            .open_bi()
            .await
            .map_err(|e| ConnectionError::StreamOpen(e.to_string()))?;

        write_framed(&mut send, data).await?;

        send.finish()
            .map_err(|e| ConnectionError::StreamWrite(e.to_string()))?;

        Ok(())
    }

    /// Send a message as an unreliable QUIC datagram.
    ///
    /// If the message is too large for a datagram, returns an error.
    pub fn send_datagram(&self, data: &[u8]) -> Result<(), ConnectionError> {
        if data.len() > MAX_DATAGRAM_SIZE {
            return Err(ConnectionError::MessageTooLarge {
                size: data.len(),
                max: MAX_DATAGRAM_SIZE,
            });
        }

        self.inner
            .send_datagram(data.to_vec().into())
            .map_err(|e| ConnectionError::DatagramSend(e.to_string()))?;

        Ok(())
    }

    /// Receive a datagram (non-blocking poll via try_recv).
    pub fn try_recv_datagram(&mut self) -> Option<Vec<u8>> {
        match self.inner.read_datagram().now_or_never() {
            Some(Ok(bytes)) => Some(bytes.to_vec()),
            _ => None,
        }
    }

    /// Accept an incoming bi-directional stream and read one framed message from it.
    pub async fn accept_bi_stream(&self) -> Result<(Vec<u8>, SendStream), ConnectionError> {
        let (send, recv) = self
            .inner
            .accept_bi()
            .await
            .map_err(|e| ConnectionError::StreamOpen(e.to_string()))?;

        let data = read_framed(recv).await?;
        Ok((data, send))
    }

    /// Push received data into the buffer for synchronous retrieval.
    pub fn push_recv(&mut self, data: Vec<u8>) {
        self.recv_buffer.push_back(data);
    }

    /// Pop buffered received messages.
    pub fn pop_recv(&mut self, max: usize) -> Vec<Vec<u8>> {
        let count = max.min(self.recv_buffer.len());
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            if let Some(data) = self.recv_buffer.pop_front() {
                out.push(data);
            }
        }
        out
    }

    /// Perform the client side of the handshake protocol.
    pub async fn client_handshake(
        &mut self,
        client_id: u64,
        token: &[u8],
    ) -> Result<u64, ConnectionError> {
        self.state = ConnectionState::Handshaking;

        let (mut send, mut recv) = self
            .inner
            .open_bi()
            .await
            .map_err(|e| ConnectionError::StreamOpen(e.to_string()))?;

        // Build handshake request:
        // [MAGIC(4)] [VERSION(1)] [CLIENT_ID(8)] [TOKEN_LEN(2)] [TOKEN(N)]
        let token_len = token.len().min(u16::MAX as usize) as u16;
        let mut request = Vec::with_capacity(4 + 1 + 8 + 2 + token_len as usize);
        request.extend_from_slice(HANDSHAKE_MAGIC);
        request.push(HANDSHAKE_VERSION);
        request.extend_from_slice(&client_id.to_le_bytes());
        request.extend_from_slice(&token_len.to_le_bytes());
        request.extend_from_slice(&token[..token_len as usize]);

        send.write_all(&request)
            .await
            .map_err(|e| ConnectionError::StreamWrite(e.to_string()))?;
        send.finish()
            .map_err(|e| ConnectionError::StreamWrite(e.to_string()))?;

        // Read handshake response:
        // [MAGIC(4)] [VERSION(1)] [STATUS(1)] [SERVER_TICK(8)]
        let mut response = vec![0u8; 14];
        recv.read_exact(&mut response)
            .await
            .map_err(|e| ConnectionError::StreamRead(e.to_string()))?;

        if &response[0..4] != HANDSHAKE_MAGIC {
            return Err(ConnectionError::HandshakeFailed(
                "invalid magic in response".to_string(),
            ));
        }

        if response[4] != HANDSHAKE_VERSION {
            return Err(ConnectionError::HandshakeFailed(format!(
                "version mismatch: server={}, client={HANDSHAKE_VERSION}",
                response[4]
            )));
        }

        let status = HandshakeStatus::from_byte(response[5]).ok_or_else(|| {
            ConnectionError::HandshakeFailed(format!("unknown status byte: {}", response[5]))
        })?;

        match status {
            HandshakeStatus::Ok => {}
            HandshakeStatus::Rejected => {
                self.state = ConnectionState::Disconnected;
                return Err(ConnectionError::HandshakeFailed(
                    "server rejected".to_string(),
                ));
            }
            HandshakeStatus::VersionMismatch => {
                self.state = ConnectionState::Disconnected;
                return Err(ConnectionError::HandshakeFailed(
                    "version mismatch".to_string(),
                ));
            }
        }

        let server_tick = u64::from_le_bytes(response[6..14].try_into().unwrap());
        self.client_id = client_id;
        self.state = ConnectionState::Connected;
        Ok(server_tick)
    }

    /// Perform the server side of the handshake protocol.
    ///
    /// Returns the client_id and token from the handshake request.
    pub async fn server_handshake(
        &mut self,
        server_tick: u64,
    ) -> Result<(u64, Vec<u8>), ConnectionError> {
        self.state = ConnectionState::Handshaking;

        let (mut send, mut recv) = self
            .inner
            .accept_bi()
            .await
            .map_err(|e| ConnectionError::StreamOpen(e.to_string()))?;

        // Read handshake request header: MAGIC(4) + VERSION(1) + CLIENT_ID(8) + TOKEN_LEN(2) = 15
        let mut header = vec![0u8; 15];
        recv.read_exact(&mut header)
            .await
            .map_err(|e| ConnectionError::StreamRead(e.to_string()))?;

        if &header[0..4] != HANDSHAKE_MAGIC {
            return Err(ConnectionError::HandshakeFailed(
                "invalid magic in request".to_string(),
            ));
        }

        let client_version = header[4];
        let client_id = u64::from_le_bytes(header[5..13].try_into().unwrap());
        let token_len = u16::from_le_bytes(header[13..15].try_into().unwrap()) as usize;

        // Read token
        let mut token = vec![0u8; token_len];
        if token_len > 0 {
            recv.read_exact(&mut token)
                .await
                .map_err(|e| ConnectionError::StreamRead(e.to_string()))?;
        }

        // Check version
        let status = if client_version != HANDSHAKE_VERSION {
            HandshakeStatus::VersionMismatch
        } else {
            HandshakeStatus::Ok
        };

        // Send handshake response:
        // [MAGIC(4)] [VERSION(1)] [STATUS(1)] [SERVER_TICK(8)]
        let mut response = Vec::with_capacity(14);
        response.extend_from_slice(HANDSHAKE_MAGIC);
        response.push(HANDSHAKE_VERSION);
        response.push(status as u8);
        response.extend_from_slice(&server_tick.to_le_bytes());

        send.write_all(&response)
            .await
            .map_err(|e| ConnectionError::StreamWrite(e.to_string()))?;
        send.finish()
            .map_err(|e| ConnectionError::StreamWrite(e.to_string()))?;

        if status != HandshakeStatus::Ok {
            self.state = ConnectionState::Disconnected;
            return Err(ConnectionError::HandshakeFailed(format!(
                "version mismatch: client={client_version}, server={HANDSHAKE_VERSION}"
            )));
        }

        self.client_id = client_id;
        self.state = ConnectionState::Connected;
        Ok((client_id, token))
    }

    /// Close the connection gracefully.
    pub fn close(&mut self, reason: &str) {
        self.state = ConnectionState::Disconnected;
        self.inner.close(0u32.into(), reason.as_bytes());
    }
}

/// Write a length-prefixed frame to a send stream.
async fn write_framed(send: &mut SendStream, data: &[u8]) -> Result<(), ConnectionError> {
    let len = data.len() as u32;
    send.write_all(&len.to_le_bytes())
        .await
        .map_err(|e| ConnectionError::StreamWrite(e.to_string()))?;
    send.write_all(data)
        .await
        .map_err(|e| ConnectionError::StreamWrite(e.to_string()))?;
    Ok(())
}

/// Read a length-prefixed frame from a recv stream.
async fn read_framed(mut recv: RecvStream) -> Result<Vec<u8>, ConnectionError> {
    let mut len_buf = [0u8; STREAM_FRAME_HEADER_SIZE];
    recv.read_exact(&mut len_buf)
        .await
        .map_err(|e| ConnectionError::StreamRead(e.to_string()))?;
    let len = u32::from_le_bytes(len_buf) as usize;

    if len > MAX_STREAM_CHUNK_SIZE {
        return Err(ConnectionError::MessageTooLarge {
            size: len,
            max: MAX_STREAM_CHUNK_SIZE,
        });
    }

    let mut data = vec![0u8; len];
    recv.read_exact(&mut data)
        .await
        .map_err(|e| ConnectionError::StreamRead(e.to_string()))?;
    Ok(data)
}

/// Extension trait for poll-like access to async functions.
trait NowOrNever {
    type Output;
    fn now_or_never(self) -> Option<Self::Output>;
}

impl<F: std::future::Future> NowOrNever for F {
    type Output = F::Output;
    fn now_or_never(self) -> Option<Self::Output> {
        let mut pinned = std::pin::pin!(self);
        let waker = noop_waker();
        let mut cx = std::task::Context::from_waker(&waker);
        match pinned.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(val) => Some(val),
            std::task::Poll::Pending => None,
        }
    }
}

fn noop_waker() -> std::task::Waker {
    use std::task::{RawWaker, RawWakerVTable};

    fn no_op(_: *const ()) {}
    fn clone(data: *const ()) -> RawWaker {
        RawWaker::new(data, &VTABLE)
    }

    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
    unsafe { std::task::Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_status_from_byte_roundtrips() {
        assert_eq!(HandshakeStatus::from_byte(0), Some(HandshakeStatus::Ok));
        assert_eq!(
            HandshakeStatus::from_byte(1),
            Some(HandshakeStatus::Rejected)
        );
        assert_eq!(
            HandshakeStatus::from_byte(2),
            Some(HandshakeStatus::VersionMismatch)
        );
        assert_eq!(HandshakeStatus::from_byte(255), None);
    }

    #[test]
    fn connection_state_enum_covers_lifecycle() {
        let states = [
            ConnectionState::Connecting,
            ConnectionState::Handshaking,
            ConnectionState::Connected,
            ConnectionState::Disconnected,
        ];
        assert_eq!(states.len(), 4);
    }

    #[test]
    fn connection_error_displays() {
        let err = ConnectionError::MessageTooLarge {
            size: 2000,
            max: 1200,
        };
        let msg = format!("{err}");
        assert!(msg.contains("2000"));
        assert!(msg.contains("1200"));
    }
}
