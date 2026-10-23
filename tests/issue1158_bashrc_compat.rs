//! Issue rubash#1158 regressions: interactive non-login shells perform the
//! owner-decided "compatible reading" — source `$HOME/.bashrc` (the GNU
//! parity migration path) and then `$HOME/.niubashrc` (niu's own config,
//! running last so it overrides).
//!
//! Before this change the engine sourced NO startup files on Windows:
//! `run_startup_files` was `#[cfg(not(windows))]`-gated (src/main.rs),
//! which is why `niu -i` ignored `~/.bashrc`. The login profile chain is
//! deliberately untouched (rubash#1159 was closed NOT-A-BUG: a login shell
//! sources profiles only, and a .bash_profile that wants .bashrc sources
//! it itself — the user's script's job, not ours).
//!
//! Verified against the WSL GNU Bash 5.3.20 oracle (script-file probes,
//! 2026-10-23):
//!
//! | case                                    | GNU 5.3.20 |
//! | --- | --- |
//! | `bash -i -c` + `~/.bashrc` marker       | marker, then AFTER, rc 0 |
//! | `bash --norc -i -c`                     | AFTER only |
//! | `bash --rcfile f -i -c` (defaults exist)| ONLY f runs |
//! | `bash --rcfile missing -i -c`           | AFTER, silent, rc 0 |
//! | `bash -c` (non-interactive)             | no rc at all |
//! | syntax error in rc                      | diagnostic, AFTER, rc 0 |
//! | `exit 7` in rc                          | shell ends, nothing after |
//!
//! The .niubashrc sibling is a product extension on top of that GNU list
//! (the issue's #1158 decision), not a GNU behavior.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug)]
struct Run {
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

/// A scratch home directory. Unique per test tag + pid so parallel tests
/// never share rc files.
fn home(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rubash-i1158-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("scratch home");
    dir
}

fn write_rc(dir: &Path, name: &str, contents: &str) {
    std::fs::write(dir.join(name), contents).expect("write rc");
}

/// Run rubash with `-i -c 'echo AFTER'` shaped args under a fake HOME.
/// `extra` are inserted before the trailing `-i -c ...` the individual
/// tests choose. Every run strips env that could leak user state in.
fn spawn(args: &[&str], home_dir: Option<&Path>, stdin: &[u8]) -> Run {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rubash"));
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = home_dir {
        // Forward-slash form is the canonical one this repo feeds the
        // engine (mixed separators resolve identically — probed).
        let home = dir.to_string_lossy().replace('\\', "/");
        command.env("HOME", &home);
    } else {
        command.env_remove("HOME");
    }
    // init.rs falls back to these when HOME is unset; pin them so the
    // "HOME unset" case really has no home, and so a host USERPROFILE can
    // never redirect the scratch home.
    if let Some(dir) = home_dir {
        command.env("USERPROFILE", dir);
    } else {
        command.env_remove("USERPROFILE");
    }
    command
        .env_remove("HOMEDRIVE")
        .env_remove("HOMEPATH")
        .env_remove("BASH_ENV")
        .env_remove("ENV")
        .env_remove("NIU_INVOKED_AS")
        .env_remove("BASH_ARGV0")
        .env_remove("__RUBASH_STARTUP_FILES_DONE")
        .env_remove("__RUBASH_SOURCED_RC")
        .stdin(Stdio::piped());
    let mut child = command.spawn().expect("spawn rubash");
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(stdin)
        .expect("write stdin");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait rubash");
    Run {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output.status.code(),
    }
}

fn run_i_c(args: &[&str], home_dir: &Path) -> Run {
    let mut full: Vec<&str> = args.to_vec();
    full.extend(["-i", "-c", "echo AFTER"]);
    spawn(&full, Some(home_dir), b"")
}

// ---------------------------------------------------------------------------
// The compatible-reading contract
// ---------------------------------------------------------------------------

/// GNU parity migration path: `~/.bashrc` alone is sourced by an
/// interactive non-login shell, before the `-c` command.
#[test]
fn bashrc_alone_is_sourced_for_interactive_shell() {
    let dir = home("bashrc-only");
    write_rc(&dir, ".bashrc", "echo BASHRC-MARK\n");
    let run = run_i_c(&[], &dir);
    assert_eq!(run.stdout, "BASHRC-MARK\nAFTER\n", "{run:?}");
    assert_eq!(run.code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Existing niu users regress nothing: `~/.niubashrc` alone is still
/// sourced.
#[test]
fn niubashrc_alone_is_sourced_for_interactive_shell() {
    let dir = home("niurc-only");
    write_rc(&dir, ".niubashrc", "echo NIUBASHRC-MARK\n");
    let run = run_i_c(&[], &dir);
    assert_eq!(run.stdout, "NIUBASHRC-MARK\nAFTER\n", "{run:?}");
    assert_eq!(run.code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Both files: `.bashrc` runs first, `.niubashrc` second — proven by a
/// variable the second file overwrites.
#[test]
fn both_files_run_bashrc_first_niubashrc_last() {
    let dir = home("both");
    write_rc(&dir, ".bashrc", "echo BASHRC-MARK\nMARKER=from-bashrc\n");
    write_rc(
        &dir,
        ".niubashrc",
        "echo NIUBASHRC-MARK\nMARKER=from-niubashrc\n",
    );
    let run = spawn(
        &["-i", "-c", "echo AFTER; echo MARKER=$MARKER"],
        Some(&dir),
        b"",
    );
    assert_eq!(
        run.stdout, "BASHRC-MARK\nNIUBASHRC-MARK\nAFTER\nMARKER=from-niubashrc\n",
        "{run:?}"
    );
    assert_eq!(run.code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Neither file: current no-rc behavior is unchanged.
#[test]
fn missing_both_sources_nothing() {
    let dir = home("none");
    let run = run_i_c(&[], &dir);
    assert_eq!(run.stdout, "AFTER\n", "{run:?}");
    assert_eq!(run.code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `--norc` skips both candidates (GNU shell.c:269-270 no_rc).
#[test]
fn norc_skips_bashrc_and_niubashrc() {
    let dir = home("norc");
    write_rc(&dir, ".bashrc", "echo BASHRC-MARK\n");
    write_rc(&dir, ".niubashrc", "echo NIUBASHRC-MARK\n");
    let run = run_i_c(&["--norc"], &dir);
    assert_eq!(run.stdout, "AFTER\n", "{run:?}");
    assert_eq!(run.code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `--rcfile` replaces the WHOLE personal-rc slot: only the named file
/// runs even when both defaults exist (GNU bashrc_file semantics).
#[test]
fn rcfile_replaces_both_default_candidates() {
    let dir = home("rcfile");
    write_rc(&dir, ".bashrc", "echo BASHRC-MARK\n");
    write_rc(&dir, ".niubashrc", "echo NIUBASHRC-MARK\n");
    write_rc(&dir, "custom.rc", "echo CUSTOM-RC\n");
    let custom_arg = dir.join("custom.rc").to_string_lossy().replace('\\', "/");
    let run = spawn(
        &["--rcfile", &custom_arg, "-i", "-c", "echo AFTER"],
        Some(&dir),
        b"",
    );
    assert_eq!(run.stdout, "CUSTOM-RC\nAFTER\n", "{run:?}");
    assert_eq!(run.code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// HOME unset (and every Windows fallback stripped): no home means no rc —
/// the engine's own contract on a platform with no passwd entry to fall
/// back to. GNU on Unix still finds .bashrc via getpwuid; that oracle has
/// no Windows analog and is documented here rather than faked.
#[test]
fn unset_home_and_fallbacks_sources_nothing() {
    let dir = home("nohome");
    write_rc(&dir, ".bashrc", "echo BASHRC-MARK\n");
    let run = spawn(&["-i", "-c", "echo AFTER"], None, b"");
    assert_eq!(run.stdout, "AFTER\n", "{run:?}");
    assert_eq!(run.code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A non-interactive `-c` reads no rc at all (GNU: $BASH_ENV is the
/// non-interactive channel).
#[test]
fn noninteractive_shell_reads_no_rc() {
    let dir = home("nonint");
    write_rc(&dir, ".bashrc", "echo BASHRC-MARK\n");
    write_rc(&dir, ".niubashrc", "echo NIUBASHRC-MARK\n");
    let run = spawn(&["-c", "echo AFTER"], Some(&dir), b"");
    assert_eq!(run.stdout, "AFTER\n", "{run:?}");
    assert_eq!(run.code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A login shell keeps master's pre-existing behavior: no rc sourcing
/// (the profile chain replaces the rc branch, shell.c:1833 if/else; on
/// Windows the login branch is the deliberate no-op).
#[test]
fn login_shell_reads_no_rc() {
    let dir = home("login");
    write_rc(&dir, ".bashrc", "echo BASHRC-MARK\n");
    write_rc(&dir, ".niubashrc", "echo NIUBASHRC-MARK\n");
    let run = spawn(&["--login", "-i", "-c", "echo AFTER"], Some(&dir), b"");
    assert_eq!(run.stdout, "AFTER\n", "{run:?}");
    assert_eq!(run.code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Error surfacing — GNU prints the diagnostic and CONTINUES (evalstring.c
// parse_and_execute ordinary return), it does not abort startup.
// ---------------------------------------------------------------------------

/// A syntax error in `.bashrc` must not stop startup: the diagnostic
/// prints, `.niubashrc` still runs, and the `-c` command still runs —
/// verified `bash --rcfile bad.rc -i -c 'echo AFTER'` on WSL GNU 5.3.20
/// (error + AFTER, rc 0). The diagnostic's exact shape is issue #1161's
/// area, so this asserts only that a syntax error is reported.
#[test]
fn broken_bashrc_warns_but_niubashrc_and_command_still_run() {
    let dir = home("broken");
    write_rc(&dir, ".bashrc", "if then\n");
    write_rc(&dir, ".niubashrc", "echo NIUBASHRC-MARK\n");
    let run = run_i_c(&[], &dir);
    assert_eq!(run.stdout, "NIUBASHRC-MARK\nAFTER\n", "{run:?}");
    assert!(
        run.stderr.contains("syntax error"),
        "expected a syntax diagnostic, stderr: {:?}",
        run.stderr
    );
    assert_eq!(run.code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Same GNU contract through `--rcfile` (the pre-existing direct-run
/// path): bad rc diagnostics do not end the invocation.
#[test]
fn broken_rcfile_warns_but_command_still_runs() {
    let dir = home("broken-rcfile");
    write_rc(&dir, "bad.rc", "if then\n");
    let bad = dir.join("bad.rc");
    let bad_arg = bad.to_string_lossy().replace('\\', "/");
    let run = spawn(
        &["--rcfile", &bad_arg, "-i", "-c", "echo AFTER"],
        Some(&dir),
        b"",
    );
    assert_eq!(run.stdout, "AFTER\n", "{run:?}");
    assert!(
        run.stderr.contains("syntax error"),
        "expected a syntax diagnostic, stderr: {:?}",
        run.stderr
    );
    assert_eq!(run.code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The interactive reader path (`-i` with piped stdin) runs the same
/// chain, ahead of the first command line.
#[test]
fn interactive_stdin_reader_runs_the_chain_before_input() {
    let dir = home("stdin-reader");
    write_rc(&dir, ".bashrc", "echo BASHRC-MARK\n");
    write_rc(&dir, ".niubashrc", "echo NIUBASHRC-MARK\n");
    let run = spawn(&["-i"], Some(&dir), b"echo STDIN-LINE\nexit\n");
    assert!(
        run.stdout.contains("BASHRC-MARK\nNIUBASHRC-MARK\n"),
        "{run:?}"
    );
    assert!(run.stdout.contains("STDIN-LINE"), "{run:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
