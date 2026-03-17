//! Multiplayer demo server.
//!
//! Starts a QUIC server on the configured port and runs the tick loop.
//!
//! Configuration via environment variables:
//!   AETHER_SERVER_PORT - server port (default: 7777)
//!   AETHER_TICK_RATE   - simulation tick rate in Hz (default: 60)
//!   AETHER_MAX_PLAYERS - maximum concurrent players (default: 20)

use aether_multiplayer::{MultiplayerConfig, MultiplayerServer};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = MultiplayerConfig::from_env();
    tracing::info!(
        port = config.bind_addr.port(),
        tick_rate = config.tick_rate,
        max_players = config.max_players,
        "starting multiplayer demo server"
    );

    let server = MultiplayerServer::new(config);
    let shutdown = server.shutdown_handle();

    // Spawn shutdown signal handler
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl+c");
        tracing::info!("received ctrl+c, shutting down");
        shutdown.notify_waiters();
    });

    if let Err(e) = server.run().await {
        tracing::error!(error = %e, "server exited with error");
        std::process::exit(1);
    }
}
