//! MCP-over-stdio transport.
//!
//! Reads newline-delimited JSON-RPC requests from `stdin`, writes responses to
//! `stdout`. Tracing goes to `stderr` (including a structured startup banner)
//! so that the stdout channel stays clean for JSON.
//!
//! This is the primary transport — designed to be launched as a subprocess by
//! Claude-in-a-box or any MCP-compatible harness.

use std::io::{BufRead, BufReader, Write};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncBufReader};

use super::{decode_or_error, handle_envelope, SharedState};

/// Default per-line read/write timeout in seconds. Configurable via
/// `AETHER_AGENT_CP_STDIO_TIMEOUT_SECS`.
pub const DEFAULT_STDIO_TIMEOUT_SECS: u64 = 300;

/// Synchronous, blocking stdio loop. Used by the unit tests and by the smoke
/// harness when an async runtime is not required. Returns once stdin reports
/// EOF.
pub fn run_blocking<R: BufRead, W: Write>(
    mut reader: R,
    mut writer: W,
    state: &SharedState,
) -> std::io::Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
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
        let bytes = serde_json::to_vec(&response).unwrap_or_else(|_| b"{}".to_vec());
        writer.write_all(&bytes)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
}

/// Async version using tokio. Used by the `agent-cp` binary. Blocks the caller
/// until stdin reports EOF.
pub async fn run_stdio(state: SharedState) -> std::io::Result<()> {
    let stdin = tokio::io::stdin();
    let mut reader = AsyncBufReader::new(stdin);
    let mut stdout = tokio::io::stdout();
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
        let bytes = serde_json::to_vec(&response)
            .unwrap_or_else(|_| br#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"encode failure"}}"#.to_vec());
        stdout.write_all(&bytes).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }
}

/// Helper for the non-async entry point: spawns its own reader to ensure
/// compatibility when stdio happens to not be a pipe.
pub fn bufread_stdin() -> BufReader<std::io::Stdin> {
    BufReader::new(std::io::stdin())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthConfig, AuthVerifier};
    use crate::backend::InMemoryBackend;
    use crate::tools::build_default_registry;
    use std::io::Cursor;
    use std::sync::Arc;

    fn state() -> SharedState {
        let backend = Arc::new(InMemoryBackend::default());
        let registry = Arc::new(build_default_registry(backend));
        let auth = AuthVerifier::new(AuthConfig {
            identity_jwks_url: "http://id".into(),
            required_role: None,
            hs256_secret: "stdio-unit-test".into(),
        });
        SharedState::new(registry, auth)
    }

    #[test]
    fn blocking_loop_handles_tools_list() {
        let st = state();
        let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n";
        let mut output = Vec::<u8>::new();
        run_blocking(Cursor::new(&input[..]), &mut output, &st).unwrap();
        let line = std::str::from_utf8(&output).unwrap().trim();
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(v.get("id").unwrap(), &serde_json::json!(1));
        assert!(v.get("result").unwrap().get("tools").is_some());
    }

    #[test]
    fn blocking_loop_rejects_bad_json() {
        let st = state();
        let input = b"{not json\n";
        let mut output = Vec::<u8>::new();
        run_blocking(Cursor::new(&input[..]), &mut output, &st).unwrap();
        let line = std::str::from_utf8(&output).unwrap().trim();
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("error").is_some());
    }

    #[test]
    fn blocking_loop_skips_blank_lines() {
        let st = state();
        let input = b"\n\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n";
        let mut output = Vec::<u8>::new();
        run_blocking(Cursor::new(&input[..]), &mut output, &st).unwrap();
        let line = std::str::from_utf8(&output).unwrap().trim();
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(v.get("result").unwrap(), &serde_json::json!({"ok": true}));
    }

    #[test]
    fn auth_required_on_tool_call() {
        let st = state();
        let input = b"{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"world.create\",\"params\":{\"manifest_yaml\":\"name:x\\n\"}}\n";
        let mut output = Vec::<u8>::new();
        run_blocking(Cursor::new(&input[..]), &mut output, &st).unwrap();
        let v: serde_json::Value = serde_json::from_str(std::str::from_utf8(&output).unwrap().trim()).unwrap();
        let code = v
            .get("error")
            .and_then(|e| e.get("data"))
            .and_then(|d| d.get("code"))
            .unwrap();
        assert_eq!(code, &serde_json::json!(crate::error::codes::UNAUTHORIZED));
    }
}
