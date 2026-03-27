//! Regression test: engine binary must send a response for the Close command
//! before exiting. Previously it called std::process::exit(0) without responding.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use braille_wire::{DaemonCommand, EngineMessage, HostMessage};

fn engine_binary_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().expect("cannot determine current executable path");
    let dir = exe.parent().expect("executable has no parent directory");

    let candidate = dir.join("braille-engine");
    if candidate.exists() {
        return candidate;
    }
    if let Some(parent) = dir.parent() {
        let candidate = parent.join("braille-engine");
        if candidate.exists() {
            return candidate;
        }
    }
    std::path::PathBuf::from("braille-engine")
}

#[test]
fn engine_responds_to_close_before_exiting() {
    let engine_bin = engine_binary_path();
    let mut child = Command::new(&engine_bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn engine at {}: {e}", engine_bin.display()));

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Send Close command
    let msg = HostMessage::Command(DaemonCommand::Close);
    let json = serde_json::to_string(&msg).unwrap();
    writeln!(stdin, "{json}").unwrap();
    stdin.flush().unwrap();

    // Engine MUST send a response before exiting
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    assert!(!line.is_empty(), "engine should send a response for Close");

    let engine_msg: EngineMessage = serde_json::from_str(line.trim()).unwrap();
    match engine_msg {
        EngineMessage::CommandResult(resp) => {
            assert!(resp.success, "Close response should be success, got: {:?}", resp.error);
        }
        other => panic!("expected CommandResult, got: {other:?}"),
    }

    // Engine should exit cleanly
    let status = child.wait().unwrap();
    assert!(status.success(), "engine should exit with status 0");
}

#[test]
fn engine_handles_snap_then_close() {
    let engine_bin = engine_binary_path();
    let mut child = Command::new(&engine_bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn engine at {}: {e}", engine_bin.display()));

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Send Snap command first
    let snap_msg = HostMessage::Command(DaemonCommand::Snap {
        mode: braille_wire::SnapMode::Compact,
    });
    let json = serde_json::to_string(&snap_msg).unwrap();
    writeln!(stdin, "{json}").unwrap();
    stdin.flush().unwrap();

    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    let snap_response: EngineMessage = serde_json::from_str(line.trim()).unwrap();
    assert!(matches!(snap_response, EngineMessage::CommandResult(ref r) if r.success),
        "Snap should succeed");

    // Now send Close
    let close_msg = HostMessage::Command(DaemonCommand::Close);
    let json = serde_json::to_string(&close_msg).unwrap();
    writeln!(stdin, "{json}").unwrap();
    stdin.flush().unwrap();

    let mut line2 = String::new();
    stdout.read_line(&mut line2).unwrap();
    assert!(!line2.is_empty(), "engine should respond to Close after Snap");

    let close_response: EngineMessage = serde_json::from_str(line2.trim()).unwrap();
    match close_response {
        EngineMessage::CommandResult(resp) => {
            assert!(resp.success, "Close should succeed after Snap");
        }
        other => panic!("expected CommandResult for Close, got: {other:?}"),
    }

    let status = child.wait().unwrap();
    assert!(status.success());
}
