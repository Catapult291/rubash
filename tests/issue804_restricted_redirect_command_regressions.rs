//! #804 regressions: the restricted shell refuses redirection DIRECTIONS,
//! not slashes in redirect targets, and `command`'s operand passes the
//! same '/' scan a disk command does.
//!
//! GNU oracle: bash 5.3.0 restricted face, WSL 2026-10-13 — probed with
//! `/tmp/rbash -> bash-5.3` argv0 and `bash -r` (both faces agree cell for
//! cell). Anchors: vendored command.h:57 `WRITE_REDIRECT(ri)` over
//! redir.c:919 (`if (restricted && (WRITE_REDIRECT (ri)))
//! return RESTRICTED_REDIRECT;`), the diagnostic arm redir.c:210, and the
//! command-name scan execute_cmd.c:5789 that `command`'s operand lands in
//! (command.def builds a bare simple command from the operand list and
//! execute_command dispatches it; -p is refused earlier at
//! command.def:97-103, -v/-V return from the describe path before any
//! execution). The pre-fix rubash inverted cells 11/12 (allowed every
//! write form whose target had no slash, refused `cat < slashed-file` with
//! the output diagnostic) and skipped `command` operands entirely.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// Distinct sandbox dir per test so the relative-target cases never share
/// state.
fn sandbox(tag: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("rubash-804-{tag}-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&dir).expect("create sandbox");
    dir
}

#[derive(Debug)]
struct Output {
    status: i32,
    stdout: String,
    stderr: String,
}

fn run_face(engine: &Path, cwd: &Path, script: &str) -> Output {
    let mut child = Command::new(engine)
        .arg("-c")
        .arg(script)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn engine");
    child
        .stdin
        .take()
        .map(|mut stdin| stdin.write_all(b"").ok());
    let output = child.wait_with_output().expect("wait");
    Output {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr)
            .to_string()
            .replace('\r', ""),
    }
}

/// The `-r` flag face.
fn run_r(cwd: &Path, script: &str) -> Output {
    let mut cmd = Command::new(shell_bin());
    cmd.arg("-r").arg("-c").arg(script).current_dir(cwd);
    run_command(cmd)
}

/// The rbash argv0 face: the engine copied under the `rbash` name.
fn run_rbash_face(cwd: &Path, script: &str) -> Output {
    let dir = sandbox("face");
    let face = dir.join(if cfg!(windows) { "rbash.exe" } else { "rbash" });
    fs::copy(shell_bin(), &face).expect("copy engine under the rbash face");
    let mut cmd = Command::new(face);
    cmd.arg("-c").arg(script).current_dir(cwd);
    run_command(cmd)
}

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

fn run_command(mut cmd: Command) -> Output {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd
        .spawn()
        .expect("spawn")
        .wait_with_output()
        .expect("wait");
    Output {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr)
            .to_string()
            .replace('\r', ""),
    }
}

