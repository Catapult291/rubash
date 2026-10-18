//! rubash#799: external command-name resolution is case-sensitive on the
//! command NAME (the stem), with only the appended PATHEXT-style extension
//! comparing ASCII-case-insensitively.
//!
//! Oracle evidence (2026-10-13):
//! * semantic gold — WSL GNU bash 5.3.0, PATH dir holding `R.exe`, `R`, `r`,
//!   `r.sh`, `R.cmd`: `command -v r` finds only the literally-spelled `r`;
//!   with ONLY `R.exe` on PATH, `r` and `R` are both "command not found"
//!   (Linux has no extension candidates at all: rc 127).
//! * form oracle — msys bash 5.2.37 (Git for Windows) and 5.3.0: a bare name
//!   resolves extension candidates (`R` -> `R.exe`/`R.bat`, extensionless
//!   file verbatim), but the name match itself is case-FOLDED by the NTFS
//!   stat layer (`command -v r` reports an on-disk `R.exe` as `r.exe`) —
//!   the artifact #799 forbids adopting: it launched GNU R's interpreter
//!   for a lowercase gitconfig alias and swallowed the session.
//!
//! Contract: stem byte-exact (never `r` -> `R.exe`), extension ASCII-ci
//! (`R` still runs an on-disk `R.EXE`), resolved spelling always the on-disk
//! one.

#![cfg(windows)]

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rubash-799-{tag}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Run `script` with PATH = fixture dir + the system dirs, and return
/// (stdout, rc). System dirs keep the shell's own startup needs (`uname`,
/// consoles) working without carrying a toolchain that owns `r`.
fn run_with_fixture(fixture: &Path, script: &str) -> (String, i32) {
    let script_path = scratch("run").join("probe.sh");
    fs::write(&script_path, script).expect("write probe script");
    let path_var = format!(
        "{};C:\\Windows\\System32;C:\\Windows",
        fixture.to_string_lossy().replace('/', "\\")
    );
    let out = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(&script_path)
        .env("PATH", &path_var)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run rubash script");
    let _ = fs::remove_dir_all(script_path.parent().unwrap());
    let mut stdout = String::new();
    let _ = out.stdout.as_slice().read_to_string(&mut stdout);
    (stdout, out.status.code().unwrap_or(-1))
}

fn write_stub(dir: &Path, name: &str) {
    fs::write(dir.join(name), b"stub").unwrap();
}

/// The reported #799 shape: fixture holds ONLY `R.exe`; a lowercase `r`
/// (gitconfig alias line `r = reflog …`) must stay "command not found"
/// (rc 127 on execution) and `command -v r` must stay empty, while `R`
/// resolves to the on-disk spelling.
#[test]
fn lowercase_r_never_lifts_uppercase_r_exe() {
    let dir = scratch("upper-only");
    write_stub(&dir, "R.exe");
    let (out, _rc) = run_with_fixture(&dir, "command -v r; echo \"vrc=$?\"\n");
    assert!(
        out.contains("vrc=1"),
        "`command -v r` must miss with only R.exe on PATH; got: {out}"
    );
    assert!(
        !out.contains("R.exe") && !out.contains("r.exe"),
        "#799: lowercase `r` resolved an uppercase on-disk R.exe; got: {out}"
    );

    let (out, rc) = run_with_fixture(&dir, "r >/dev/null 2>&1; echo \"rc=$?\"\n");
    assert!(
        out.contains("rc=127"),
        "executing `r` must be 127 command-not-found, got rc line: {out} (status {rc})"
    );

    let (out, _rc) = run_with_fixture(&dir, "c=$(command -v R); echo \"found=${c##*/}\"\n");
    assert!(
        out.contains("found=R.exe"),
        "`R` resolves the on-disk spelling; got: {out}"
    );
}

