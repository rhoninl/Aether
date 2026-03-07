<p align="center">
  <img src="assets/logo.png" alt="Aether Logo" width="400">
</p>

<h1 align="center">Aether</h1>

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)]()

A modular, open-source VR engine built in Rust for creating immersive virtual worlds.

> **Status:** Early development (v0.1.0). APIs are unstable and subject to change.

## Why Aether?

Building virtual worlds today means stitching together dozens of disparate libraries, dealing with C++ interop, and fighting runtime crashes. Aether takes a different approach:

- **Rust-native from day one** — Memory safety, fearless concurrency, and zero-cost abstractions without sacrificing performance.
- **Modular by design** — Every subsystem lives in its own crate. Use only what you need, swap out what you don't.
- **Built for multiplayer** — Networking and state synchronization are first-class citizens, not an afterthought.
- **Social VR focus** — Avatars, social graphs, economies, and user-generated content are part of the core engine, not plugins.

## Features

### Core Engine

- **Entity Component System** — Archetype-based ECS with parallel iteration via [rayon](https://crates.io/crates/rayon)
- **Physics** — 3D rigid-body physics powered by [Rapier3D](https://rapier.rs/)
- **Renderer** — Rendering policy primitives and software rasterizer for prototyping
- **Audio** — Spatial audio subsystem
- **Input** — Cross-platform input handling
- **Scripting** — User-facing scripting layer

### World & Networking

- **World Runtime** — World loading, streaming, and lifecycle management
- **Networking** — State synchronization and networking primitives
- **Zoning** — Spatial partitioning and zone management
- **Federation** — Cross-instance world federation

### Social & Economy

- **Avatars** — Avatar representation and customization
- **Social** — Social graph, presence, and communication
- **Economy** — Virtual economy and transaction primitives
- **UGC** — User-generated content pipeline
- **Asset Pipeline** — Asset import, processing, and optimization

### Platform & Safety

- **Security** — Access control and authentication
- **Trust & Safety** — Content moderation and compliance
- **Gateway** — API gateway and service registry
- **Deployment** — Deployment and orchestration tooling

## Quick Start

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)

### Build

```bash
cargo build
```

### Run Tests

```bash
cargo test
```

### Run the 3D Demo

An interactive demo showcasing the ECS, physics, and renderer working together:

```bash
cargo run -p aether-3d-demo
```

<p align="center">
  <img src="assets/3d-demo.png" alt="3D Demo Screenshot" width="600">
</p>

| Key | Action |
|-----|--------|
| `W` `A` `S` `D` | Move |
| Arrow keys | Orbit camera |
| `Q` / `E` | Zoom in / out |
| `ESC` | Quit |

## Roadmap

Aether is under active development. Here's a high-level view of what's planned:

- [x] Core ECS with parallel queries
- [x] Rapier3D physics integration
- [x] Software renderer and interactive 3D demo
- [x] Input handling subsystem
- [ ] GPU-accelerated rendering (wgpu)
- [ ] Networked multiplayer prototype
- [ ] VR headset support (OpenXR)
- [ ] Scripting runtime integration
- [ ] Asset hot-reloading
- [ ] First public release

## Contributing

Contributions are welcome! Whether it's a bug report, feature request, or pull request — all forms of participation are appreciated.

Please see [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding guidelines, and the PR process.

## Community

- **Issues** — [GitHub Issues](../../issues) for bug reports and feature requests
- **Discussions** — [GitHub Discussions](../../discussions) for questions and ideas

## License

Aether is licensed under the [Apache License, Version 2.0](LICENSE).

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project by you shall be licensed under Apache-2.0, without any additional terms or conditions.