fn drop_sandbox(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

/// Gap 11: every WRITE_REDIRECT direction is refused on ANY target —
/// absolute, relative, or /dev/null — with
/// `<target>: restricted: cannot redirect output` and rc 1, and the
/// command never runs (GNU: `echo hi > /tmp/rb_out1` -> rc 1,
/// `/tmp/rb_out1: restricted: cannot redirect output`; `echo hi >
/// rb_rel.txt` -> the relative spelling in the message).
#[test]
fn write_directions_are_refused_regardless_of_target() {
    let dir = sandbox("gap11");
    let script = r#"
echo hi > rb804-out.txt
echo hi >> rb804-out.txt
echo hi <> rb804-out.txt
echo hi >| rb804-out.txt
echo hi &> rb804-out.txt
echo hi &>> rb804-out.txt
echo hi 2> rb804-out.txt
echo hi 2>> rb804-out.txt
{ echo hi; } > rb804-out.txt
exec > rb804-out.txt
exec 3> rb804-out.txt
echo hi > /dev/null
echo hi <> /dev/null
echo CONTINUED
"#;
    let out = run_r(&dir, script);
    // GNU keeps the script reading after a refused redirect (the failing
    // command's rc is 1; only that command is skipped).
    assert_eq!(out.stdout, "CONTINUED\n", "{out:?}");
    // Every refusal names its target like GNU; count the diagnostics
    // (13 write forms, `exec 3>` included in the list above).
    let refusals = out
        .stderr
        .lines()
        .filter(|line| line.contains("restricted: cannot redirect output"))
        .count();
    assert_eq!(refusals, 13, "stderr: {:?}", out.stderr);
    assert!(
        out.stderr
            .contains("rb804-out.txt: restricted: cannot redirect output"),
        "relative target spelling in the message: {:?}",
        out.stderr
    );
    assert!(
        out.stderr
            .contains("/dev/null: restricted: cannot redirect output"),
        "/dev/null is refused too: {:?}",
        out.stderr
    );
    drop_sandbox(&dir);
}

/// Gap 12: read-only input redirection is NOT restricted — GNU runs
/// `cat < /etc/hostname` and `exec 3< file` happily; the old scan refused
/// any slashed target with the output diagnostic.
#[test]
fn read_only_input_redirection_stays_allowed() {
    let dir = sandbox("gap12");
    let seed = dir.join("rb804-in.txt");
    fs::write(&seed, "rb804-payload\n").expect("seed input file");
    let seed_display = seed.display().to_string().replace('\\', "/");
    let script = format!(
        "read line < '{seed}'; echo \"read=$line\"\ncat < '{seed}'\nexec 3< '{seed}'; read -u 3 line3; echo \"fd3=$line3\"\necho prelude <&0\n",
        seed = seed_display
    );
    let out = run_r(&dir, &script);
    assert!(
        !out.stderr.contains("restricted"),
        "input redirection must not hit the restricted gate: {:?}",
        out.stderr
    );
    assert_eq!(out.status, 0, "stderr: {:?}", out.stderr);
    assert!(
        out.stdout.contains("read=rb804-payload"),
        "{:?}",
        out.stdout
    );
    assert!(out.stdout.contains("fd3=rb804-payload"), "{:?}", out.stdout);
    drop_sandbox(&dir);
}

/// Gap 12 diagnostic routing: a read-only open failure keeps the OS
/// diagnostic class (no "restricted" wording), matching GNU's plain
/// `No such file or directory`.
#[test]
fn input_open_failure_reports_the_os_error_not_restriction() {
    let dir = sandbox("gap12b");
    let out = run_r(&dir, "cat < ./rb804-no-such-input-file\n");
    assert!(
        !out.stderr.contains("restricted"),
        "stderr: {:?}",
        out.stderr
    );
    assert!(
        out.stderr.contains("No such file") || out.stderr.contains("no such file"),
        "stderr: {:?}",
        out.stderr
    );
    assert_eq!(out.status, 1);
    drop_sandbox(&dir);
}

/// Gap 13: `command /bin/echo` is refused with the disk-command
/// diagnostic on the OPERAND (`/bin/echo: restricted: cannot specify
/// `/' in command names`, rc 1). The describe forms keep working
/// (`command -v /bin/echo` describes), `--` ends the options, and
/// `command echo -p` treats `-p` as echo's argument exactly like GNU
/// (prints `-p`, rc 0).
#[test]
fn command_builtin_operand_goes_through_the_slash_scan() {
    let dir = sandbox("gap13");
    let out = run_r(&dir, "command /bin/echo hi\n");
    assert_eq!(out.status, 1);
    assert!(
        out.stderr
            .contains("/bin/echo: restricted: cannot specify `/' in command names"),
        "stderr: {:?}",
        out.stderr
    );
    let describe = run_r(&dir, "command -v /bin/echo\n");
    assert_eq!(describe.status, 0, "stderr: {:?}", describe.stderr);
    assert_eq!(describe.stdout.trim(), "/bin/echo");
    let dashdash = run_r(&dir, "command -- /bin/echo hi\n");
    assert_eq!(dashdash.status, 1, "stderr: {:?}", dashdash.stderr);
    assert!(
        dashdash
            .stderr
            .contains("/bin/echo: restricted: cannot specify `/' in command names"),
        "stderr: {:?}",
        dashdash.stderr
    );
    let echo_p = run_r(&dir, "command echo -p\n");
    assert_eq!(echo_p.status, 0, "stderr: {:?}", echo_p.stderr);
    assert_eq!(echo_p.stdout, "-p\n", "{:?}", echo_p.stdout);
    let dash_p = run_r(&dir, "command -p /bin/echo hi\n");
    assert_eq!(dash_p.status, 1);
    assert!(
        dash_p.stderr.contains("command: -p: restricted"),
        "stderr: {:?}",
        dash_p.stderr
    );
    drop_sandbox(&dir);
}

/// Gap 11 order routing: the restriction fires at the redirect's own
/// position in the list — a read-only input BEFORE the write still ran its
/// expansion (and a write BEFORE a failing input reports the restriction,
/// not the open error), matching GNU's do_redirections order.
#[test]
fn restriction_fires_in_redirect_list_order() {
    let dir = sandbox("order");
    let seed = dir.join("rb804-order.txt");
    fs::write(&seed, "x\n").expect("seed");
    // write first: restricted diagnostic, not the input open error.
    let out = run_r(&dir, "echo hi > rb804-out.txt < ./rb804-no-such-file\n");
    assert!(
        out.stderr.contains("restricted: cannot redirect output"),
        "stderr: {:?}",
        out.stderr
    );
    // input first: the open error wins its own position, no restriction
    // anywhere (nothing write-shaped before it).
    let out = run_r(&dir, "cat < ./rb804-no-such-file\n");
    assert!(
        !out.stderr.contains("restricted"),
        "stderr: {:?}",
        out.stderr
    );
    drop_sandbox(&dir);
}

/// Both entry faces agree: the rbash argv0 face refuses and allows the
/// same cells as `-r` (GNU probe: `/tmp/rbin/rbash -c` equals `bash -r -c`).
#[test]
fn rbash_argv0_face_matches_the_r_flag() {
    let dir = sandbox("faces");
    for script in [
        "echo hi > rb804-face.txt",
        "echo hi <> rb804-face.txt",
        "command /bin/echo hi",
        "echo prelude | cat < /dev/null",
    ] {
        let via_flag = run_r(&dir, script);
        let via_face = run_rbash_face(&dir, script);
        assert_eq!(
            via_flag.status, via_face.status,
            "rc divergence on {script}: {:?} vs {:?}",
            via_flag.stderr, via_face.stderr
        );
        assert_eq!(via_flag.stdout, via_face.stdout, "script: {script}");
        let flag_refused = via_flag.stderr.contains("restricted");
        let face_refused = via_face.stderr.contains("restricted");
        assert_eq!(flag_refused, face_refused, "script: {script}");
    }
    drop_sandbox(&dir);
}

/// The `set +r` lock is untouched: a restricted shell keeps refusing the
/// unlock attempt with the usage diagnostic and the next write redirect is
/// still refused (GNU: `set +r` -> "set: +r: invalid option" + usage, and
/// the following redirect still dies).
#[test]
fn set_plus_r_stays_locked() {
    let dir = sandbox("lock");
    let out = run_r(&dir, "set +r\necho hi > rb804-locked.txt\n");
    assert!(
        out.stderr.contains("set: +r: invalid option"),
        "stderr: {:?}",
        out.stderr
    );
    assert!(
        out.stderr.contains("restricted: cannot redirect output"),
        "still restricted after the refused unlock: {:?}",
        out.stderr
    );
    drop_sandbox(&dir);
}

/// Pipeline members take the same refusals (GNU runs every member through
/// execute_simple_command -> execute_disk_command / do_redirections), and
/// the fd-var spelling reports under the VARIABLE name like the C code's
/// REDIR_VARASSIGN arm (redir.c:137-138).
#[test]
fn pipeline_member_and_fd_var_spelling_match_gnu() {
    let dir = sandbox("member");
    let member = run_r(&dir, "echo piped | cat > rb804-member.txt\n");
    assert_eq!(member.status, 1);
    assert!(
        member
            .stderr
            .contains("rb804-member.txt: restricted: cannot redirect output"),
        "stderr: {:?}",
        member.stderr
    );
    let fd_var = run_r(&dir, "echo hi {v}> rb804-var.txt\n");
    assert_eq!(fd_var.status, 1);
    assert!(
        fd_var
            .stderr
            .contains("v: restricted: cannot redirect output"),
        "variable name in the message: {:?}",
        fd_var.stderr
    );
    let dup_word = run_r(&dir, "echo hi 2>& rb804-dup.txt\n");
    assert_eq!(dup_word.status, 1);
    assert!(
        !dup_word.stderr.contains("restricted"),
        "a redirector-2 dup word is ambiguous, not restricted: {:?}",
        dup_word.stderr
    );
    assert!(
        dup_word
            .stderr
            .contains("rb804-dup.txt: ambiguous redirect"),
        "stderr: {:?}",
        dup_word.stderr
    );
    let comsub = run_r(&dir, "echo hi > $(echo rb804-comsub.txt)\n");
    assert_eq!(comsub.status, 1);
    assert!(
        comsub
            .stderr
            .contains("$(echo rb804-comsub.txt): restricted: cannot redirect output"),
        "literal substitution spelling in the message: {:?}",
        comsub.stderr
    );
    drop_sandbox(&dir);
}
