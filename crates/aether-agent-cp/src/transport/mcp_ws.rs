//! MCP-over-WebSocket transport.
//!
//! **Design note.** A full RFC 6455 WebSocket server would pull in
//! `tokio-tungstenite`, which is not yet in the workspace. To keep this
//! crate's dependency footprint tight and to match the rest of Aether's
//! cautious dep policy, the transport speaks newline-delimited JSON over a
//! plain TCP socket. A thin bridge (e.g. `websocat --text -`) turns that into
//! a real WebSocket for browser clients. The framing and the envelope are
//! byte-identical, so swapping in `tokio-tungstenite` later is a drop-in.
//!
//! Hidden behind the `transport-ws-tcp` feature (default-on).

#![cfg(feature = "transport-ws-tcp")]

use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use super::{decode_or_error, handle_envelope, SharedState};

/// Accept loop. Each incoming TCP connection is handled on its own task.
pub async fn run_ws(addr: &str, state: SharedState) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, peer) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, state).await {
                tracing::warn!(target: "agent_cp::ws", ?peer, error = %err, "ws connection ended with error");
            }
        });
    }
}

async fn handle_connection(stream: TcpStream, state: SharedState) -> std::io::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            return Ok(());
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let response = match decode_or_error(trimmed.as_bytes()) {
            Ok(req) => handle_envelope(req, &state.registry, &state.auth),
            Err(err_resp) => err_resp,
        };
        let bytes = serde_json::to_vec(&response).unwrap_or_default();
        write.write_all(&bytes).await?;
        write.write_all(b"\n").await?;
        write.flush().await?;
    }
}

/// Used by the binary: takes the shared state and a pre-resolved bind address
/// and blocks forever serving requests.
pub async fn serve(addr: String, state: Arc<SharedState>) -> std::io::Result<()> {
    run_ws(&addr, (*state).clone()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthConfig, AuthVerifier};
    use crate::backend::InMemoryBackend;
    use crate::tools::build_default_registry;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn state() -> SharedState {
        let backend = Arc::new(InMemoryBackend::default());
        let registry = Arc::new(build_default_registry(backend));
        let auth = AuthVerifier::new(AuthConfig {
            identity_jwks_url: "http://id".into(),
            required_role: None,
            hs256_secret: "ws-unit-test".into(),
        });
        SharedState::new(registry, auth)
    }

    #[tokio::test]
    async fn ws_listen_and_respond_tools_list() {
        let st = state();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accept_state = st.clone();
        let accept = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, accept_state).await.unwrap();
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n")
            .await
            .unwrap();
        client.shutdown().await.unwrap();
        let mut buf = Vec::new();
        client.read_to_end(&mut buf).await.unwrap();
        let text = std::str::from_utf8(&buf).unwrap().trim();
        let v: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(v.get("result").unwrap().get("tools").is_some());

        accept.await.unwrap();
    }
}
