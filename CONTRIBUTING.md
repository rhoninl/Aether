# Contributing to Aether

Thank you for your interest in contributing to Aether! This document provides guidelines and information for contributors.

## Getting Started

1. Fork the repository on GitHub
2. Clone your fork locally:
   ```bash
   git clone https://github.com/<your-username>/Aether.git
   cd Aether
   ```
3. Create a feature branch:
   ```bash
   git checkout -b feature/my-feature
   ```
4. Make your changes and ensure all tests pass:
   ```bash
   cargo test
   ```
5. Push to your branch and open a pull request

## Development Setup

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Code Formatting

Please ensure your code is formatted before submitting:

```bash
cargo fmt --all
```

### Linting

Run clippy to catch common mistakes:

```bash
cargo clippy --workspace -- -D warnings
```

## Pull Request Guidelines

- Keep PRs focused on a single change
- Add tests for new functionality
- Update documentation if your change affects public APIs
- Ensure `cargo test`, `cargo fmt --check`, and `cargo clippy` all pass
- Write clear commit messages that explain *why*, not just *what*

## Reporting Issues

- Use GitHub Issues to report bugs or suggest features
- Include reproduction steps for bugs
- Describe expected vs. actual behavior

## Code of Conduct

We are committed to providing a welcoming and inclusive experience for everyone. Please be respectful and constructive in all interactions.

## License

By contributing to Aether, you agree that your contributions will be licensed under the Apache License 2.0.
