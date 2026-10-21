//! Regression tests for word-side byte-spelling in pattern matching.
//!
//! GNU matches bytes, not chars (subst.c glob_pattern_patscan walks
//! unsigned char buffers). An expanded word can carry a multi-char
//! spelling of a single byte (raw-byte marker pair), and the matcher must
//! fold it before verdicting — otherwise `[[ $x == ? ]]` wrongly fails
//! against a one-byte word (the two-char marker spelling never equals one
//! `?` under plain char-wise matching).
//!
//! GNU oracle: bash 5.3.20, script-file probes (verified 2026-10-22):
//!   x=$'\x11'; [[ $x == ? ]]          -> match     (was: NOMATCH)
//!   x=$'\x11'; [[ $x == ?? ]]         -> NOMATCH
//!   x=$'\x11'; case $x in ?) single   -> single
//!   plain words keep working as before.

use std::io::Write;
use std::process::{Command, Stdio};

fn rubash_stdin(script: &str) -> (String, String, Option<i32>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rubash stdin");
    child
        .stdin
        .as_mut()
        .expect("stdin pipe")
        .write_all(script.as_bytes())
        .expect("write stdin script");
    let output = child.wait_with_output().expect("collect stdin run");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

#[test]
fn carried_byte_matches_single_char_bracket_pattern() {
    let script = "x=$'\\x11'\nif [[ $x == ? ]]; then echo match; else echo NOMATCH; fi\n";
    let (stdout, stderr, code) = rubash_stdin(script);
    assert_eq!(code, Some(0), "stderr: {stderr}");
    assert_eq!(stdout, "match\n", "stderr: {stderr}");
}

#[test]
fn carried_byte_does_not_match_two_char_pattern() {
    let script = "x=$'\\x11'\nif [[ $x == ?? ]]; then echo match; else echo NOMATCH; fi\n";
    let (stdout, stderr, code) = rubash_stdin(script);
    assert_eq!(code, Some(0), "stderr: {stderr}");
    assert_eq!(stdout, "NOMATCH\n", "stderr: {stderr}");
}

#[test]
fn carried_byte_matches_in_case_statement() {
    let script = "x=$'\\x11'\ncase $x in\n?) echo single;;\n*) echo other;;\nesac\n";
    let (stdout, stderr, code) = rubash_stdin(script);
    assert_eq!(code, Some(0), "stderr: {stderr}");
    assert_eq!(stdout, "single\n", "stderr: {stderr}");
}

#[test]
fn carried_byte_matches_star_and_literal_forms() {
    let script = concat!(
        "x=$'\\x11'\n",
        "[[ $x == * ]] && echo star || echo nostar\n",
        "[[ $x == $'\\x11' ]] && echo lit || echo nolit\n",
    );
    let (stdout, stderr, code) = rubash_stdin(script);
    assert_eq!(code, Some(0), "stderr: {stderr}");
    assert_eq!(stdout, "star\nlit\n", "stderr: {stderr}");
}
