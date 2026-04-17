# agent-cp

Aether Agent Control Plane — the MCP + gRPC surface that lets AI agents
author, mutate, simulate and moderate Aether worlds. Every tool returns
either a committed result or a structured error envelope that carries a
machine-applicable repair patch, so agents can loop
`tool_call -> error -> apply(repair_patch) -> retry` without a human in the
loop.

## Transports

| Transport | Wire | Default bind | Off switch |
|-----------|------|--------------|------------|
| MCP over stdio | newline-delimited JSON-RPC 2.0 | stdin / stdout | `AETHER_AGENT_CP_MCP_STDIO` unset |
| MCP over WS / TCP | newline-delimited JSON-RPC 2.0 | `0.0.0.0:7830` | `--no-ws` |
| gRPC-sibling | u32-LE length-delimited JSON-RPC 2.0 frames | `0.0.0.0:7831` | `--no-grpc` |

A thin bridge such as `websocat --text -` turns the WS-TCP transport into a
proper RFC 6455 WebSocket for browser clients; the framing and envelope are
byte-identical so a future `tokio-tungstenite` swap is a drop-in.

## Environment variables

| Var | Default | Meaning |
|-----|---------|---------|
| `AETHER_AGENT_CP_MCP_STDIO` | unset | Any non-empty value enables the stdio transport |
| `AETHER_AGENT_CP_WS_ADDR` | `0.0.0.0:7830` | Bind addr for the WS-TCP transport |
| `AETHER_AGENT_CP_GRPC_ADDR` | `0.0.0.0:7831` | Bind addr for the gRPC-sibling transport |
| `AETHER_AGENT_CP_GRPC_MAX_FRAME_BYTES` | `8388608` | Max frame size for the gRPC-sibling transport |
| `AETHER_AGENT_CP_IDENTITY_JWKS_URL` | `http://identity:8080/auth/.well-known/jwks.json` | Where the identity service's JWKS lives |
| `AETHER_AGENT_CP_REQUIRED_ROLE` | unset | When set, caller must carry this `role` claim |
| `JWT_SECRET` | _dev default_ | HS256 shared secret (reused from `aether-security`) |

## Quick start

```bash
# 1. stdio smoke: list all 15 registered tools.
AETHER_AGENT_CP_MCP_STDIO=1 \
  cargo run -p agent-cp --quiet -- --stdio < tests/fixtures/mcp_tools_list.json \
  | jq '.result.count'

# 2. auth rejection: a tool call without a bearer token returns TOOL-E4010.
echo '{"jsonrpc":"2.0","id":2,"method":"world.create","params":{"manifest_yaml":"name: hello\n"}}' \
  | cargo run -p agent-cp --quiet -- --stdio
```

## Tool surface

| Tool | Purpose |
|------|---------|
| `world.create` / `world.patch` / `world.query` | Author worlds (Bet 4 / task 90) |
| `entity.spawn` / `entity.modify` / `entity.link` | Populate worlds (task 91) |
| `script.compile` / `script.deploy` | Author behavior (task 92) |
| `sim.run` | Dry-run a scenario (task 93) |
| `ugc.upload` / `ugc.scan_status` / `ugc.approve` / `ugc.publish` | UGC lifecycle (task 94) |
| `moderation.report` | File a moderation report (task 94) |
| `telemetry.stream` | Stream telemetry events (task 95) |

Every error carries a `TOOL-E####` code plus an optional `repair_patch` (task 96).

## gRPC proto

Conceptual schema (the in-tree transport is JSON-framed; a `tonic`-based
implementation can be added behind a future `grpc-tonic` feature):

```proto
syntax = "proto3";
package aether.agent_cp.v1;

service AgentControlPlane {
  rpc Call(ToolCallRequest) returns (ToolCallResponse);
  rpc Stream(ToolCallRequest) returns (stream ToolCallResponse);
}

message ToolCallRequest {
  string jsonrpc = 1;  // always "2.0"
  string id = 2;
  string method = 3;
  string params_json = 4;
  string auth_bearer = 5;
}

message ToolCallResponse {
  string jsonrpc = 1;
  string id = 2;
  string result_json = 3;
  int32 error_code = 4;
  string error_message = 5;
  string error_envelope_json = 6;
}
```

## Docker

```bash
docker build -t aether/agent-cp:latest -f services/agent-cp/Dockerfile .
docker run --rm -p 7830:7830 -p 7831:7831 \
  -e AETHER_AGENT_CP_IDENTITY_JWKS_URL=http://host.docker.internal:8080/auth/.well-known/jwks.json \
  aether/agent-cp:latest
```
