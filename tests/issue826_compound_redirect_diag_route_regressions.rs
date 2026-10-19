//! Issue rubash#826 — a compound's own failed redirection diagnostic is
//! routed through the fd table and emitted exactly once.
//!
//! The exit status of `{ :; } > /nonexistent/f` was always correct (1); the
//! DIAGNOSTIC was wrong in three faces sharing one mechanism — the brace
//! wrapper's open-failure report bypassed the fd table the command
//! established:
//!
//! 1. `{ :; } 2>err2.txt > /nonexistent_dir/f` printed the message TWICE on
//!    the real stderr and never created err2.txt (the wrapper applied the
//!    fd-1 arm before the fd-2 arm — field order, not GNU's parse order —
//!    so the failing open happened before fd 2 was bound; the swallowed
//!    error then fell through to a re-dispatch that failed and reported a
//!    second time). GNU: one diagnostic, captured in err2.txt.
//! 2. `{ :; } > /nonexistent_dir/f` doubled the message too (same
//!    report-then-re-dispatch shape).
//! 3. `( { :; } > /nonexistent_dir/f ) 2>err.txt` captured AND leaked to the
//!    console: the wrapper's raw `eprintln!` bypassed the child's bound fd 2.
//!
//! The fix (GNU do_redirection_internal redir.c:767-955 walking the redirect
//! list in parse order, redir.c:135 redirection_error reporting through the
//! fd 2 binding in force): the brace arms run in parse order and bind fd 2
//! at group level for `2>`/`2>>` file targets; the failed open is reported
//! once through `fd_table`'s fd-2 endpoint while those bindings are in
//! force; and the brace dispatch adjudicates the wrapper failure (status 1,
//! ERR trap, errexit) WITHOUT re-dispatching the command.
//!
//! Expected outputs are byte-verified against WSL GNU Bash 5.3.0(1) via the
//! stdin oracle (2026-10-13, audit probes `compound-redirect-fail-status`,
//! `brace-redirect-err-route`, `subshell-redirect-diag-route`).

use std::path::{Path, PathBuf};
use std::process::Command;

