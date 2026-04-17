# Agent-Native Demo — Recording Instructions

This page tells a human (or automation) how to produce the canonical
recorded-session artifact for task 66.

Scope: one ~60-second screen recording showing the `agent-native-demo` binary
running end-to-end, side-by-side with the emitted JSON-lines transcript. The
video itself is **not** checked into Git; only this instruction sheet is.

## What to capture

1. Terminal 1 (left): `cargo run -p agent-native-demo | tee demo.jsonl`.
2. Terminal 2 (right): `tail -f demo.jsonl | jq -c .` for live pretty-printing.
3. Terminal 3 (bottom, small): `RUST_LOG=info cargo run -p agent-native-demo`
   showing tracing output on stderr.

Each of the 8 expected steps should be clearly visible as it crosses the
screen:

| # | Step            | What the viewer sees                                 |
|---|-----------------|------------------------------------------------------|
| 1 | `mcp.connect`   | transport name                                       |
| 2 | `world.create`  | world CID, name, chunk count                         |
| 3 | `entity.spawn`  | entity id, kind, position                            |
| 4 | `script.compile`| script CID, wasm length, verb count                  |
| 5 | `script.deploy` | entity id + script CID binding                       |
| 6 | `sim.run`       | scenario name, ticks run, verdict = pass             |
| 7 | `vcs.merge`     | base, head, signer, merge CID — the human-review diff|
| 8 | `done`          | final verdict + merge CID                            |

## Tooling

- macOS: built-in Screenshot.app (`Cmd+Shift+5` → Record Selected Portion).
- Linux: `peek` or `wf-recorder`.
- Cross-platform: OBS with a 1080p canvas at 30 fps.

Export as H.264 MP4, target bitrate 4 Mbps, mono audio (optional narration or
silent).

## What NOT to include

- No source code editing: the whole point is that the agent does not touch
  Rust.
- No secrets: confirm `AETHER_DEMO_*` env vars don't include API keys on
  screen.
- No git history window: this demo is about the engine surface, not vcs
  plumbing.

## Upload destination

Per task 66, drop the final MP4 plus the captured `demo.jsonl` into the
release channel called out in the batch coordinator's instructions. Do
**not** commit binary video to this repo.

## Quick script

```bash
#!/usr/bin/env bash
set -euo pipefail
cargo build -p agent-native-demo
# Start recording (tool-specific, see above).
cargo run -p agent-native-demo | tee /tmp/demo.jsonl
# Stop recording. Trim to ~60 s. Upload video + /tmp/demo.jsonl.
```

## Verification checklist

- [ ] Final JSON-lines record reads `{"step":"done","verdict":"pass","merge_cid":"cid:v1:…"}`.
- [ ] Wall-clock elapsed on the final record is under 5000 ms.
- [ ] All 7 step records show `ok: true`.
- [ ] The `vcs.merge` record shows a `signer` field (human-review gate).
