//! `agent-cp`: Aether Agent Control Plane binary.
//!
//! Spins up the MCP-stdio, MCP-WS (TCP-framed newline JSON) and gRPC-sibling
//! (length-delimited JSON) transports, all sharing the same tool registry and
//! auth verifier. Every runtime knob is driven by an environment variable;
//! zero hard-coded URLs, ports or secrets.
//!
//! Banner-on-stderr JSON is emitted at startup so orchestration tooling can
//! parse the listen endpoints without tailing logs.

use std::env;
use std::sync::Arc;

use aether_agent_cp::{
    auth::AuthVerifier, backend::InMemoryBackend, tools::build_default_registry,
    transport::SharedState,
};
use clap::Parser;
use serde_json::json;

// --- constants --------------------------------------------------------------

/// Env var enabling the stdio transport. Any non-empty value turns it on.
const ENV_MCP_STDIO: &str = "AETHER_AGENT_CP_MCP_STDIO";
/// Env var for the WS-TCP bind address.
const ENV_WS_ADDR: &str = "AETHER_AGENT_CP_WS_ADDR";
/// Env var for the gRPC-sibling bind address.
const ENV_GRPC_ADDR: &str = "AETHER_AGENT_CP_GRPC_ADDR";
/// Env var for the gRPC transport's max frame size.
const ENV_GRPC_MAX_FRAME: &str = "AETHER_AGENT_CP_GRPC_MAX_FRAME_BYTES";

const DEFAULT_WS_ADDR: &str = "0.0.0.0:7830";
const DEFAULT_GRPC_ADDR: &str = "0.0.0.0:7831";

// --- CLI --------------------------------------------------------------------

/// Command-line overrides. All flags are optional; when omitted the env vars
/// (or their defaults) take over.
#[derive(Debug, Parser)]
#[command(
    name = "agent-cp",
    version,
    about = "Aether Agent Control Plane: MCP + gRPC agent-native authoring surface"
)]
struct Cli {
    /// Force stdio mode (equivalent to `AETHER_AGENT_CP_MCP_STDIO=1`).
    #[arg(long)]
    stdio: bool,
    /// Disable the WS-TCP transport.
    #[arg(long)]
    no_ws: bool,
    /// Disable the gRPC-sibling transport.
    #[arg(long)]
    no_grpc: bool,
    /// Override WS bind address.
    #[arg(long)]
    ws_addr: Option<String>,
    /// Override gRPC bind address.
    #[arg(long)]
    grpc_addr: Option<String>,
}

fn main() -> std::io::Result<()> {
    install_tracing();
    let cli = Cli::parse();

    let backend = Arc::new(InMemoryBackend::default());
    let registry = Arc::new(build_default_registry(backend));
    let auth = AuthVerifier::from_env();
    let state = Arc::new(SharedState::new(registry.clone(), auth.clone()));

    let want_stdio = cli.stdio || env::var(ENV_MCP_STDIO).is_ok_and(|v| !v.is_empty());
    let ws_addr = if cli.no_ws {
        None
    } else {
        Some(
            cli.ws_addr
                .clone()
                .unwrap_or_else(|| env::var(ENV_WS_ADDR).unwrap_or_else(|_| DEFAULT_WS_ADDR.into())),
        )
    };
    let grpc_addr = if cli.no_grpc {
        None
    } else {
        Some(
            cli.grpc_addr
                .clone()
                .unwrap_or_else(|| env::var(ENV_GRPC_ADDR).unwrap_or_else(|_| DEFAULT_GRPC_ADDR.into())),
        )
    };
    let max_frame = env::var(ENV_GRPC_MAX_FRAME)
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(aether_agent_cp::transport::grpc::DEFAULT_GRPC_MAX_FRAME_BYTES);

    emit_banner(&registry, &auth, want_stdio, ws_addr.as_deref(), grpc_addr.as_deref());

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let mut handles: Vec<tokio::task::JoinHandle<std::io::Result<()>>> = Vec::new();

        if want_stdio {
            let state = state.clone();
            handles.push(tokio::spawn(async move {
                aether_agent_cp::transport::mcp_stdio::run_stdio((*state).clone()).await
            }));
        }

        if let Some(addr) = ws_addr {
            let state = state.clone();
            handles.push(tokio::spawn(async move {
                aether_agent_cp::transport::mcp_ws::serve(addr, state).await
            }));
        }

        if let Some(addr) = grpc_addr {
            let state = state.clone();
            handles.push(tokio::spawn(async move {
                aether_agent_cp::transport::grpc::serve(addr, state, max_frame).await
            }));
        }

        if handles.is_empty() {
            tracing::error!(target: "agent_cp", "no transports enabled; exiting");
            return Ok::<(), std::io::Error>(());
        }

        // First transport to exit / error decides the process exit code.
        let (res, _idx, _rest) = futures_select(handles).await;
        match res {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
        }
    })
}

// Tiny helper: awaits the first of N tasks to finish. Written inline to avoid
// pulling in `futures` just for `select_all`.
async fn futures_select<T>(
    mut handles: Vec<tokio::task::JoinHandle<T>>,
) -> (
    Result<T, tokio::task::JoinError>,
    usize,
    Vec<tokio::task::JoinHandle<T>>,
)
where
    T: Send + 'static,
{
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    struct FirstOf<'a, T>(&'a mut Vec<tokio::task::JoinHandle<T>>);
    impl<'a, T> Future for FirstOf<'a, T>
    where
        T: Send + 'static,
    {
        type Output = (Result<T, tokio::task::JoinError>, usize);
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.get_mut();
            for (idx, h) in this.0.iter_mut().enumerate() {
                if let Poll::Ready(r) = Pin::new(h).poll(cx) {
                    return Poll::Ready((r, idx));
                }
            }
            Poll::Pending
        }
    }

    let (res, idx) = FirstOf(&mut handles).await;
    let _ = handles.remove(idx);
    (res, idx, handles)
}

fn install_tracing() {
    // Subscribers write to stderr; stdout is reserved for the stdio transport.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

fn emit_banner(
    registry: &aether_agent_cp::ToolRegistry,
    auth: &AuthVerifier,
    stdio: bool,
    ws_addr: Option<&str>,
    grpc_addr: Option<&str>,
) {
    let banner = json!({
        "service": "agent-cp",
        "version": env!("CARGO_PKG_VERSION"),
        "endpoints": {
            "stdio": stdio,
            "ws_addr": ws_addr,
            "grpc_addr": grpc_addr,
        },
        "identity_jwks_url": auth.jwks_url(),
        "tools": registry.tool_names(),
    });
    // eprintln is the only non-tracing write we allow — it's the documented
    // banner, not a log line.
    eprintln!("{}", banner);
}
