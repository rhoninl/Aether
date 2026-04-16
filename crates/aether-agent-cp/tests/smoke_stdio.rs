//! Smoke test: launches the `agent-cp` binary as a child process, pipes a
//! `tools/list` request over stdin and verifies the response shape. Also
//! covers the auth-rejection path in a second invocation.
//!
//! Uses `std::process::Command` so we don't take a dependency on
//! `assert_cmd` (the workspace isn't configured with it).

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Locate the `agent-cp` binary that Cargo built for this test. Cargo places
/// per-package binaries under `target/<profile>/` alongside the test
/// executable; `CARGO_BIN_EXE_<name>` only works when the binary is in the
/// same package. Because we're a different package, we resolve the path
/// relative to the current test executable.
fn agent_cp_binary() -> PathBuf {
    // `std::env::current_exe` points at the integration-test binary inside
    // `target/<profile>/deps/`. The sibling `agent-cp` binary lives one level up.
    let mut path = std::env::current_exe().expect("test binary path");
    while path.file_name().and_then(|s| s.to_str()) != Some("deps") {
        if !path.pop() {
            panic!("could not walk up to target/<profile>/deps from the test exe");
        }
    }
    path.pop(); // now at target/<profile>
    path.push("agent-cp");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    path
}

fn ensure_built() -> PathBuf {
    let bin = agent_cp_binary();
    if !bin.exists() {
        let status = Command::new("cargo")
            .args(["build", "-p", "agent-cp", "--quiet"])
            .status()
            .expect("cargo build -p agent-cp failed to spawn");
        assert!(status.success(), "cargo build -p agent-cp failed");
    }
    assert!(
        bin.exists(),
        "agent-cp binary still not present at {}",
        bin.display()
    );
    bin
}

fn run_with_stdin(stdin_payload: &[u8]) -> (String, String, i32) {
    let bin = ensure_built();
    let mut child = Command::new(&bin)
        .arg("--stdio")
        .arg("--no-ws")
        .arg("--no-grpc")
        .env("AETHER_AGENT_CP_MCP_STDIO", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn agent-cp");

    {
        let mut stdin = child.stdin.take().expect("child stdin");
        stdin.write_all(stdin_payload).unwrap();
        // Drop stdin so the child sees EOF and exits the stdio loop.
    }

    // Poll-with-timeout: we expect the child to exit quickly after EOF.
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut out_pipe = child.stdout.take().expect("child stdout");
    let mut err_pipe = child.stderr.take().expect("child stderr");
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                out_pipe.read_to_string(&mut stdout).ok();
                err_pipe.read_to_string(&mut stderr).ok();
                let code = status.code().unwrap_or(-1);
                return (stdout, stderr, code);
            }
            Ok(None) => {
                if std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    panic!("agent-cp did not exit within timeout; stderr={}", stderr);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("try_wait failed: {}", e),
        }
    }
}

#[test]
fn smoke_tools_list_returns_all_tools() {
    let request = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n";
    let (stdout, stderr, code) = run_with_stdin(request);
    assert_eq!(code, 0, "exit code should be 0; stderr={}", stderr);
    let first_line = stdout.lines().next().expect("no stdout line");
    let v: serde_json::Value = serde_json::from_str(first_line).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON: {}\nraw: {}\nstderr: {}",
            e, first_line, stderr
        )
    });
    let count = v
        .get("result")
        .and_then(|r| r.get("count"))
        .and_then(|c| c.as_u64())
        .expect("result.count missing");
    assert_eq!(count, 15, "expected 15 tools; got {}", count);
    // Banner on stderr must include the service name.
    assert!(
        stderr.contains("\"service\":\"agent-cp\""),
        "banner missing on stderr: {}",
        stderr
    );
}

#[test]
fn smoke_tool_call_without_auth_is_rejected() {
    let request = b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"world.create\",\"params\":{\"manifest_yaml\":\"name: hello\\n\"}}\n";
    let (stdout, _stderr, code) = run_with_stdin(request);
    assert_eq!(code, 0);
    let first_line = stdout.lines().next().expect("no stdout line");
    let v: serde_json::Value = serde_json::from_str(first_line).unwrap();
    let envelope = v
        .get("error")
        .and_then(|e| e.get("data"))
        .expect("error envelope missing");
    assert_eq!(envelope.get("code").unwrap(), "TOOL-E4010");
    assert!(
        envelope
            .get("suggested_fix")
            .and_then(|s| s.as_str())
            .is_some_and(|s| s.contains("identity")),
        "expected identity-service hint in suggested_fix"
    );
}

#[test]
fn smoke_ping_is_open() {
    let request = b"{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"ping\"}\n";
    let (stdout, _stderr, code) = run_with_stdin(request);
    assert_eq!(code, 0);
    let line = stdout.lines().next().unwrap();
    let v: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(v.get("result").unwrap(), &serde_json::json!({"ok": true}));
}
