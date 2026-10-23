//! Regression guards for the rubash#1225..#1247 batch.
//!
//! Each expectation is byte-verified against WSL GNU Bash 5.3.x with a
//! script-file probe. GNU owners:
//!
//! * #1225 — builtins/declare.def:966-980 / arrayfunc.c: find_or_make_array_variable:
//!   a bare `name=` binds element 0 (empty) when the variable is an array;
//!   it never fabricates contents from the variable NAME.
//! * #1232/#1244 — general.c:1119-1136 bash_special_tilde_expansions +
//!   builtins/pushd.def:596-642 get_dirstack_from_string: `~+`, `~-`,
//!   `~N`, `~+N`, `~-N` expand before getpwnam, and the suffix glues.
//! * #1234 — lib/tilde is unrelated; this is strtrans.c:161-181 u32cconv:
//!   a lone surrogate / out-of-range \U encodes as wctomb's raw 3/4-byte
//!   form, never as the internal U+E000 marker bytes.
//! * #1246 — builtins/set.def:594-604 (invalid SHELLOPTS name),
//!   builtins/evalfile.c:135-139 (`is a directory`), jobs.c:1968
//!   printable_job_status (long format signal description), jobs.c:2803
//!   wait_for_background_pids (`wait: warning: job N[PID] stopped`).
//! * #1247 — jobs.c:4064 waitchld only uses WUNTRACED under job control,
//!   so without `set -m` a SIGSTOPped job still reads as Running.

use std::process::Command;

fn rubash(script: &str) -> (String, String, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .output()
        .expect("run rubash");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

/// #1225: `declare assoc=`/`declare array=` used to fabricate array bodies
/// keyed on the variable NAME; `declare -A x=` dropped its `([0]="")`.
#[test]
fn declare_empty_assignment_is_not_fabricated() {
    let (stdout, _stderr, code) = rubash(
        "declare assoc=\n\
         declare -p assoc\n\
         declare array=\n\
         declare -p array\n\
         declare -A assoc2=\n\
         declare -p assoc2\n",
    );
    assert_eq!(
        stdout,
        "declare -- assoc=\"\"\n\
         declare -- array=\"\"\n\
         declare -A assoc2=([0]=\"\" )\n"
    );
    assert_eq!(code, Some(0));
}

/// #1232: every `~+`/`~-` form takes a suffix and an optional stack index,
/// in command words and in assignment segments.
#[test]
fn tilde_plus_minus_suffix_and_dirstack_forms() {
    let (stdout, stderr, code) = rubash(
        "pushd /tmp >/dev/null\n\
         pushd / >/dev/null\n\
         echo ~+/x\n\
         echo ~+1\n\
         echo ~+9\n\
         x=~/a:~+/b; echo \"x=$x\"\n",
    );
    assert!(stdout.starts_with("//x\n"), "got {stdout:?}");
    assert!(stdout.contains("\n/tmp\n"), "got {stdout:?}");
    assert!(
        stdout.contains("~+9\n"),
        "out-of-range stays literal: {stdout:?}"
    );
    assert!(
        !stdout.contains("~+/b"),
        "assignment segment not expanded: {stdout:?}"
    );
    assert_eq!(code, Some(0), "stderr={stderr:?}");
}

/// #1244: `cd ~+N` reaches the directory-stack entry before cd runs.
#[test]
fn cd_accepts_explicit_dirstack_tilde_index() {
    let (stdout, stderr, code) = rubash(
        "pushd /tmp >/dev/null\n\
         pushd / >/dev/null\n\
         cd ~+1; echo \"rc=$? dir=${PWD##*/}\"\n",
    );
    assert_eq!(stdout, "rc=0 dir=tmp\n", "stderr={stderr:?}");
    assert_eq!(code, Some(0));
}

/// #1234: out-of-range `\U` / lone-surrogate `\u` emit GNU's raw bytes,
/// never the U+E000 transport markers.
#[test]
fn ansi_c_invalid_codepoints_emit_raw_bytes() {
    let path = std::env::temp_dir().join(format!(
        "rubash_issue1234_{}_{}.bin",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let display = path.to_string_lossy().replace('\\', "/");
    let (stdout, stderr, code) = rubash(&format!(
        "printf '%s' $'\\U00110000' > '{display}'\n\
         printf '%s' $'\\uD800' >> '{display}'\n"
    ));
    let bytes = std::fs::read(&path).expect("read probe output");
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        bytes, b"\xf4\x90\x80\x80\xed\xa0\x80",
        "stdout={stdout:?} stderr={stderr:?}"
    );
    assert_eq!(code, Some(0));
}

/// #1246 (1): an unknown inherited SHELLOPTS name is diagnosed with GNU's
/// `name: line 0: X: invalid option name`; BASHOPTS stays silent.
#[test]
fn invalid_inherited_shellopts_name_is_diagnosed() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("echo done")
        .env("SHELLOPTS", "bogusopt123")
        .output()
        .expect("run rubash");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "done\n");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("line 0: bogusopt123: invalid option name"),
        "stderr={stderr:?}"
    );
    assert_eq!(output.status.code(), Some(0));
}

/// #1247: without monitor mode SIGSTOP does not rewrite job state.
#[test]
fn stopped_job_without_monitor_still_lists_as_running() {
    let (stdout, _stderr, code) = rubash(
        "sleep 5 & p=$!\n\
         kill -STOP $p; sleep 0.2\n\
         echo \"-s-\"; jobs -s\n\
         echo \"-r-\"; jobs -r\n\
         kill -CONT $p; kill $p; wait 2>/dev/null\n",
    );
    assert!(
        stdout.contains("-s-\n-r-\n"),
        "jobs -s not empty: {stdout:?}"
    );
    assert!(
        stdout.contains("Running"),
        "jobs -r missing Running: {stdout:?}"
    );
    assert_eq!(code, Some(0));
}

/// #1246 (3)+(4): with monitor mode the long format names the stop signal
/// and a bare wait warns about the stopped job.
#[test]
fn stopped_job_long_format_and_wait_warning() {
    let (stdout, stderr, code) = rubash(
        "set -m\n\
         sleep 5 & p=$!\n\
         kill -STOP $p; sleep 0.2\n\
         jobs -l\n\
         wait; echo \"w=$?\"\n\
         kill -CONT $p; kill $p; wait 2>/dev/null\n",
    );
    assert!(stdout.contains("Stopped (signal)"), "stdout={stdout:?}");
    assert!(stdout.contains("w=0"), "wait blocked: {stdout:?}");
    assert!(
        stderr.contains("wait: warning: job 1["),
        "stderr={stderr:?}"
    );
    assert_eq!(code, Some(0));
}
