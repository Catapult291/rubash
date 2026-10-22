//! issue#1038 regression: `shopt -s gnu_errfmt` must switch the
//! error_prolog/parser_error families from `file: line N:` to `file:N:`.
//!
//! GNU ownership: error.c:71-86 error_prolog
//! (`gnu_error_format ? "" : _(" line ")`) and error.c:300-316
//! parser_error gate the ` line N` segment; builtins/common.c:83-110
//! builtin_error_prolog never gates. So under gnu_errfmt a missing
//! command, an arithmetic evalerror and a redirection failure report
//! `d1.sh:2: ...` while a builtin failure (`cd`) keeps
//! `d1.sh: line 2: cd: ...`. The compgen pair splits the same way: the
//! `-F` warning (builtin_error) keeps `line N:`, the missing-function
//! lookup (pcomplete.c:1058-1066 internal_error) gates to `:N:`.
//!
//! Probes below are one script file each (oracle: GNU bash 5.3.20).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn shell_bin() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_rubash") {
        return PathBuf::from(path);
    }
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(profile)
        .join(if cfg!(windows) {
            "rubash.exe"
        } else {
            "rubash"
        })
}

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rubash-1038-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn run_script(path: &Path) -> (i32, String, String) {
    let output = Command::new(shell_bin())
        .arg(path)
        .stdin(Stdio::null())
        .output()
        .expect("run rubash");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).replace('\r', ""),
        String::from_utf8_lossy(&output.stderr).replace('\r', ""),
    )
}

fn script_with(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).expect("write script");
    path
}

#[test]
fn command_not_found_gates_line_segment() {
    let dir = scratch_dir("notfound");
    let script = script_with(&dir, "d1.sh", "shopt -s gnu_errfmt\nnonexistent_cmd_xyz\n");
    let (status, _, stderr) = run_script(&script);
    assert_eq!(status, 127);
    let name = rubash::executor::shell_pwd_display_path(&script.to_string_lossy());
    assert_eq!(
        stderr,
        format!("{name}:2: nonexistent_cmd_xyz: command not found\n")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn arithmetic_division_by_zero_gates_line_segment() {
    let dir = scratch_dir("arith");
    let script = script_with(&dir, "d2.sh", "shopt -s gnu_errfmt\necho $((1/0))\n");
    let (status, _, stderr) = run_script(&script);
    assert_eq!(status, 1);
    let name = rubash::executor::shell_pwd_display_path(&script.to_string_lossy());
    assert_eq!(
        stderr,
        format!("{name}:2: 1/0: division by 0 (error token is \"0\")\n")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn redirect_open_failure_gates_line_segment() {
    let dir = scratch_dir("redir");
    let script = script_with(
        &dir,
        "d3.sh",
        "shopt -s gnu_errfmt\necho hi > /nonexistent_dir_xyz/f\n",
    );
    let (status, _, stderr) = run_script(&script);
    assert_eq!(status, 1);
    let name = rubash::executor::shell_pwd_display_path(&script.to_string_lossy());
    assert_eq!(
        stderr,
        format!("{name}:2: /nonexistent_dir_xyz/f: No such file or directory\n")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn builtin_errors_keep_line_segment() {
    let dir = scratch_dir("builtin");
    let script = script_with(
        &dir,
        "d4.sh",
        "shopt -s gnu_errfmt\ncd /nonexistent_dir_xyz\n",
    );
    let (status, _, stderr) = run_script(&script);
    assert_eq!(status, 1);
    let name = rubash::executor::shell_pwd_display_path(&script.to_string_lossy());
    assert_eq!(
        stderr,
        format!("{name}: line 2: cd: /nonexistent_dir_xyz: No such file or directory\n")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compgen_pair_splits_warning_and_lookup() {
    let dir = scratch_dir("compgen");
    let script = script_with(
        &dir,
        "g2.sh",
        "shopt -s gnu_errfmt\ncompgen -F _nosuchfunc_xyz -- \"\"\n",
    );
    let (status, _, stderr) = run_script(&script);
    assert_eq!(status, 1);
    let name = rubash::executor::shell_pwd_display_path(&script.to_string_lossy());
    assert_eq!(
        stderr,
        format!(
            "{name}: line 2: compgen: warning: -F option may not work as you expect\n\
             {name}:2: completion: function `_nosuchfunc_xyz' not found\n"
        )
    );
    let _ = std::fs::remove_dir_all(&dir);
}
