//! niubash#508: a bare indexed-array name in an arithmetic expression
//! resolves through its element zero — and an UNSET element zero (an empty
//! array, or a sparse array with no [0]) evaluates 0, never the serialized
//! storage text. The pre-fix evaluator fell through to the typed store's
//! whole-array form when `array_value_at(storage, 0)` missed, so
//! `a=(); $((a))` parsed the literal `()` and died with
//! `arithmetic syntax error: operand expected (error token is "()")` —
//! first hit by oh-my-bash's fzf plugin (`eval "$(fzf --bash)"`, Ctrl+R).
//!
//! GNU oracle (WSL bash 5.3, 2026-10-22):
//!   a=();            $((a))   -> 0
//!   a=();            $((a+1)) -> 1
//!   a[5]=7;          $((a))   -> 0   (element zero unset)
//!   a=(4 5);         $((a))   -> 4
//! The element-zero read itself is the #1004/#1005 seam (empty expanded
//! SUBSCRIPT); this file pins the adjacent bare-NAME seam.

#![cfg(windows)]

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn run_script(script: &str) -> (String, i32) {
    let dir = std::env::temp_dir().join(format!(
        "rubash-508-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("scratch dir");
    let script_path: PathBuf = dir.join("probe.sh");
    fs::write(&script_path, script).expect("write probe script");
    let out = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(&script_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run rubash script");
    let _ = fs::remove_dir_all(&dir);
    let mut stdout = String::new();
    let _ = out.stdout.as_slice().read_to_string(&mut stdout);
    (stdout, out.status.code().unwrap_or(-1))
}

#[test]
fn empty_array_bare_name_evaluates_zero() {
    let (out, rc) = run_script("a=(); echo \"empty=$((a))\"\n");
    assert!(
        out.contains("empty=0"),
        "empty array bare name must evaluate 0 (GNU); got: {out} (rc {rc})"
    );
    assert!(
        !out.contains("error token"),
        "no arithmetic syntax error may surface; got: {out}"
    );
}

#[test]
fn empty_array_in_expression_evaluates_zero() {
    let (out, rc) = run_script("a=(); echo \"empty_plus=$((a+1))\"\n");
    assert!(
        out.contains("empty_plus=1"),
        "$((a+1)) with an empty a must be 1; got: {out} (rc {rc})"
    );
}

#[test]
fn sparse_array_without_element_zero_evaluates_zero() {
    let (out, rc) = run_script("declare -a a; a[5]=7; echo \"sparse=$((a))\"\n");
    assert!(
        out.contains("sparse=0"),
        "a sparse array with no [0] must evaluate 0 (GNU); got: {out} (rc {rc})"
    );
}

#[test]
fn nonempty_array_bare_name_still_reads_element_zero() {
    let (out, rc) = run_script("a=(4 5); echo \"nonempty=$((a))\"\n");
    assert!(
        out.contains("nonempty=4"),
        "a populated array must keep reading element zero; got: {out} (rc {rc})"
    );
}

#[test]
fn error_token_is_never_the_storage_form() {
    // The regression's signature: the evaluator received the serialized
    // whole-array form `()` as the operand text. Pin that the storage text
    // can never leak into a diagnostic (or an answer) through this path.
    let (out, rc) = run_script("a=(); x=$((a)); echo \"x=$x rc=$?\"\n");
    assert!(
        out.contains("x=0"),
        "assignment through the bare empty-array name must yield 0; got: {out} (rc {rc})"
    );
    assert_eq!(rc, 0, "script must succeed; stderr path saw: {out}");
}