/// The fold arm never relaxes the STEM: `r3` vs on-disk `R3.EXE` stays a
/// miss, `R3` resolves and reports the on-disk spelling byte-exactly.
#[test]
fn fold_arm_keeps_stem_byte_exact() {
    let dir = scratch("upper-ext");
    write_stub(&dir, "R3.EXE");
    let (out, _rc) = run_with_fixture(
        &dir,
        "a=$(command -v r3); echo \"lower=${a##*/}\"\nb=$(command -v R3); echo \"upper=${b##*/}\"\n",
    );
    assert!(
        out.contains("lower=") && !out.contains("lower=R3"),
        "`r3` must not lift `R3.EXE`; got: {out}"
    );
    assert!(
        out.contains("upper=R3.EXE"),
        "`R3` resolves `R3.EXE` with the on-disk spelling; got: {out}"
    );
}

/// Extensionless names have no extension to fold: the whole name stays
/// byte-exact (`weird` hits, `WEIRD` misses).
#[test]
fn extensionless_name_stays_byte_exact() {
    let dir = scratch("noext");
    write_stub(&dir, "weird");
    let (out, _rc) = run_with_fixture(
        &dir,
        "command -v weird >/dev/null; echo \"low=$?\"\ncommand -v WEIRD >/dev/null; echo \"up=$?\"\n",
    );
    assert!(out.contains("low=0"), "verbatim lowercase hits; got: {out}");
    assert!(
        out.contains("up=1"),
        "uppercase query must miss; got: {out}"
    );
}

/// A lowercase on-disk name is found by its own case and stays invisible to
/// the uppercase query (the mirror of the #799 direction).
#[test]
fn lowercase_exe_resists_uppercase_query() {
    let dir = scratch("lower-exe");
    write_stub(&dir, "r2.exe");
    let (out, _rc) = run_with_fixture(
        &dir,
        "a=$(command -v r2); echo \"low=${a##*/}\"\ncommand -v R2 >/dev/null; echo \"up=$?\"\n",
    );
    assert!(out.contains("low=r2.exe"), "own case hits; got: {out}");
    assert!(
        out.contains("up=1"),
        "uppercase query must miss; got: {out}"
    );
}

/// The exact #799 asset shape: sourcing a pure gitconfig-alias file (one
/// line is the lowercase `r` alias) must spray `command not found` and
/// RETURN — the harness's next probe line must reach the shell, never a
/// foreign REPL.
#[test]
fn sourcing_gitconfig_aliases_reports_and_returns() {
    let dir = scratch("asset");
    // A real PE so a pre-fix resolution genuinely launches something.
    let real = dir.join("R.exe");
    fs::copy("C:\\Windows\\System32\\where.exe", &real).expect("seed R.exe PE");
    let script = "source './aliases.gitconfig'\ncommand echo PROBE_$((42))_OK\n";
    let script_path = scratch("asset-run").join("probe.sh");
    let alias_path = script_path.parent().unwrap().join("aliases.gitconfig");
    fs::write(&script_path, script).expect("write probe script");
    fs::write(
        &alias_path,
        "[alias]\n\ta = add\n\tr = reflog --date=iso8601 --pretty=reflog-default\n\tst = status\n",
    )
    .expect("write gitconfig");
    let path_var = format!(
        "{};C:\\Windows\\System32;C:\\Windows",
        dir.to_string_lossy().replace('/', "\\")
    );
    let out = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(&script_path)
        .env("PATH", &path_var)
        .current_dir(script_path.parent().unwrap())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run rubash script");
    let mut stdout = String::new();
    let mut stderr = String::new();
    let _ = out.stdout.as_slice().read_to_string(&mut stdout);
    let _ = out.stderr.as_slice().read_to_string(&mut stderr);
    let _ = fs::remove_dir_all(script_path.parent().unwrap());
    assert!(
        stdout.contains("PROBE_42_OK"),
        "source must return so the next probe runs; stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        stderr.contains("r: command not found"),
        "lowercase `r` alias must report command not found; stderr={stderr:?}"
    );
}
