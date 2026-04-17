# agent-native-demo

Thin-slice end-to-end proof (task 66 / unit U10) that the Aether engine can be
driven, from first MCP byte to promoted merge, by an AI agent.

```bash
cargo run -p agent-native-demo
```

Prints one JSON-lines record per step and exits 0. The full flow:

1. `mcp.connect`    — open stdio MCP transport
2. `world.create`   — submit `fixtures/hello.world.yaml`
3. `entity.spawn`   — batch-spawn one cube at `(0, 1, 0)`
4. `script.compile` — compile `fixtures/patrol.beh` to WASM
5. `script.deploy`  — attach the script to the cube
6. `sim.run`        — run `fixtures/patrol.scenario.yaml` (10 ticks) → `Pass`
7. `vcs.merge`      — sign a diff from genesis → head, merge into `main`
8. `done`           — final verdict + merge CID

Wall-clock budget: under 5 s with stubs, under 5 min with real wiring.

## Configuration

| Env var                              | Default                                                |
| ------------------------------------ | ------------------------------------------------------ |
| `AETHER_DEMO_WORLD_FIXTURE_PATH`     | `examples/agent-native-demo/fixtures/hello.world.yaml` |
| `AETHER_DEMO_BEHAVIOR_FIXTURE_PATH`  | `examples/agent-native-demo/fixtures/patrol.beh`       |
| `AETHER_DEMO_SCENARIO_FIXTURE_PATH`  | `examples/agent-native-demo/fixtures/patrol.scenario.yaml` |
| `AETHER_DEMO_AGENT_CP_ADDR`          | `stdio`                                                |
| `AETHER_DEMO_SIGNER_ID`              | `agent:demo`                                           |
| `AETHER_DEMO_TARGET_BRANCH`          | `main`                                                 |
| `RUST_LOG`                           | `info`                                                 |

## Feature flags

| Feature  | Default | Purpose                                                   |
| -------- | ------- | --------------------------------------------------------- |
| `stubs`  | yes     | Run against `src/stubs.rs` (in-tree, mirrors five units) |
| `real`   | no      | Placeholder — flip on when U03/U05/U07/U08/U09 have merged |

### Post-batch handoff

When these five units land on `main`:

- **U03** `aether-schemas` — `WorldManifest`, `Cid`, `ContentAddress`
- **U05** `aether-sim-harness` — `Harness::run`, `SimReport`, `Verdict`
- **U07** `aether-behavior-dsl` — parse + compile to WASM for 5 verbs
- **U08** `aether-agent-cp` + `services/agent-cp` — MCP tool registry
- **U09** `aether-world-vcs` — `Diff`, `Branch`, `merge`, `sign`

flip the default feature in `Cargo.toml`:

```toml
[features]
default = ["real"]
real = [
  "dep:aether-schemas",
  "dep:aether-sim-harness",
  "dep:aether-behavior-dsl",
  "dep:aether-agent-cp",
  "dep:aether-world-vcs",
]
```

delete `src/stubs.rs`, and swap the `use stubs::*` imports in `src/main.rs`
for the real crate paths. See `docs/design/agent-native-demo.md` for the
one-page checklist.

## Tests

```bash
cargo test -p agent-native-demo
```

`tests/end_to_end.rs` runs the full flow in-process and asserts:

- all 7 step records in order, each with `ok: true`
- a final `{"step":"done","verdict":"pass","merge_cid":"cid:v1:…"}`
- wall clock under 5 s
