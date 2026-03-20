//! Multiplayer demo client.
//!
//! Connects to the server and sends simulated avatar position updates,
//! printing received world state to stdout.
//!
//! Usage:
//!   mp-client [server_addr]
//!
//! Default server address: 127.0.0.1:7777

use std::net::SocketAddr;
use std::time::Duration;

use aether_multiplayer::{AvatarState, MultiplayerClient};

/// Default server address.
const DEFAULT_SERVER_ADDR: &str = "127.0.0.1:7777";

/// Interval between simulated input sends.
const INPUT_SEND_INTERVAL_MS: u64 = 50;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let server_addr: SocketAddr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| {
            std::env::var("AETHER_SERVER_ADDR").unwrap_or_else(|_| DEFAULT_SERVER_ADDR.to_string())
        })
        .parse()
        .unwrap_or_else(|_| DEFAULT_SERVER_ADDR.parse().unwrap());

    tracing::info!(server = %server_addr, "connecting to multiplayer server");

    let mut client =
        MultiplayerClient::new(server_addr).expect("failed to create multiplayer client");

    match client.connect().await {
        Ok(server_tick) => {
            tracing::info!(server_tick = server_tick, "connected to server");
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to connect");
            std::process::exit(1);
        }
    }

    // Simulate movement: walk in a circle
    let mut angle: f32 = 0.0;
    let radius: f32 = 5.0;
    let mut tick_count: u64 = 0;

    loop {
        tokio::time::sleep(Duration::from_millis(INPUT_SEND_INTERVAL_MS)).await;

        angle += 0.02;
        tick_count += 1;

        let avatar = AvatarState {
            head_position: [angle.cos() * radius, 1.7, angle.sin() * radius],
            head_rotation: [0.0, (angle / 2.0).sin(), 0.0, (angle / 2.0).cos()],
            left_hand_position: [angle.cos() * radius - 0.3, 1.0, angle.sin() * radius - 0.3],
            left_hand_rotation: [0.0, 0.0, 0.0, 1.0],
            right_hand_position: [angle.cos() * radius + 0.3, 1.0, angle.sin() * radius - 0.3],
            right_hand_rotation: [0.0, 0.0, 0.0, 1.0],
        };

        if let Err(e) = client.send_input(avatar).await {
            tracing::error!(error = %e, "failed to send input");
            break;
        }

        // Print state every 60 ticks (~3 seconds)
        if tick_count.is_multiple_of(60) {
            let state = client.world_state().await;
            tracing::info!(
                tick = state.last_tick,
                players = state.avatars.len(),
                "world state"
            );
            for (pid, avatar) in &state.avatars {
                tracing::info!(
                    player = %pid,
                    x = format!("{:.2}", avatar.head_position[0]),
                    y = format!("{:.2}", avatar.head_position[1]),
                    z = format!("{:.2}", avatar.head_position[2]),
                    "avatar position"
                );
            }
        }
    }

    client.shutdown();
}
