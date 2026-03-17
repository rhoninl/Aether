//! ECS systems that bridge engine subsystems through shared components.
//!
//! Each system reads/writes specific components and resources, connecting
//! isolated crates (renderer, physics, network, scripts, hot-reload) into
//! a unified game loop.

pub mod hot_reload;
pub mod input;
pub mod network;
pub mod physics;
pub mod render;
pub mod scripting;
