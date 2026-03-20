//! QUIC transport backend for the Aether networking layer.
//!
//! This module provides a Quinn-based QUIC implementation that bridges the
//! existing `RuntimeTransport` trait with actual network I/O over UDP.
//!
//! # Architecture
//!
//! - [`config`] - Configuration with environment variable support
//! - [`tls`] - TLS 1.3 certificate management (self-signed dev + production)
//! - [`connection`] - Connection wrapper with framed messaging and handshake
//! - [`server`] - QUIC server accepting client connections
//! - [`client`] - QUIC client connecting to a server
//! - [`transport`] - `QuicTransport` implementing `RuntimeTransport`

pub mod client;
pub mod config;
pub mod connection;
pub mod server;
pub mod tls;
pub mod transport;

pub use client::{ClientError, QuicClient};
pub use config::QuicConfig;
pub use connection::{ConnectionError, ConnectionState, HandshakeStatus, QuicConnection};
pub use server::{QuicServer, ServerError};
pub use tls::{generate_self_signed, TlsCertPair, TlsError};
pub use transport::QuicTransport;
