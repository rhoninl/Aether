# CLI Subcommands - Design Document

## Background

The `aether` CLI currently only supports `run` (launch examples) and `version`. End-developers need project scaffolding, a local dev server, and validation tools to build worlds without deep Rust knowledge.

## Why

- `aether new` removes boilerplate for starting a new world project
- `aether serve` provides a local development workflow without external tooling
- `aether check` catches manifest and script errors before deployment

## What

Three new subcommands:

1. **`aether new <name>`** — Scaffold a new world project directory with manifest, starter Lua script, and assets folder
2. **`aether serve [path]`** — Start a local dev server that watches and serves a world project
3. **`aether check [path]`** — Validate a world manifest and check script references

## How

### `aether new <name>`

```
aether new my-world
```

Creates:
```
my-world/
├── world.toml          # World manifest
├── scripts/
│   └── main.lua        # Starter Lua script
└── assets/             # Empty assets directory
```

### `aether serve [path]`

```
aether serve              # serves current directory
aether serve ./my-world   # serves specified path
aether serve --port 8080  # custom port (default: 3000)
```

Reads `world.toml`, starts a TCP listener, and serves world info + assets over HTTP.

### `aether check [path]`

```
aether check              # checks current directory
aether check ./my-world   # checks specified path
```

Validates:
- `world.toml` exists and is parseable
- Referenced script files exist
- Required fields (name, version) are present

### Test Design

- `new`: verify directory structure, file contents, error on existing directory
- `serve`: verify server binds, responds to health check, shuts down cleanly
- `check`: verify pass on valid manifest, fail on missing/malformed manifest, fail on missing scripts
