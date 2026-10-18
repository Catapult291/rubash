//! rubash#723 (variant): an unbounded external producer feeding a pipeline
//! whose rightmost stage is served in-shell (a `while read` loop), with a
//! multi-member external prefix (`yes | head -50 | while read ...`).
//!
//! The Unix port of the #761 streaming-tail machinery
//! (`execute_external_prefix_streaming_tail`) must run the external prefix
//! on real OS pipes and let the in-shell tail read the live stream. The
//! pre-fix sequential loop materialized the unbounded producer's whole
//! output into RAM (8+ GB RSS, OOM-killed or hung).
#![cfg(unix)]

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Deadline for one pipeline run. The fixed binary finishes every shape here
/// in well under a second; the pre-fix binary never returns (or is OOM-killed).
const RUN_LIMIT: Duration = Duration::from_secs(20);

struct RunOutcome {
    timed_out: bool,
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_rubash_script(script: &str) -> RunOutcome {
    let dir = std::env::temp_dir().join(format!("rubash-issue723-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create issue723 scratch dir");
    let script_path = dir.join("probe.sh");
    let mut file = std::fs::File::create(&script_path).expect("create probe script");
    file.write_all(script.as_bytes())
        .expect("write probe script");
    drop(file);

    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(&script_path)
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("BASH_ENV")
        .spawn()
        .expect("spawn rubash");

    let stdout_pipe = child.stdout.take().expect("piped stdout");
    let stderr_pipe = child.stderr.take().expect("piped stderr");
    let stdout_reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let mut pipe = stdout_pipe;
        let _ = std::io::Read::read_to_end(&mut pipe, &mut buffer);
        buffer
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let mut pipe = stderr_pipe;
        let _ = std::io::Read::read_to_end(&mut pipe, &mut buffer);
        buffer
    });

    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait().expect("poll rubash") {
            Some(status) => break Some(status),
            None => {
                if started.elapsed() >= RUN_LIMIT {
                    timed_out = true;
                    let _ = child.kill();
                    break child.wait().ok();
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    };
    let stdout = stdout_reader.join().expect("join stdout reader");
    let stderr = stderr_reader.join().expect("join stderr reader");
    let _ = std::fs::remove_dir_all(&dir);
    RunOutcome {
        timed_out,
        code: status.and_then(|status| status.code()),
        stdout,
        stderr,
    }
}

/// Resolve an external tool to an absolute path so the probe does not depend
/// on the child's PATH.
fn resolve_tool(name: &str) -> String {
    for dir in std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|entry| !entry.is_empty())
    {
        let path = PathBuf::from(format!("{dir}/{name}"));
        if path.is_file() {
            return path.to_string_lossy().into_owned();
        }
    }
    panic!("issue723 tests need a real external `{name}` on PATH");
}

fn assert_terminates_ok(label: &str, out: &RunOutcome) {
    assert!(
        !out.timed_out,
        "`{label}` did not terminate within {RUN_LIMIT:?} (rubash#723 unbounded-producer hang)"
    );
    assert_eq!(
        out.code,
        Some(0),
        "`{label}` exit status, stderr: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The issue's reproducer: a two-member external prefix feeding a
/// `while read` tail. Must print `done` immediately, not buffer `yes`
/// into memory.
#[test]
fn unbounded_multi_prefix_while_read_tail_terminates() {
    let yes = resolve_tool("yes");
    let head = resolve_tool("head");
    let out = run_rubash_script(&format!(
        "\"{yes}\" | \"{head}\" -50 | while read -r l; do :; done; echo done\n"
    ));
    assert_terminates_ok("yes | head -50 | while read", &out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "done\n",
        "the while-read tail must see EOF once head exits"
    );
}

/// The tail must actually receive the prefix's bytes through the live pipe.
#[test]
fn multi_prefix_while_read_tail_gets_data() {
    let yes = resolve_tool("yes");
    let head = resolve_tool("head");
    let out = run_rubash_script(&format!(
        "\"{yes}\" | \"{head}\" -5 | while read -r l; do echo \"got:$l\"; done\n"
    ));
    assert_terminates_ok("yes | head -5 | while read; echo", &out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "got:y\ngot:y\ngot:y\ngot:y\ngot:y\n",
        "the compound tail must read the producer's stream"
    );
}

/// GNU anchor: the unbounded producer dies on SIGPIPE (141) once the
/// bounding consumer exits; the tail reports its own status.
#[test]
fn multi_prefix_while_read_pipestatus_is_sigpipe() {
    let yes = resolve_tool("yes");
    let head = resolve_tool("head");
    let out = run_rubash_script(&format!(
        "\"{yes}\" | \"{head}\" -50 | while read -r l; do :; done\necho \"PS=${{PIPESTATUS[0]}},${{PIPESTATUS[1]}},${{PIPESTATUS[2]}}\"\n"
    ));
    assert_terminates_ok("yes | head -50 | while read PIPESTATUS", &out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "PS=141,0,0\n",
        "GNU anchor: `yes | head -50 | while read` reports PIPESTATUS (141 0 0)"
    );
}

/// Single external producer with a compound tail (`break` closes the read
/// end; the producer must die rather than be drained).
#[test]
fn unbounded_single_prefix_while_read_break_terminates() {
    let yes = resolve_tool("yes");
    let out = run_rubash_script(&format!(
        "\"{yes}\" | while read -r l; do break; done; echo done\n"
    ));
    assert_terminates_ok("yes | while read; do break; done", &out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "done\n");
}

/// A large but finite producer through the same shape still drains fully.
#[test]
fn finite_prefix_while_read_tail_terminates() {
    let seq = resolve_tool("seq");
    let out = run_rubash_script(&format!(
        "\"{seq}\" 100000 | while read -r l; do :; done; echo done\n"
    ));
    assert_terminates_ok("seq 100000 | while read", &out);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "done\n");
}
