//! Networked multiplayer prototype for the Aether VR engine.
//!
//! Integrates `aether-network` (QUIC transport) with `aether-world-runtime`
//! (tick scheduling, input buffering, state sync, sessions) into a working
//! single-server multiplayer system.

pub mod avatar;
pub mod client;
pub mod config;
pub mod protocol;
pub mod server;
pub mod simulation;

pub use avatar::AvatarState;
pub use client::{ClientError, MultiplayerClient, RemoteWorldState};
pub use config::MultiplayerConfig;
pub use protocol::{ClientMessage, PlayerId, ServerMessage};
pub use server::{MultiplayerServer, ServerError};
pub use simulation::WorldState;