fn rubash_in(script: &str, cwd: &Path) -> (String, String, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .current_dir(cwd)
        .output()
        .expect("run rubash");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

fn rubash(script: &str) -> (String, String, Option<i32>) {
    rubash_in(script, &std::env::temp_dir())
}

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rubash-issue826-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Face 1: the diagnostic lands in err2.txt exactly once, nothing reaches
/// the real stderr, and the file EXISTS (GNU applied `2>err2.txt` before
/// the failing `>` open). GNU: `rc=1` +
/// `err=[bash: line 1: /nonexistent_dir/f: No such file or directory]`.
#[test]
fn face1_err_capture_single_emission() {
    let dir = scratch_dir("face1");
    let (stdout, stderr, code) = rubash_in(
        "{ :; } 2>err2.txt > /nonexistent_dir/f\necho \"rc=$?\"\necho \"err=[$(cat err2.txt)]\"",
        &dir,
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(stderr, "");
    assert_eq!(
        stdout,
        "rc=1\nerr=[bash: line 1: /nonexistent_dir/f: No such file or directory]\n"
    );
    assert_eq!(code, Some(0));
}

/// Face 2: the plain shape reports the diagnostic exactly once.
#[test]
fn face2_single_emission() {
    let (stdout, stderr, code) = rubash("{ :; } > /nonexistent_dir/f");
    assert_eq!(stdout, "");
    assert_eq!(
        stderr,
        "bash: line 1: /nonexistent_dir/f: No such file or directory\n"
    );
    assert_eq!(code, Some(1));
}

/// Face 3: a subshell's redirected stderr captures the group's failure
/// diagnostic; nothing leaks to the console (the child's fd 2 binding is
/// consulted — GNU execute_in_subshell applies the parens' own redirections
/// to the child before the body runs).
#[test]
fn face3_subshell_capture_no_console_leak() {
    let dir = scratch_dir("face3");
    let (stdout, stderr, code) = rubash_in(
        "( { :; } > /nonexistent_dir/f ) 2>err.txt\necho \"err=[$(cat err.txt)]\"",
        &dir,
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(stderr, "");
    assert_eq!(
        stdout,
        "err=[bash: line 1: /nonexistent_dir/f: No such file or directory]\n"
    );
    assert_eq!(code, Some(0));
}

/// The ERR trap fires exactly once for the wrapper's own failure and the
/// status reaches the script (GNU redir12.sub face: `E` then `rc=1`).
#[test]
fn err_trap_fires_once_for_wrapper_failure() {
    let (stdout, stderr, code) =
        rubash("set -E\ntrap 'echo E' ERR\n{ :; } > /nonexistent_dir/f\necho \"rc=$?\"");
    assert_eq!(
        stderr,
        "bash: line 3: /nonexistent_dir/f: No such file or directory\n"
    );
    assert_eq!(stdout, "E\nrc=1\n");
    assert_eq!(code, Some(0));
}

/// errexit acts on the fresh status: nothing after the group runs (GNU
/// probe 2026-10-13: no `survived`).
#[test]
fn errexit_acts_on_wrapper_failure() {
    let (stdout, stderr, code) = rubash("set -e\n{ :; } > /nonexistent_dir/f\necho survived");
    assert_eq!(
        stderr,
        "bash: line 2: /nonexistent_dir/f: No such file or directory\n"
    );
    assert_eq!(stdout, "");
    assert_eq!(code, Some(1));
}

/// `2>>` also establishes the group's fd 2 (GNU opens the target with
/// O_APPEND|O_CREAT at do_redirections time): the diagnostic is appended.
#[test]
fn append_err_capture() {
    let dir = scratch_dir("append");
    let (stdout, stderr, code) = rubash_in(
        "{ :; } 2>>app.log > /nonexistent_dir/f\necho \"rc=$?\"\necho \"log=[$(cat app.log)]\"",
        &dir,
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(stderr, "");
    assert_eq!(
        stdout,
        "rc=1\nlog=[bash: line 1: /nonexistent_dir/f: No such file or directory]\n"
    );
    assert_eq!(code, Some(0));
}

/// `2>/dev/null` silences the wrapper's diagnostic like GNU (the null
/// device is the bound fd 2).
#[test]
fn devnull_err_silences_wrapper_diagnostic() {
    let (stdout, stderr, code) = rubash("{ :; } 2>/dev/null > /nonexistent_dir/f\necho \"rc=$?\"");
    assert_eq!(stderr, "");
    assert_eq!(stdout, "rc=1\n");
    assert_eq!(code, Some(0));
}

/// The face-3 shape under `2>/dev/null` is fully silent.
#[test]
fn subshell_devnull_silent() {
    let (stdout, stderr, code) =
        rubash("( { :; } > /nonexistent_dir/f ) 2>/dev/null\necho \"rc=$?\"");
    assert_eq!(stderr, "");
    assert_eq!(stdout, "rc=1\n");
    assert_eq!(code, Some(0));
}

/// An outer group's `2>` captures an inner group's failure diagnostic
/// (GNU: the outer binding is fd 2 in force for the nested wrapper).
#[test]
fn nested_brace_capture() {
    let dir = scratch_dir("nested");
    let (stdout, stderr, code) = rubash_in(
        "{ { :; } > /nonexistent_dir/f; } 2>outer.txt\necho \"rc=$?\"\necho \"outer=[$(cat outer.txt)]\"",
        &dir,
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(stderr, "");
    assert_eq!(
        stdout,
        "rc=1\nouter=[bash: line 1: /nonexistent_dir/f: No such file or directory]\n"
    );
    assert_eq!(code, Some(0));
}

/// The body never runs when the group's own redirect fails (`hi` absent) —
/// GNU abandons the command at do_redirections.
#[test]
fn body_does_not_run_on_wrapper_failure() {
    let (stdout, stderr, code) = rubash("{ echo hi; } > /nonexistent_dir/f\necho \"rc=$?\"");
    assert_eq!(
        stderr,
        "bash: line 1: /nonexistent_dir/f: No such file or directory\n"
    );
    assert_eq!(stdout, "rc=1\n");
    assert_eq!(code, Some(0));
}

/// The `&&` right-hand side is skipped on the wrapper's status (GNU:
/// `rc=1`, no SKIPPED) — the single adjudication keeps the and-or contract.
#[test]
fn and_or_rhs_skipped_on_wrapper_failure() {
    let (stdout, stderr, code) =
        rubash("{ :; } > /nonexistent_dir/f && echo SKIPPED\necho \"rc=$?\"");
    assert_eq!(
        stderr,
        "bash: line 1: /nonexistent_dir/f: No such file or directory\n"
    );
    assert_eq!(stdout, "rc=1\n");
    assert_eq!(code, Some(0));
}

/// Success path guard: `{ :; } 2>err.txt` still creates the file and runs
/// the body; the group's fd-2 binding dies with the group (nothing after
/// the group writes to err.txt).
#[test]
fn success_path_binding_dies_with_group() {
    let dir = scratch_dir("success");
    let (stdout, stderr, code) = rubash_in(
        "{ :; } 2>err.txt\necho after > later.txt\necho \"err=[$(cat err.txt)]\"\necho rc=$?",
        &dir,
    );
    let later = std::fs::read_to_string(dir.join("later.txt")).expect("later.txt exists");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(stderr, "");
    assert_eq!(stdout, "err=[]\nrc=0\n");
    assert_eq!(later, "after\n");
    assert_eq!(code, Some(0));
}
