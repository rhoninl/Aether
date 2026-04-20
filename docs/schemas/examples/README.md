# Canonical schema examples

Paired with `../*.v1.json`. Each example is hand-authored, round-trips with its
matching Rust type in `aether-schemas`, and produces a stable content address.

| File                               | Rust type (`aether_schemas`)                |
| ---------------------------------- | ------------------------------------------- |
| `world-manifest.example.yaml`      | `WorldManifest`                              |
| `entity.example.yaml`              | `Entity`                                     |
| `prop.example.yaml`                | `Prop`                                       |
| `chunk-manifest.example.yaml`      | `ChunkManifest`                              |
| `script-artifact.example.yaml`     | `ScriptArtifact`                             |

These files are the training corpus for agents authoring Aether artifacts — a
Claude agent given the matching JSON Schema plus the example is expected to
produce a valid artifact on first try.
