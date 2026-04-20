# agent-native-demo

Thin-slice end-to-end proof (task 66 / unit U10) that the Aether engine can be
driven, from first tool call to promoted merge, by an AI agent.

```bash
cargo run -p agent-native-demo
```

Prints one JSON-lines record per step and exits 0. The full flow:

1. `mcp.connect`    — build the `aether-agent-cp` ToolRegistry in-process
2. `world.create`   — submit `fixtures/hello.world.yaml`
3. `entity.spawn`   — batch-spawn one cube at `(0, 1, 0)`
4. `script.compile` — compile `fixtures/patrol.beh` to WASM bytes
5. `script.deploy`  — attach the script to the cube
6. `sim.run`        — run `fixtures/patrol.scenario.yaml` (10 ticks) → `pass`
7. `vcs.merge`      — sign a diff with Ed25519, move `main` head via
                      `aether-world-vcs::MemoryBranchStore`
8. `done`           — final verdict + merge CID

Typical wall-clock: ~2 ms on a laptop; hard ceiling 5 s (enforced by
`tests/end_to_end.rs`).

## Configuration

| Env var                              | Default                                                |
| ------------------------------------ | ------------------------------------------------------ |
| `AETHER_DEMO_WORLD_FIXTURE_PATH`     | `examples/agent-native-demo/fixtures/hello.world.yaml` |
| `AETHER_DEMO_BEHAVIOR_FIXTURE_PATH`  | `examples/agent-native-demo/fixtures/patrol.beh`       |
| `AETHER_DEMO_SCENARIO_FIXTURE_PATH`  | `examples/agent-native-demo/fixtures/patrol.scenario.yaml` |
| `AETHER_DEMO_AGENT_CP_ADDR`          | `in-process://tool-registry`                           |
| `AETHER_DEMO_SIGNER_ID`              | `agent:demo`                                           |
| `AETHER_DEMO_TARGET_BRANCH`          | `main`                                                 |
| `RUST_LOG`                           | `info`                                                 |

## How it wires

Steps 2–6 dispatch through `aether_agent_cp::ToolRegistry::call(name, params)`
against `InMemoryBackend::default()` — the same code path services/agent-cp
serves over stdio / WebSocket / gRPC, just plumbed in-process so `cargo run`
has no server dependency. Swapping `InMemoryBackend` for the `wire`-feature
adapter on agent-cp (which routes into `aether-schemas`, `aether-sim-harness`,
`aether-behavior-dsl`, `aether-ugc`) is the remaining follow-up; the demo's
contract does not change.

Step 7 uses `aether-world-vcs` directly: generate an Ed25519 keypair, build a
`Diff { base: 0_cid, target: sha256(world_cid), ops: [], ... }`, sign it,
verify the signature round-trips, then move the `main` branch head in a
`MemoryBranchStore`. The emitted `merge_cid` is `cid:v1:<sha256 of the diff's
canonical CBOR>`.

## Tests

```bash
cargo test -p agent-native-demo
```

`tests/end_to_end.rs` runs the full flow in-process and asserts:

- all 7 step records in order, each with `ok: true`
- a final `{"step":"done","verdict":"pass","merge_cid":"cid:v1:…"}`
- wall clock under 5 s

## Follow-ups

- Wire `services/agent-cp` as a real MCP server and run the demo over its
  stdio transport (today it is called in-process for hermeticity).
- Flip `aether-agent-cp`'s `wire` feature to route through the five real
  crates end-to-end instead of `InMemoryBackend`. See `aether-agent-cp`'s
  `backend.rs` module doc for the adapter contract.
- Swap the minimal `on tick do …` behavior source for a full 5-verb block
  (task #86) once agent-cp's backend consumes `aether-behavior-dsl`.
