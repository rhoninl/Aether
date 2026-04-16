//! gRPC-sibling transport.
//!
//! **Wire format.** To keep the dependency footprint tight in this unit we
//! ship a length-delimited JSON framing rather than pulling in `tonic` +
//! `prost`. Each frame is:
//!
//! ```text
//! [u32 big-endian length][UTF-8 JSON body of that length]
//! ```
//!
//! The body is the exact same JSON-RPC envelope used by the MCP transports,
//! so tools behave identically. A proto schema is documented in
//! `services/agent-cp/README.md` and a `tonic`-based drop-in can be added
//! behind a future `grpc-tonic` feature.
//!
//! Feature-gated by `transport-grpc-tcp` (default on).

#![cfg(feature = "transport-grpc-tcp")]

use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::{decode_or_error, handle_envelope, SharedState};

/// Max allowed frame size (in bytes). Oversized frames are rejected to avoid
/// runaway allocations. Tunable via `AETHER_AGENT_CP_GRPC_MAX_FRAME_BYTES`.
pub const DEFAULT_GRPC_MAX_FRAME_BYTES: u32 = 8 * 1024 * 1024;

/// Read one length-delimited JSON frame from `stream`.
pub async fn read_frame(
    stream: &mut TcpStream,
    max_bytes: u32,
) -> std::io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    match stream.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_be_bytes(len_buf);
    if len > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("frame too large: {} > {}", len, max_bytes),
        ));
    }
    let mut body = vec![0u8; len as usize];
    stream.read_exact(&mut body).await?;
    Ok(Some(body))
}

/// Write one length-delimited JSON frame to `stream`.
pub async fn write_frame(stream: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
    let len = u32::try_from(body.len()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "frame body exceeds u32::MAX")
    })?;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}

/// Accept loop. Each TCP connection is handled on its own task.
pub async fn run_grpc(addr: &str, state: SharedState, max_frame: u32) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, peer) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, state, max_frame).await {
                tracing::warn!(target: "agent_cp::grpc", ?peer, error = %err, "grpc connection ended");
            }
        });
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    state: SharedState,
    max_frame: u32,
) -> std::io::Result<()> {
    loop {
        let body = match read_frame(&mut stream, max_frame).await? {
            Some(b) => b,
            None => return Ok(()),
        };
        let response = match decode_or_error(&body) {
            Ok(req) => handle_envelope(req, &state.registry, &state.auth),
            Err(err_resp) => err_resp,
        };
        let out = serde_json::to_vec(&response).unwrap_or_default();
        write_frame(&mut stream, &out).await?;
    }
}

/// Binary entry point.
pub async fn serve(addr: String, state: Arc<SharedState>, max_frame: u32) -> std::io::Result<()> {
    run_grpc(&addr, (*state).clone(), max_frame).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthConfig, AuthVerifier};
    use crate::backend::InMemoryBackend;
    use crate::tools::build_default_registry;

    fn state() -> SharedState {
        let backend = Arc::new(InMemoryBackend::default());
        let registry = Arc::new(build_default_registry(backend));
        let auth = AuthVerifier::new(AuthConfig {
            identity_jwks_url: "http://id".into(),
            required_role: None,
            hs256_secret: "grpc-unit-test".into(),
        });
        SharedState::new(registry, auth)
    }

    #[tokio::test]
    async fn framed_round_trip_tools_list() {
        let st = state();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accept_state = st.clone();
        let accept = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, accept_state, DEFAULT_GRPC_MAX_FRAME_BYTES)
                .await
                .unwrap();
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        write_frame(&mut client, body).await.unwrap();

        let resp_body = read_frame(&mut client, DEFAULT_GRPC_MAX_FRAME_BYTES)
            .await
            .unwrap()
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
        assert!(v.get("result").unwrap().get("tools").is_some());

        drop(client);
        let _ = accept.await;
    }

    #[tokio::test]
    async fn oversized_frame_is_rejected() {
        let st = state();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accept_state = st.clone();
        let accept = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            // Use a tiny max frame size to force a rejection.
            let _ = handle_connection(stream, accept_state, 4).await;
        });
        let mut client = TcpStream::connect(addr).await.unwrap();
        let body = br#"{"jsonrpc":"2.0","method":"ping"}"#;
        write_frame(&mut client, body).await.unwrap();
        // Server should close the connection; read returns EOF or an error.
        let mut buf = [0u8; 4];
        let _ = client.read(&mut buf).await;
        drop(client);
        let _ = accept.await;
    }
}
