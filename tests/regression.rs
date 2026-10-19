//! GNU-diff regression suite (wt4/ci lane).
//!
//! Two kinds of byte-pinned regressions live under `tests/regression/`:
//!
//! 1. **GNU goldens** (`golden/<name>.gnu.out` + `.gnu.err`): fixture
//!    scripts under `fixtures/` whose expected stdout/stderr were captured
//!    ONCE from WSL GNU Bash 5.3.0 (`/usr/local/bin/bash`, the repository
//!    baseline). Each fixture header documents the exact capture command.
//!    CI runners have no WSL, so the recorded bytes ARE the baseline.
//!
//! 2. **rubash snapshots** (`snapshots/<suite>.snap`): stdout of GNU
//!    upstream suite slices (`third_party/bash/tests/<suite>.tests`) run
//!    under TODAY's rubash. These are NOT GNU output — they pin the
//!    current engine so that any future semantic change that shifts a
//!    slice fails CI and forces a conscious re-record. They are
//!    Windows-only (the prebuilt helper binaries under
//!    tests/gnu-compat/helpers-win are PE executables) and run in the
//!    SAME airtight environment as the goldens (see `run_suite_snapshot`):
//!    a snapshot byte that depends on an ambient variable is a
//!    machine-dependent byte, and machine-dependent bytes are how a
//!    green-local/red-CI (or green-CI/red-local) baseline happens.
//!
//!    RE-RECORD PROTOCOL (the anti-baseline-red rule): re-record only
//!    with
//!
//!    ```text
//!    RUBASH_REGRESSION_RECORD=1 cargo test --test regression upstream_suite_slice_snapshots
//!    ```
//!
//!    and the recording commit message MUST name the engine change that
//!    shifted the bytes (issue or PR reference, per slice). A snapshot
//!    diff IS a semantic change; "update golden" with no behavior
//!    reference is exactly how this job burned down in 2026-10 (the
//!    `${var@P}` marker-strip fix in #431/#480 shifted exp without a
//!    re-record and every subsequent PR inherited the red). On CI the
//!    "Record upstream-suite snapshots" workflow step records with the
//!    same code and uploads the bytes as the `regression-snapshots`
//!    artifact — that artifact is the runner-true replacement for stale
//!    committed snapshots; prefer it over transcribing the truncated
//!    40-line panic window by hand.
//!
//! Every fixture runs in its own empty temp directory with a minimal PATH
//! (rubash emulates cat/mkdir/grep/sed when they are absent), a bounded
//! per-test timeout, and an AIRTIGHT environment: `env_clear` plus a
//! machine-infrastructure whitelist (see `run_rubash_in`), so no ambient
//! user/tool variable — whose name may collide with a fixture variable —
//! can influence a golden.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const FIXTURE_DIR: &str = "tests/regression/fixtures";
const GOLDEN_DIR: &str = "tests/regression/golden";
#[cfg(windows)]
const SNAPSHOT_DIR: &str = "tests/regression/snapshots";
const CORPUS_DIR: &str = "tests/regression/corpus";
#[cfg(windows)]
const HELPERS_WIN: &str = "tests/gnu-compat/helpers-win";
#[cfg(windows)]
const UPSTREAM_TESTS: &str = "third_party/bash/tests";

/// Fixture table entry generation: one #[test] per fixture so CI reports
/// each matrix with its own name and the per-fixture bound applies
/// independently. Timeouts are deliberately generous: CI machines vary
/// and the point of this suite is byte parity, not speed (perf has its
/// own canaries below).
fn run_golden_fixture(name: &str, timeout_secs: u64, corpus: &[&str]) {
    let root = manifest_dir();
    std::env::set_current_dir(&root).expect("cwd to manifest dir");
    // The trap matrix is timing-sensitive (the parent must reach its
    // `kill' after the child installed the trap but before its sleep
    // ends); under parallel test load that window can close. Serialize
    // it against the CPU-heavy perf canaries so neither starves the
    // other (observed once as a 3-test flake during codification).
    let _heavy_guard = if name == "trap-signal-matrix" {
        Some(HEAVY_TEST_LOCK.lock().unwrap())
    } else {
        None
    };
    let dir = prepare_scratch(name, corpus);
    let out = run_rubash_in(
        &dir,
        &[&format!("{name}.sh")],
        Duration::from_secs(timeout_secs),
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!out.timed_out, "{name}: TIMED OUT after {timeout_secs}s");
    let expected_out = std::fs::read(root.join(GOLDEN_DIR).join(format!("{name}.gnu.out")))
        .unwrap_or_else(|e| panic!("{name}: missing golden stdout: {e}"));
    let expected_err = std::fs::read(root.join(GOLDEN_DIR).join(format!("{name}.gnu.err")))
        .unwrap_or_else(|e| panic!("{name}: missing golden stderr: {e}"));
    assert_eq!(
        out.code,
        Some(0),
        "{name}: unexpected exit code {:?}",
        out.code
    );
    assert_bytes_eq(&out.stdout, &expected_out, "stdout", name);
    assert_bytes_eq(&out.stderr, &expected_err, "stderr", name);
}

macro_rules! golden_fixture {
    ($fn_name:ident => $name:literal, timeout $secs:literal, corpus [$($rel:literal),*]) => {
        #[test]
        fn $fn_name() {
            run_golden_fixture($name, $secs, &[$($rel),*]);
        }
    };
}

golden_fixture!(golden_andor_continuation => "andor-continuation", timeout 60, corpus []);
golden_fixture!(golden_at_star_matrix => "at-star-matrix", timeout 60, corpus []);
golden_fixture!(golden_catfile_comsub => "catfile-comsub", timeout 60, corpus []);
golden_fixture!(golden_digit_intfit => "digit-intfit", timeout 60, corpus []);
golden_fixture!(golden_eco_completion_register => "eco-completion-register", timeout 60, corpus []);
golden_fixture!(golden_eco_git_completion_smoke => "eco-git-completion-smoke", timeout 60, corpus []);
golden_fixture!(golden_eco_omb_theme_ps1 => "eco-omb-theme-ps1", timeout 60, corpus []);
golden_fixture!(golden_extglob_parse_gate => "extglob-parse-gate", timeout 60, corpus []);
golden_fixture!(golden_fnbody_strict_battery => "fnbody-strict-battery", timeout 90, corpus []);
golden_fixture!(golden_funsub_valsub => "funsub-valsub", timeout 60, corpus []);
golden_fixture!(golden_issue296_carrier_adjacent_comsub => "issue296-carrier-adjacent-comsub", timeout 60, corpus []);
golden_fixture!(golden_nounset_exit_matrix => "nounset-exit-matrix", timeout 60, corpus []);
golden_fixture!(golden_nvm_load => "nvm-load", timeout 180, corpus ["nvm/nvm.sh"]);
golden_fixture!(golden_redir_operand_battery => "redir-operand-battery", timeout 60, corpus []);
golden_fixture!(golden_rustup_check_help => "rustup-check-help", timeout 60, corpus []);
golden_fixture!(golden_trap_signal_matrix => "trap-signal-matrix", timeout 180, corpus []);
golden_fixture!(golden_xtrace_decl_compound => "xtrace-decl-compound", timeout 60, corpus []);

/// Upstream suite slices pinned as rubash snapshots (coordinator list,
/// 2026-09-27): the slices the lanes rely on for parity ledgers.
#[cfg(windows)]
const SNAPSHOT_SUITES: &[&str] = &[
    "arith-for",
    "braces",
    "case",
    "comsub",
    "comsub2",
    "dbg-support",
    "errors",
    "exp",
    "heredoc",
    "nquote",
    "quote",
    "redir",
    "rhs-exp",
];

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn minimal_path() -> String {
    if cfg!(windows) {
        // System32 only: rubash emulates the coreutils the fixtures use.
        // This matches what a bare windows-latest CI runner provides, so
        // the local run and the CI run exercise the same code paths.
        r"C:\Windows\System32".to_string()
    } else {
        "/usr/bin:/bin".to_string()
    }
}

struct RunOutcome {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    code: Option<i32>,
    timed_out: bool,
}

fn wait_bounded(child: &mut Child, limit: Duration) -> RunOutcome {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut pipe) = child.stdout.take() {
                    let _ = pipe.read_to_end(&mut stdout);
                }
                if let Some(mut pipe) = child.stderr.take() {
                    let _ = pipe.read_to_end(&mut stderr);
                }
                return RunOutcome {
                    stdout,
                    stderr,
                    code: status.code(),
                    timed_out: false,
                };
            }
            Ok(None) => {
                if start.elapsed() > limit {
                    let _ = child.kill();
                    let _ = child.wait();
                    return RunOutcome {
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                        code: None,
                        timed_out: true,
                    };
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return RunOutcome {
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    code: None,
                    timed_out: false,
                };
            }
        }
    }
}

fn prepare_scratch(name: &str, corpus: &[&str]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rubash-regression-{}-{}-{}",
        name,
        std::process::id(),
        std::thread::current()
            .name()
            .unwrap_or("t")
            .replace('/', "-")
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    std::fs::copy(
        Path::new(FIXTURE_DIR).join(format!("{name}.sh")),
        dir.join(format!("{name}.sh")),
    )
    .expect("copy fixture");
    for rel in corpus {
        let src = Path::new(CORPUS_DIR).join(rel);
        let dst = dir.join(Path::new(rel).file_name().unwrap());
        std::fs::copy(src, dst).expect("copy corpus file");
    }
    dir
}

fn run_rubash_in(dir: &Path, args: &[&str], limit: Duration) -> RunOutcome {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rubash"));
    cmd.args(args)
        .current_dir(dir)
        // Airtight environment (cifix lane 2026-09-27): a golden must not
        // depend on ambient variables. golden_nounset_exit_matrix passed
        // locally but failed on CI because a runner-side variable whose
        // name case-insensitively matched the fixture's `$m2` reached the
        // child: on Windows `std::env::var` is case-insensitive, so the
        // engine's bare-$NAME env fallback treated `$m2` as bound while
        // GNU (variables.c, exact-case lookup) — and therefore the WSL
        // golden — has it unbound. Drop every user/tool variable and pass
        // only machine infrastructure plus the minimal PATH, so a local
        // run, a CI runner and any future image observe the same fixture
        // world. (Suite snapshots deliberately keep the ambient env:
        // their bytes were recorded with it.)
        .env_clear()
        .env("PATH", minimal_path())
        // Real-world fixtures consult $HOME (nvm's NVM_DIR defaults under
        // it; trap subshells cd there; ruby-build refs it). A fixed
        // synthetic HOME = the fixture's own scratch dir keeps the env
        // airtight while giving every run — local or CI — the same value.
        .env("HOME", dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if cfg!(windows) {
        // Windows-process infrastructure only: values never enter golden
        // bytes (no fixture prints them), and the child needs SystemRoot
        // for OS services and TEMP for any internal temp-file use.
        for name in [
            "SystemRoot",
            "SystemDrive",
            "WINDIR",
            "COMSPEC",
            "PATHEXT",
            "OS",
            "TEMP",
            "TMP",
        ] {
            if let Ok(value) = std::env::var(name) {
                cmd.env(name, value);
            }
        }
    }
    let mut child = cmd.spawn().expect("spawn rubash");
    wait_bounded(&mut child, limit)
}

/// run_rubash_in with the discriminating stdin state of the readhang
/// family: a pipe whose write end this test holds open and never writes
/// to — open, empty, and never EOF. The Child handle keeps that write
/// end alive for the whole bounded wait; an engine that reaches for the
/// inherited process stdin where GNU would see EOF (or would not read at
/// all) blocks until the bound kills it.
fn run_rubash_piped_stdin_in(dir: &Path, args: &[&str], limit: Duration) -> RunOutcome {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rubash"));
    cmd.args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", minimal_path())
        .env("HOME", dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if cfg!(windows) {
        for name in [
            "SystemRoot",
            "SystemDrive",
            "WINDIR",
            "COMSPEC",
            "PATHEXT",
            "OS",
            "TEMP",
            "TMP",
        ] {
            if let Ok(value) = std::env::var(name) {
                cmd.env(name, value);
            }
        }
    }
    let mut child = cmd.spawn().expect("spawn rubash");
    // Deliberately leave child.stdin (the pipe write end) unwritten and
    // open: this is the "parent stdin stays piped open" condition.
    let outcome = wait_bounded(&mut child, limit);
    drop(child.stdin.take());
    outcome
}

fn assert_bytes_eq(actual: &[u8], expected: &[u8], label: &str, name: &str) {
    if actual == expected {
        return;
    }
    let show = |data: &[u8]| -> String {
        let text = String::from_utf8_lossy(data);
        let lines: Vec<&str> = text.split('\n').collect();
        if lines.len() <= 40 {
            return lines.join("\n");
        }
        let marker = format!("... {} more lines ...", lines.len() - 40);
        let mut joined = lines[..20].join("\n");
        joined.push('\n');
        joined.push_str(&marker);
        joined.push('\n');
        joined.push_str(&lines[lines.len() - 20..].join("\n"));
        joined
    };
    panic!(
        "{name}: {label} differs from golden.\n--- expected ---\n{}\n--- actual ---\n{}\n-----------",
        show(expected),
        show(actual)
    );
}

#[test]
fn golden_fixture_matrix_smoke() {
    // Guards the table itself: every fixture .sh has a golden pair and
    // every golden pair has a fixture (catches rename drift at a glance
    // instead of per-test panics).
    let root = manifest_dir();
    let mut fixtures: Vec<String> = std::fs::read_dir(root.join(FIXTURE_DIR))
        .expect("fixture dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".sh"))
        .collect();
    fixtures.sort();
    for f in &fixtures {
        let stem = f.trim_end_matches(".sh");
        for ext in [".gnu.out", ".gnu.err"] {
            let golden = root.join(GOLDEN_DIR).join(format!("{stem}{ext}"));
            assert!(golden.is_file(), "fixture {f} has no golden {stem}{ext}");
        }
    }
}

#[cfg(windows)]
/// Strip Windows CR from suite output and anonymize the scratch root and
/// the shell-under-test path so snapshots do not embed absolute paths.
fn normalize_snapshot(bytes: &[u8], scratch: &Path) -> Vec<u8> {
    let text = String::from_utf8_lossy(bytes);
    let scratch_fwd = scratch.to_string_lossy().replace('\\', "/");
    let scratch_raw = scratch.to_string_lossy().to_string();
    let exe_fwd = env!("CARGO_BIN_EXE_rubash").replace('\\', "/");
    let exe_raw = env!("CARGO_BIN_EXE_rubash").to_string();
    // `hash -p ${THIS_SH} ...` style output renders the exe in POSIX form
    // (/d/...); anonymize that form too.
    let exe_posix_prefix = {
        let lowered = exe_fwd.to_lowercase();
        if let Some(rest) = lowered.strip_prefix("d:/") {
            format!("/d/{}", rest)
        } else {
            String::new()
        }
    };
    // The manifest root itself can surface (e.g. `cd -` printing OLDPWD);
    // scrub it in all three renderings.
    let root_fwd = manifest_dir().to_string_lossy().replace('\\', "/");
    let root_raw = manifest_dir().to_string_lossy().to_string();
    let root_posix = {
        let lowered = root_fwd.to_lowercase();
        if let Some(rest) = lowered.strip_prefix("d:/") {
            format!("/d/{}", rest)
        } else {
            String::new()
        }
    };
    let joined = text
        .split('\n')
        .map(|line| line.replace('\r', ""))
        .map(|line| line.replace(&scratch_fwd, "<T>"))
        .map(|line| line.replace(&scratch_raw, "<T>"))
        .map(|line| line.replace(&exe_fwd, "<SH>"))
        .map(|line| line.replace(&exe_raw, "<SH>"))
        .map(|line| {
            if exe_posix_prefix.is_empty() {
                line
            } else {
                line.replace(&exe_posix_prefix, "<SH>")
            }
        })
        .map(|line| line.replace(&root_fwd, "<R>"))
        .map(|line| line.replace(&root_raw, "<R>"))
        .map(|line| {
            if root_posix.is_empty() {
                line
            } else {
                line.replace(&root_posix, "<R>")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    joined.into_bytes()
}

#[cfg(windows)]
/// rubash#812: a space-free directory to anchor machine-absolute paths the
/// suite lane produces. The ambient temp dir is per-machine layout
/// (`C:\Users\<name>\AppData\Local\Temp`, a mounted scratch volume, a
/// spaced system profile) and its SHAPE must never decide snapshot bytes,
/// so a spaced ambient falls back to `%SystemRoot%\Temp` — the same
/// candidates the #812 THIS_SH pin uses — and a host where neither is
/// space-free fails loudly instead of recording divergent bytes.
fn space_free_temp_root() -> PathBuf {
    let ambient = std::env::temp_dir();
    if !ambient.to_string_lossy().contains(' ') {
        return ambient;
    }
    if let Ok(sysroot) = std::env::var("SystemRoot") {
        let sys_temp = Path::new(&sysroot).join("Temp");
        if !sys_temp.to_string_lossy().contains(' ') {
            return sys_temp;
        }
    }
    panic!(
        "rubash#812: no space-free temp root available; the snapshot lane \
         cannot produce machine-independent bytes here"
    );
}

#[cfg(windows)]
/// rubash#812: pin THIS_SH to a space-free copy of the engine for suite
/// runs. The upstream assets expand ${THIS_SH} UNQUOTED
/// (errors.tests:369 `${THIS_SH} ./errors10.sub`; errors10.sub:10-16 then
/// re-invokes it for every `after exit:`/`after shift:` status line), and
/// with no explicit value the engine auto-detects current_exe's FULL path
/// (executor/init.rs). When the checkout lives under a path containing a
/// space — any per-machine layout outside this repo — every
/// sub-invocation word-splits (`.../no: No such file or directory`) and
/// the errors slice's status lines COLLAPSE: a host-shape-dependent byte
/// set that no snapshot normalizer can repair (the lines are absent, not
/// misspelled; errors.snap:63-80 disappears whole). GNU never sees this
/// because /usr/local/bin/bash has no space, and true-baseline.sh pins
/// the GNU side contractually (`THIS_SH=$GNU_BASH`); pin the snapshot
/// side the same way with a copy of this test's own engine named
/// `bash.exe` under a space-free directory. The engine keeps an inherited
/// THIS_SH exactly when its basename stem is sh/bash and the path exists
/// (executor/init.rs `inherited_sh`), so the child resolves ${THIS_SH} to
/// the pinned copy on every host; a bash-named wrapper is the blessed
/// conventional-name configuration (same comment). Bytes on a space-free
/// host are unchanged — the same engine runs either way — so this is not
/// a re-record; a spaced-host run converges onto the committed bytes.
fn space_free_this_sh(scratch: &Path) -> (PathBuf, Option<PathBuf>) {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_rubash"));
    let mut candidates = vec![(scratch.join(".thissh"), None)];
    if let Ok(sysroot) = std::env::var("SystemRoot") {
        let sys_temp = Path::new(&sysroot)
            .join("Temp")
            .join(format!("rubash-812-thissh-{}", std::process::id()));
        // The fallback must be cleaned up by the caller (it lives outside
        // the per-suite scratch dir).
        candidates.push((sys_temp.clone(), Some(sys_temp)));
    }
    for (cdir, cleanup) in candidates {
        if cdir.to_string_lossy().contains(' ') {
            continue;
        }
        if std::fs::create_dir_all(&cdir).is_ok() {
            let shadow = cdir.join("bash.exe");
            if std::fs::copy(&exe, &shadow).is_ok() {
                return (shadow, cleanup);
            }
        }
    }
    // The scratch dir passed here is anchored at a space-free root by the
    // caller (space_free_temp_root), so the first candidate above wins in
    // practice; the SystemRoot fallback stays as a belt against a future
    // caller reorder, and the panic below is the loud host-shape failure
    // the lane prefers over divergent bytes.
    panic!(
        "rubash#812: no space-free directory available to pin THIS_SH; \
         the snapshot lane cannot produce machine-independent bytes here"
    );
}

#[cfg(windows)]
fn run_suite_snapshot(suite: &str) {
    let root = manifest_dir();
    let src = root.join(UPSTREAM_TESTS);
    // rubash#812: anchor the scratch at a space-free root so the suite's
    // whole path surface (HOME, the .thissh pin, TMPDIR below) keeps one
    // shape on every host. The scratch path itself is scrubbed by
    // normalize_snapshot, but what MATTERS is that assets expand the
    // machine-absolute variables built from it UNQUOTED (see the TMPDIR
    // pin below), so the space must never be there in the first place.
    let dir = space_free_temp_root().join(format!("rubash-snap-{suite}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create snapshot scratch dir");
    // rubash#812: the shell-visible temp dir, pinned like true-baseline.sh
    // pins the GNU side (`TMPDIR="$w/tmp"` in scripts/true-baseline.sh).
    // Created eagerly: the assets redirect into $TMPDIR with no mkdir.
    let suite_tmp = dir.join("tmp");
    std::fs::create_dir_all(&suite_tmp).expect("create snapshot TMPDIR");
    let (this_sh, this_sh_cleanup) = space_free_this_sh(&dir);

    // Copy the suite and everything it may reference relative to cwd:
    // <suite>.tests plus every <suite>*.{sub,right} neighbor.
    let mut copied = 0;
    if let Ok(entries) = std::fs::read_dir(&src) {
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            let wanted = fname == format!("{suite}.tests")
                || fname == format!("{suite}.right")
                || (fname.starts_with(suite)
                    && (fname.ends_with(".tests")
                        || fname.ends_with(".sub")
                        || fname.ends_with(".right")))
                || fname == "recho"
                || fname == "zecho"
                || fname == "printenv"
                || fname == "bash";
            if wanted && entry.path().is_file() {
                // Normalize CRLF to LF while copying (rubash#803). The
                // windows-latest CI image materializes the third_party/bash
                // submodule with core.autocrlf=true, so every suite file
                // lands CRLF there while a developer checkout (autocrlf
                // unset) keeps the canonical LF bytes — and the suite's
                // stdout diverges by whole faces (nquote4.sub's
                // `alias foo=$'echo foo\n\nquux\n'` went silent on CI:
                // 1226 runner bytes vs 1230 LF bytes vs 1230 under WSL
                // GNU bash 5.3.0). The suite executes a unix corpus: the
                // 144 tracked files matching this copy predicate (13
                // slices) contain no CR in any canonical blob; on-disk
                // copies of a built submodule can add untracked build
                // artifacts such as `recho`, which are outside that corpus
                // claim. The pair-wise conversion is therefore exactly the
                // autocrlf inverse and cannot touch semantic CR bytes;
                // the recorded snapshot bytes stop depending on the
                // checkout's line-ending config.
                let bytes = std::fs::read(entry.path()).unwrap_or_default();
                if bytes.is_empty() {
                    let _ = std::fs::copy(entry.path(), dir.join(&fname));
                } else {
                    let mut lf = Vec::with_capacity(bytes.len());
                    let mut it = bytes.iter().copied().peekable();
                    while let Some(b) = it.next() {
                        if b == b'\r' && it.peek() == Some(&b'\n') {
                            continue; // CRLF pair: the CR is autocrlf noise
                        }
                        lf.push(b);
                    }
                    let _ = std::fs::write(dir.join(&fname), &lf);
                }
                copied += 1;
            }
        }
    }
    assert!(
        copied >= 1,
        "{suite}: no suite files found under {}",
        src.display()
    );

    let helpers = root.join(HELPERS_WIN);
    let sep = if cfg!(windows) { ';' } else { ':' };
    let mut path = helpers.to_string_lossy().to_string();
    path.push(sep);
    path.push_str(&minimal_path());

    let started = Instant::now();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rubash"));
    cmd.arg(format!("{suite}.tests"))
        .current_dir(&dir)
        // Airtight environment — same contract as run_rubash_in above.
        // The snapshot path originally kept the ambient env because the
        // first recording happened to be made with it, and that choice
        // made the bytes machine-dependent: exp.tests:416 runs
        // `${THIS_SH} -c ...`, so any machine that exports THIS_SH (a
        // shell-managed env) recorded extra argv lines a clean runner
        // never produces — and shell option variables (BASHOPTS,
        // SHELLOPTS, POSIXLY_CORRECT classes) are exactly the inputs the
        // alias-expansion machinery keys on. Drop everything user/tool
        // shaped and pass machine infrastructure + the explicit PATH, so
        // a local run, the CI runner and any future image see the same
        // suite world (see the re-record protocol in the module header).
        .env_clear()
        .env("PATH", &path)
        .env("HOME", &dir)
        // rubash#812: the pinned space-free THIS_SH (see
        // space_free_this_sh) — the one deliberate variable, because the
        // upstream assets invoke ${THIS_SH} unquoted and the auto-detected
        // full exe path word-splits on any checkout layout with a space.
        .env("THIS_SH", &this_sh)
        // rubash#812, the second machine-absolute variable: the engine
        // self-injects a Windows TMPDIR default from the process temp dir
        // (executor/init.rs, the /tmp mapping fixture), and the upstream
        // assets expand it UNQUOTED for real faces in three slices —
        // heredoc.tests:115-124 (`cat > ${TMPDIR}/bash-zzz-$$` heredoc
        // round-trip), quote.tests:136-141 (`printf ... > $TMPDIR/quote-$$`
        // re-sourced through ${THIS_SH} $TMPDIR/...), and the redir.tests
        // file battery (redir.tests:35-119, `> $TMPDIR/null-redir-a` plus
        // the `testf`/`oops -- $1 not found` branch). Under a spaced
        // ambient TEMP every one of those paths word-splits and the faces
        // go silent or fall to the oops branch: verified, spaced-TEMP run
        // shifted heredoc/quote/redir against the committed bytes (heredoc
        // 1483 -> 1463; redir printed `oops -- /d/temp-812/rub not found`)
        // while CI's space-free TEMP stayed green — the same machine
        // dependence as the THIS_SH face one layer over. The value never
        // prints on a success face in any of the 13 slices (file targets
        // and PATH assignments only), so pinning it is byte-neutral on a
        // space-free host and converges spaced hosts onto the committed
        // bytes; being under the scratch, even a hypothetical leak renders
        // as the normalized `<T>` shape.
        .env("TMPDIR", &suite_tmp)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if cfg!(windows) {
        for name in [
            "SystemRoot",
            "SystemDrive",
            "WINDIR",
            "COMSPEC",
            "PATHEXT",
            "OS",
            "TEMP",
            "TMP",
        ] {
            if let Ok(value) = std::env::var(name) {
                cmd.env(name, value);
            }
        }
    }
    let mut child = cmd.spawn().expect("spawn rubash for suite");
    let out = wait_bounded(&mut child, Duration::from_secs(120));

    let snap_path = root.join(SNAPSHOT_DIR).join(format!("{suite}.snap"));
    let normalized = normalize_snapshot(&out.stdout, &dir);
    let _ = std::fs::remove_dir_all(&dir);
    if let Some(cleanup) = this_sh_cleanup {
        let _ = std::fs::remove_dir_all(cleanup);
    }

    if std::env::var("RUBASH_REGRESSION_RECORD").as_deref() == Ok("1") {
        std::fs::write(&snap_path, &normalized).expect("record snapshot");
        eprintln!("recorded {suite}.snap ({} bytes)", normalized.len());
        return;
    }
    let expected = std::fs::read(&snap_path).unwrap_or_else(|e| {
        panic!("{suite}: missing snapshot (record with RUBASH_REGRESSION_RECORD=1): {e}")
    });
    assert!(
        !out.timed_out,
        "{suite}: suite run timed out ({}s)",
        started.elapsed().as_secs()
    );
    assert_bytes_eq(&normalized, &expected, "snapshot stdout", suite);
}

#[cfg(windows)]
#[test]
fn upstream_suite_slice_snapshots() {
    let root = manifest_dir();
    std::env::set_current_dir(&root).expect("cwd to manifest dir");
    for suite in SNAPSHOT_SUITES {
        run_suite_snapshot(suite);
    }
}

// ---------------------------------------------------------------------------
// Perf canaries: generous absolute bounds that only fail on the O(N^2)
// parser blowup class (rubash#176/#178/#185: nested-brace re-scan and
// dolbrace span re-collection; measured pre-fix at >300s). They share one
// mutex so they never compete for CPU with each other (cargo runs tests
// in parallel; a contended timing bound is a flaky bound).
// ---------------------------------------------------------------------------

static HEAVY_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn perf_canary_nvm_loader_parses_bounded() {
    let _guard = HEAVY_TEST_LOCK.lock().unwrap();
    let root = manifest_dir();
    let dir = std::env::temp_dir().join(format!("rubash-perf-nvm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create perf scratch");
    std::fs::copy(root.join(CORPUS_DIR).join("nvm/nvm.sh"), dir.join("nvm.sh"))
        .expect("copy nvm corpus");
    let started = Instant::now();
    let out = run_rubash_in(&dir, &["-n", "nvm.sh"], Duration::from_secs(240));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.code, Some(0), "bash -n nvm.sh must parse clean");
    assert!(
        started.elapsed() < Duration::from_secs(180),
        "nvm.sh -n took {:.1}s (bound 180s; O(N^2) lexer regression class)",
        started.elapsed().as_secs_f32()
    );
}

#[test]
fn perf_canary_ruby_build_parse_bounded() {
    let _guard = HEAVY_TEST_LOCK.lock().unwrap();
    let root = manifest_dir();
    let dir = std::env::temp_dir().join(format!("rubash-perf-rbld-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create perf scratch");
    std::fs::copy(
        root.join(CORPUS_DIR).join("ruby-build/ruby-build"),
        dir.join("ruby-build"),
    )
    .expect("copy ruby-build corpus");
    let started = Instant::now();
    let out = run_rubash_in(&dir, &["-n", "ruby-build"], Duration::from_secs(360));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.code, Some(0), "bash -n ruby-build must parse clean");
    assert!(
        started.elapsed() < Duration::from_secs(300),
        "ruby-build -n took {:.1}s (bound 300s; #185 dolbrace regression class)",
        started.elapsed().as_secs_f32()
    );
}

/// Synthetic nested-brace + autoconf-mkdir_p repetition shape (the
/// rubash#176/#178 benchmark forms from commit 9bf2df9e), generated in
/// place so no corpus is needed. Post-fix debug timings are ~1-10s;
/// pre-fix the nested form exceeded 300s.
#[test]
fn perf_canary_synthetic_brace_repetition() {
    let _guard = HEAVY_TEST_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("rubash-perf-rep-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create perf scratch");

    let as_fn_mkdir_p = r#"
as_fn_mkdir_p () {
  case $as_dir in #(
    -*) as_dir=./$as_dir;;
    esac
  test -d "$as_dir" || eval $as_mkdir_p || {
    as_dirs=
    while : ; do
      case $as_dir in #(
      *\'*) as_dir=`$as_echo "$as_dir" | sed "s/'/'\\\\\\\\''/g"`;; #(
      *) as_dir=$as_dir;;
      esac
      as_dirs="'$as_dir' $as_dirs"
      as_dir=`$as_dirname -- "$as_dir" ||
      $as_expr X"$as_dir" : 'X\(.*[^/]\)//*[^/][^/]*/*$' \| \
       X"$as_dir" : 'X\(//\)[^/]' \| \
       X"$as_dir" : 'X\(//\)$' \| \
       X"$as_dir" : 'X\(/\)' \| . 2>/dev/null ||
      $as_echo X"$as_dir" |
      sed '/^X\(.*[^/]\)\/\/*[^/][^/]*\/*$/{
        s//\1/
        q
      }
      /^X\(\/\/\)[^/].*/{
        s//\1/
        q
      }
      /^X\(\/\/\)$/{
        s//\1/
        q
      }
      /^X\(\/\).*/{
        s//\1/
        q
      }
      s/.*/./; q'`
      test -d "$as_dir" && break
    done
    test -z "$as_dirs" || eval "mkdir $as_dirs"
  } || test -d "$as_dir" || as_fn_error $? "cannot create directory $as_dir"
}
"#;
    let mut script = String::new();
    // 20 units, not 24: a 24-unit build trips a known open parse
    // divergence (identical units x24 + a nested group -> spurious
    // "unexpected end of file from `{'"; GNU parses clean; reproducer
    // saved as target/ciwork/syn/rep-boundary-24.sh during codification,
    // 2026-09-27). Reported to the crew; do not raise the count until
    // that lands.
    for i in 0..20 {
        script.push_str(&format!("# rep {i}\n{as_fn_mkdir_p}\n"));
    }
    // Nested multi-line brace groups (the nst2 shape). Capped at depth 3:
    // a depth-4 group anywhere in the file makes rubash (master 9bf2df9e,
    // brace_scan_cache) mis-parse the EARLIER depth-2 group with
    // "unexpected end of file from `{'" while GNU parses clean — a
    // regression introduced by 9bf2df9e (builds <= cf322dda are fine).
    // Minimal reproducer saved at
    // target/issue-suites/results/wt4-ci/nested-brace-depth4-repro.sh;
    // raise this cap once that lands.
    for depth in 1..=3usize {
        script.push_str(&"{\n".repeat(depth));
        script.push_str("echo nested\n");
        script.push_str(&"}\n".repeat(depth));
        script.push('\n');
    }
    std::fs::write(dir.join("rep.sh"), &script).expect("write synthetic script");
    let _ = std::fs::create_dir_all("target/ciwork");
    let _ = std::fs::write("target/ciwork/syn-rust-rep.sh", &script);

    let started = Instant::now();
    let out = run_rubash_in(&dir, &["-n", "rep.sh"], Duration::from_secs(240));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.code, Some(0), "synthetic script must parse clean");
    assert!(
        started.elapsed() < Duration::from_secs(150),
        "synthetic brace/mkdir_p -n took {:.1}s (bound 150s; #176/#178 class)",
        started.elapsed().as_secs_f32()
    );
}

/// The `yes | head | while read` termination canary (rubash#206/#243).
/// Pre-#206-fix (before 62c7e5fd) this shape never terminated and RSS grew
/// unbounded (the sequential stage loop `wait_with_output`'d stage-0 `yes`
/// into one Vec forever; GNU execute_cmd.c:2620 execute_pipeline forks every
/// left element on pipe(2) instead). Post-fix it finishes rc=0 with bounded
/// RSS: ~7 s for N=10000 in debug (this canary), ~23-41 s for N=100000
/// (release ~3.5-6 s). rubash#243's "never terminates" report was the
/// perf-suite timeout=20 truncating the N=100000 debug run, not a
/// regression — this canary pins the class so a real one shows up as a
/// timeout instead of an RSS explosion. `yes`/`head` need no PATH: the
/// concurrent external admission falls back to the internal pipeline
/// utilities (src/main.rs run_internal_pipeline_utility).
#[test]
fn perf_canary_yes_head_read_terminates() {
    let _guard = HEAVY_TEST_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("rubash-perf-yhr-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create perf scratch");
    std::fs::write(
        dir.join("yhr.sh"),
        "yes | head -10000 | while read -r l; do :; done\necho \"RC=$?\"\nexit 0\n",
    )
    .expect("write yhr script");
    let started = Instant::now();
    let out = run_rubash_in(&dir, &["yhr.sh"], Duration::from_secs(90));
    let elapsed = started.elapsed();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        !out.timed_out,
        "yes|head|read pipeline did not terminate in 90s (#206 class regression)"
    );
    assert_eq!(
        out.code,
        Some(0),
        "yhr.sh exit status, stderr: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"RC=0\n", "yhr.sh stdout");
    assert!(
        elapsed < Duration::from_secs(60),
        "yes|head|read N=10000 took {:.1}s (bound 60s; #206/#243 class)",
        elapsed.as_secs_f32()
    );
}

/// A short consumer must close its read end while the producer is still
/// running. The parent must drain the final stage before waiting upstream;
/// otherwise `yes` can remain blocked forever with `head -1` already exited.
#[test]
fn yes_head_one_terminates_after_downstream_closes() {
    let _guard = HEAVY_TEST_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("rubash-yes-head-one-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create pipeline scratch");
    std::fs::write(dir.join("yes-head-one.sh"), "yes | head -1\n").expect("write pipeline script");

    let out = run_rubash_in(&dir, &["yes-head-one.sh"], Duration::from_secs(10));
    let _ = std::fs::remove_dir_all(&dir);

    assert!(!out.timed_out, "yes | head -1 did not terminate");
    assert_eq!(out.code, Some(0), "pipeline exit status");
    assert_eq!(out.stdout, b"y\n", "pipeline output");
    assert!(out.stderr.is_empty(), "pipeline stderr: {:?}", out.stderr);
}

/// readhang regression (2026-09-28, bisected to b6cc735a): a script-file
/// `while IFS= read -r ...; done < file` loop must terminate on the
/// redirect's own EOF while the parent's stdin stays piped open.
///
/// Root cause: b6cc735a's shared-offset binding (command_input_scope.rs
/// compound_fd0_regular_file) gave the compound's fd 0 a real FileFd
/// WITHOUT the FUNCTION_STDIN mirror, so the loop-EOF `read` exhausted
/// every transport and fell through read_input_for_command's legacy
/// tail into read_inherited_process_stdin — a blocking raw read of the
/// parent's still-open stdin pipe. GNU builtin_read reads exactly ONE
/// descriptor, fd 0 as do_redirections bound it (read.def:294;
/// redir.c:767-955): zread returning <= 0 sets eof and breaks
/// (read.def:762-768) and the builtin returns EXECUTION_FAILURE
/// (read.def:949) — there is no second input to fall through to. The
/// fix keys the process-stdin fallback on fd 0's endpoint
/// (FdTable::fd0_bound_away_from_process_stdin) in both
/// read_io.rs::read_input_for_command and read_builtin.rs::execute_read.
///
/// Expected bytes verified against WSL GNU Bash 5.3.0
/// (`/usr/local/bin/bash`, script file, same piped-open parent stdin):
/// `<alpha>\n<beta>\ncount=2\n` on stdout, empty stderr, rc=0.
#[test]
fn script_file_while_read_redirect_does_not_wait_for_parent_stdin() {
    let dir = std::env::temp_dir().join(format!("rubash-readhang-fs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create readhang scratch");
    std::fs::write(dir.join("list.txt"), b"alpha\nbeta\n").expect("write list.txt");
    std::fs::write(
        dir.join("readlist.sh"),
        concat!(
            "count=0\n",
            "while IFS= read -r file; do\n",
            "  printf '<%s>\\n' \"$file\"\n",
            "  count=$((count + 1))\n",
            "done < list.txt\n",
            "printf 'count=%s\\n' \"$count\"\n",
        ),
    )
    .expect("write readlist.sh");
    let out = run_rubash_piped_stdin_in(&dir, &["readlist.sh"], Duration::from_secs(15));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        !out.timed_out,
        "readlist.sh hung: loop-EOF read waited for the parent's piped-open stdin \
         (read_input_for_command / execute_read fell through to the inherited stdin)"
    );
    assert_eq!(
        out.code,
        Some(0),
        "readlist.sh exit status, stderr: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        out.stdout, b"<alpha>\n<beta>\ncount=2\n",
        "readlist.sh stdout"
    );
    assert_eq!(out.stderr, b"", "readlist.sh stderr");
}

/// readhang companion (pre-existing since 26780a7b, same macro-symptom):
/// `cat file | while IFS= read -r ...` in a script file with the parent
/// stdin piped open must run the pipeline without the shell ever reading
/// the child's stdin.
///
/// Root cause: initial_pipeline_input (pipeline_exec.rs) eagerly slurped
/// the WHOLE inherited process stdin (`read_to_string`, blocking on an
/// open pipe) whenever stage 0's command name matched a hardcoded list
/// (cat/grep/sed/head/...) — even `cat list.txt`, which has an operand
/// and never reads stdin. GNU execute_cmd.c execute_pipeline forks every
/// element with the shell's own descriptors: the first element's fd 0 IS
/// the shell's fd 0, and the parent never reads a child's stdin. The
/// slurp (and its name-list predicate) was deleted; stage 0 gets the
/// real handle via execute_pipeline_stage's stdin_inherit arm instead.
///
/// Expected bytes verified against WSL GNU Bash 5.3.0: the while runs in
/// the pipeline's subshell, so the trailing count is 0 —
/// `<alpha>\n<beta>\ncount=0\n`, empty stderr, rc=0.
#[test]
fn script_file_cat_pipe_while_read_does_not_slurp_parent_stdin() {
    let dir = std::env::temp_dir().join(format!("rubash-readhang-pe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create readhang scratch");
    std::fs::write(dir.join("list.txt"), b"alpha\nbeta\n").expect("write list.txt");
    std::fs::write(
        dir.join("pipeelem.sh"),
        concat!(
            "count=0\n",
            "cat list.txt | while IFS= read -r file; do\n",
            "  printf '<%s>\\n' \"$file\"\n",
            "  count=$((count + 1))\n",
            "done\n",
            "printf 'count=%s\\n' \"$count\"\n",
        ),
    )
    .expect("write pipeelem.sh");
    let out = run_rubash_piped_stdin_in(&dir, &["pipeelem.sh"], Duration::from_secs(15));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        !out.timed_out,
        "pipeelem.sh hung: initial_pipeline_input slurped the parent's piped-open stdin"
    );
    assert_eq!(
        out.code,
        Some(0),
        "pipeelem.sh exit status, stderr: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        out.stdout, b"<alpha>\n<beta>\ncount=0\n",
        "pipeelem.sh stdout"
    );
    assert_eq!(out.stderr, b"", "pipeelem.sh stderr");
}

/// The GNU brace/separator acceptance rules (rubash#241 side-findings,
/// fixed in the perffix lane). Four classes, each verified byte-for-byte
/// against WSL GNU bash 5.3.0 (matrix under
/// target/issue-suites/results/perffix/matrix.sh):
///
/// 1. `}}'/`}x' are literal WORDS, not reserved `}' tokens — GNU
///    syntax.h:29-30 puts `}' in neither shell_meta_chars nor
///    shell_break_chars, and CHECK_FOR_RESERVED_WORD (parse.y:3168,
///    STREQ at parse.y:3174-3175) needs the whole word to be exactly
///    `}'. `{{ echo hi; }}' runs both words as commands (127), and
///    `echo } }x }}' prints exactly `} }x }}'.
/// 2. `;;' (SEMI_SEMI, parse.y:3711-3717) is grammatical only in
///    case_clause_sequence (parse.y:1237-1247): `{ :;;}' and `{ ;; :; }'
///    are rc=2 `syntax error near unexpected token `;;''.
/// 3. An empty separator segment is never grammatical: the list grammar
///    (parse.y:1264-1290) requires `list1 ';' newline_list list1', so
///    `; :', `: ; ; :', `: <newline> ; :' and `: & & :' are rc=2 errors
///    naming `;' (or `&').
/// 4. Legal shapes stay legal: `{ :; }', `{ : ; }', trailing `; :',
///    `cat <<EOF ; :', `case $x in a) : ; esac', `for ((i=0;;i++))'.
#[test]
fn parse_acceptance_gnu_separator_and_brace_word_rules() {
    let _guard = HEAVY_TEST_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("rubash-pa-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch");

    let reject = [
        ("{{ :; }\n", "`}'"),
        ("{ :;;}\n", "`;;'"),
        ("{ :; ; }\n", "`;'"),
        ("; :\n", "`;'"),
        (": ; ; :\n", "`;'"),
        (":\n; :\n", "`;'"),
        (": & & :\n", "`&'"),
        ("if :; then :; ; fi\n", "`;'"),
    ];
    for (index, (source, token)) in reject.iter().enumerate() {
        std::fs::write(dir.join(format!("rej{index}.sh")), source).expect("write");
        let out = run_rubash_in(&dir, &[&format!("rej{index}.sh")], Duration::from_secs(30));
        assert_eq!(
            out.code,
            Some(2),
            "rej{index} ({source:?}) rc, stderr: {:?}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains(&format!("syntax error near unexpected token {token}")),
            "rej{index} ({source:?}) stderr: {stderr}"
        );
    }

    // `}}' is a literal word: GNU runs `{{' / `}}' as commands (127).
    std::fs::write(dir.join("word.sh"), "{{ echo hi; }}\n").expect("write");
    let out = run_rubash_in(&dir, &["word.sh"], Duration::from_secs(30));
    assert_eq!(out.code, Some(127));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("{{: command not found"), "stderr: {stderr}");
    assert!(stderr.contains("}}: command not found"), "stderr: {stderr}");

    std::fs::write(dir.join("args.sh"), "echo } }x }}\n").expect("write");
    let out = run_rubash_in(&dir, &["args.sh"], Duration::from_secs(30));
    assert_eq!(out.code, Some(0));
    assert_eq!(out.stdout, b"} }x }}\n");

    // The legal neighborhood must stay legal (rc 0, no stderr).
    let accept = [
        "{ :; }\n",
        "{ : ; }\n",
        ": ;\n",
        "cat <<EOF ; :\nbody\nEOF\n",
        "case $x in a) : ; esac\n",
        "for ((i=0;;i++)); do break; done\n",
        "f() {\n:\n}\nf\n",
    ];
    for (index, source) in accept.iter().enumerate() {
        std::fs::write(dir.join(format!("acc{index}.sh")), source).expect("write");
        let out = run_rubash_in(&dir, &[&format!("acc{index}.sh")], Duration::from_secs(30));
        assert_eq!(
            out.code,
            Some(0),
            "acc{index} ({source:?}) rc, stderr: {:?}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            out.stderr.is_empty(),
            "acc{index} ({source:?}) stderr: {:?}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The bash_completion full-file `bash -n` canary (rubash#131): GNU and
/// rubash both reject the corpus at line 1820 with rc 2. The corpus is
/// GPL-2.0+ and is deliberately NOT vendored (target-ecosys license
/// guidance), so this canary runs only where the lane sandbox is present
/// and skips (with the reason) elsewhere — CI included.
#[test]
fn canary_bash_completion_bash_n_when_corpus_present() {
    let corpus =
        PathBuf::from("D:/repo/rubash/target-ecosys/repos/bash-completion/bash_completion");
    if !corpus.is_file() {
        eprintln!(
            "SKIP: bash_completion corpus not present ({}); the license-clean \
             minimal form is covered by extglob-parse-gate",
            corpus.display()
        );
        return;
    }
    let dir = std::env::temp_dir().join(format!("rubash-canary-bc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create canary scratch");
    std::fs::copy(&corpus, dir.join("bash_completion")).expect("copy corpus");
    let out = run_rubash_in(&dir, &["-n", "bash_completion"], Duration::from_secs(240));
    let _ = std::fs::remove_dir_all(&dir);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.code, Some(2), "expected rc 2, stderr:\n{stderr}");
    assert!(
        stderr.contains("line 1820: syntax error near unexpected token `('"),
        "expected the #131 extglob gate at line 1820, stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("-?(\\[)+([a-zA-Z0-9?]))"),
        "expected the offending line echo, stderr:\n{stderr}"
    );
}

// ---------------------------------------------------------------------------
// Issue case catalogue (rubash#746).
//
// Every fix below carries the id `<issue>.<n>`: the module is
// `issue_<issue>` and the test is `case_<n>_...`, so a failing CI name points
// straight at its issue (`issue_721::case_2_compound_loop_reports_enoent`).
//
// Why these live in `--test regression` instead of `tests/issueNNN_*.rs`:
// the per-issue files are only reached by a full `cargo test`, and the only
// target the Windows gate runs is `--test regression` (ci.yml, job
// `regression`). A case outside that target cannot ratchet. The module name
// keeps the issue reference that §3.4 of
// docs/pitfall-taxonomy-and-governance.md requires, and the test name keeps
// the #746 catalogue id.
//
// Case states (#746): a `ratchet` case is `must-match` and MUST turn red when
// the fix is reverted — that is the §3.4 criterion. A `control` case asserts a
// behaviour the fix must NOT change (a live fd still reads, `-n` still splits);
// it passes on the pre-fix engine by construction, so it is never counted as
// the issue's ratchet. A `guard` case pins a fix DIRECTION: it is green on the
// original bug and on the final fix, and red on the wrong fix that was tried in
// between. States are marked in each case's doc comment.
//
// GNU Bash 5.3.0 is the sole oracle, stdout and stderr are asserted separately
// (a single `2>&1` capture interleaves differently and produced a false
// positive in the #706 sweep).
//
// Revert check for this catalogue (2026-10-18): restoring all 25 touched `src/`
// files to their pre-fix parents left the engine compiled but turned 18 of the
// 20 cases red. The two that stayed green are exactly the cases marked
// `[control]` / `[guard]` below, which is the expected shape. The issue_703
// batch (same day): reverting the comsub-collector fix turned 4 of its 5
// cases red; the survivor is the 703.2 control (the shapes the issue report
// named as already working).
// ---------------------------------------------------------------------------

/// Per-case scratch directory. The case id keeps concurrent cases of the same
/// issue from sharing one directory under the harness's default parallelism.
fn case_scratch(case: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rubash-{case}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch");
    dir
}

/// niubash#362: `&` on a shell carrying a large function table must not die
/// with "File name too long".
///
/// The async child is re-exec'd with the WHOLE function table serialized into
/// its `-c` script (`compound_exec::background_command_source`). On Windows
/// that string rides `CreateProcessW`'s lpCommandLine, capped at 32767 UTF-16
/// units, so a shell with a few thousand functions aborts the spawn with
/// ERROR_FILENAME_EXCED_RANGE — surfaced as `niu: File name too long` — even
/// for a trivial `echo hello &`. Unix has the analogous per-argument execve
/// cap (MAX_ARG_STRLEN, 128 KiB). The spawn side now hands an over-budget
/// script to the child through a temp file named by __RUBASH_CMD_FILE, and the
/// entry point splices it back into the `-c` operand before parsing, so argv
/// semantics are unchanged.
mod issue_niubash_362 {
    use super::*;

    /// 362.1 — the generated script (~6000 functions, ~200 KB serialized)
    /// clears BOTH caps, so this fails pre-fix on Windows AND on Unix; the
    /// background child must still print its output and exit 0.
    #[test]
    fn case_1_background_command_with_large_function_table() {
        let _guard = HEAVY_TEST_LOCK.lock().unwrap();
        let dir = case_scratch("362.1");

        let mut script = String::new();
        for i in 0..6000 {
            script.push_str(&format!("f{i}() {{ local v={i}; : \"$v\"; }}\n"));
        }
        script.push_str("echo hello &\nwait\necho large-fn-status:$?\n");
        std::fs::write(dir.join("big.sh"), &script).expect("write script");

        let out = run_rubash_in(&dir, &["big.sh"], Duration::from_secs(60));
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.timed_out, "timed out; stderr:\n{stderr}");
        assert_eq!(out.code, Some(0), "stderr:\n{stderr}");
        assert!(
            stdout.contains("hello"),
            "background child output missing; stdout:\n{stdout}\nstderr:\n{stderr}"
        );
        assert!(
            stdout.contains("large-fn-status:0"),
            "wait status wrong; stdout:\n{stdout}\nstderr:\n{stderr}"
        );
        assert!(
            !stderr.contains("File name too long") && !stderr.contains("Argument list too long"),
            "spawn hit an argv/command-line cap; stderr:\n{stderr}"
        );
    }
}

/// rubash#780: an argument-less `-O`/`+O` is the shopt LIST request, not a
/// usage error — the sibling of the `-o`/`+o` case fixed in rubash#547.
mod issue_780 {
    use super::*;

    /// 780.1 — GNU 5.3.0 prints the shopt table (`name<TAB>state`, rc 0) for
    /// `-O` and the `shopt -u|-s name` recreate list for `+O`, then continues
    /// startup. Asserted against the engine's own builtin bytes so the check
    /// needs no GNU oracle: startup `-O` equals `shopt`, `+O` equals `shopt -p`.
    #[test]
    fn case_1_argument_less_capital_o_prints_shopt_listing() {
        let dir = case_scratch("780.1");

        let dash = run_rubash_in(&dir, &["-O"], Duration::from_secs(30));
        let builtin = run_rubash_in(&dir, &["-c", "shopt"], Duration::from_secs(30));
        assert_eq!(
            dash.code,
            Some(0),
            "`-O` rc, stderr:\n{}",
            String::from_utf8_lossy(&dash.stderr)
        );
        assert!(!dash.stdout.is_empty(), "`-O` printed nothing");
        assert_eq!(
            dash.stdout, builtin.stdout,
            "`-O` must be the `shopt` bytes"
        );

        let plus = run_rubash_in(&dir, &["+O"], Duration::from_secs(30));
        let builtin_p = run_rubash_in(&dir, &["-c", "shopt -p"], Duration::from_secs(30));
        assert_eq!(
            plus.code,
            Some(0),
            "`+O` rc, stderr:\n{}",
            String::from_utf8_lossy(&plus.stderr)
        );
        assert!(!plus.stdout.is_empty(), "`+O` printed nothing");
        assert_eq!(
            plus.stdout, builtin_p.stdout,
            "`+O` must be the `shopt -p` bytes"
        );
    }
}

/// rubash#738: a heredoc delimiter carrying an escaped quote or backslash is
/// DEQUOTED (GNU quote removal), not stripped of those characters.
mod issue_738 {
    use super::*;

    /// 738.1 — before the fix `<<E\'OF` named `EOF`, the terminator `E'OF`
    /// never matched, and the heredoc ran to end-of-file — silently eating the
    /// rest of the script with `rc 0` and a warning on a stream scripts rarely
    /// read. Expected bytes are GNU Bash 5.3.0's: the terminator matches, the
    /// body is exactly `hello`, and execution continues after the heredoc.
    #[test]
    fn case_1_escaped_quote_and_backslash_delimiters_terminate() {
        let dir = case_scratch("738.1");

        // `<<E\'OF` … `E'OF` — an escaped single quote is literal delimiter text.
        std::fs::write(
            dir.join("esc_quote.sh"),
            "cat <<E\\'OF\nhello\nE'OF\necho after-quote\n",
        )
        .expect("write esc_quote");
        let out = run_rubash_in(&dir, &["esc_quote.sh"], Duration::from_secs(30));
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.code, Some(0), "esc_quote rc, stderr:\n{stderr}");
        assert_eq!(
            stdout, "hello\nafter-quote\n",
            "esc_quote stdout (the heredoc must end at E'OF):\n{stdout}\nstderr:\n{stderr}"
        );
        assert!(
            !stderr.contains("delimited by end-of-file"),
            "esc_quote still ran to EOF:\n{stderr}"
        );

        // `<<a\\b` … `a\b` — the escaped backslash is a literal delimiter byte.
        std::fs::write(
            dir.join("esc_backslash.sh"),
            "cat <<a\\\\b\na\\b\necho after-backslash\n",
        )
        .expect("write esc_backslash");
        let out = run_rubash_in(&dir, &["esc_backslash.sh"], Duration::from_secs(30));
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.code, Some(0), "esc_backslash rc, stderr:\n{stderr}");
        assert_eq!(
            stdout, "after-backslash\n",
            "esc_backslash stdout (the heredoc must end at a\\b):\n{stdout}\nstderr:\n{stderr}"
        );
        assert!(
            !stderr.contains("delimited by end-of-file"),
            "esc_backslash still ran to EOF:\n{stderr}"
        );
    }
}

/// niubash#404 (rubash side): GNU's exit echo is `interactive &&
/// sourcelevel == 0` (builtins/exit.def:59-62).
mod issue_niubash_404 {
    use super::*;

    /// 404.1 — a reader-phase `exit` prints "exit" to stderr; an `exit` inside
    /// a sourced file does not, which is why `bash --rcfile rc -i -c 'echo
    /// AFTER'` with `exit 3` in the rc stays silent while `bash -i -c 'exit 3'`
    /// echoes.
    #[test]
    fn case_1_exit_echo_suppressed_inside_sourced_file() {
        let dir = case_scratch("404.1");
        std::fs::write(dir.join("rc.sh"), "echo RAN\nexit 3\n").expect("write rc");

        let run = |args: &[&str]| {
            let out = Command::new(env!("CARGO_BIN_EXE_rubash"))
                .args(args)
                .current_dir(&dir)
                .env_clear()
                .env("PATH", minimal_path())
                .env("HOME", &dir)
                .env("__RUBASH_INTERACTIVE", "1")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .expect("spawn rubash");
            (
                String::from_utf8_lossy(&out.stdout).into_owned(),
                String::from_utf8_lossy(&out.stderr).into_owned(),
                out.status.code(),
            )
        };

        // Reader-phase exit: the echo is required.
        let (_, stderr, code) = run(&["--norc", "-c", "exit 3"]);
        assert_eq!(code, Some(3), "stderr:\n{stderr}");
        assert!(
            stderr.lines().any(|line| line == "exit"),
            "reader-phase `exit` must echo \"exit\" (GNU exit.def:59-62): {stderr:?}"
        );

        // Exit inside the sourced file: no echo.
        let (stdout, stderr, code) = run(&["--norc", "-c", ". rc.sh"]);
        assert_eq!(code, Some(3), "stderr:\n{stderr}");
        assert_eq!(stdout.trim_end(), "RAN", "stdout:\n{stdout}");
        assert!(
            !stderr.lines().any(|line| line == "exit"),
            "an `exit` inside a sourced file must stay silent: {stderr:?}"
        );
    }
}

/// rubash#788: a W_COMPASSIGN operand makes the variable an array even with no
/// `-a`/`-A` flag, and that array bit survives an explicit `+a` in the same
/// command (GNU expand_declaration_argument -> find_or_make_array_variable,
/// arrayfunc.c:454-514, runs BEFORE the attribute pass).
mod issue_788 {
    use super::*;

    fn run(dir: &Path, code: &str) -> (String, String, Option<i32>) {
        let out = run_rubash_in(dir, &["-c", code], Duration::from_secs(30));
        (
            String::from_utf8_lossy(&out.stdout).trim_end().to_string(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            out.code,
        )
    }

    /// 788.1 — before the fix rubash wrote the indexed storage word but never
    /// set ARRAY_VARS, so the variable still READ as a scalar holding the raw
    /// storage text: `${a[0]}` returned `([0]="x" [1]="y")`, `${#a[@]}` was 1
    /// and `${a[1]}` was empty. That is the `golden_fnbody_strict_battery`
    /// failure (`array-y` expected, `array-` printed) that kept the Windows
    /// regression job red. Expected bytes are GNU Bash 5.3.0's.
    #[test]
    fn case_1_compound_operand_sets_the_array_attribute() {
        let dir = case_scratch("788.1");

        for (code, expected) in [
            // No -a/-A: the compound list alone makes it an array.
            ("declare a=(x y); echo \"${a@a}|${#a[@]}|${a[1]}\"", "a|2|y"),
            (
                "declare a=(x); declare a+=(y); echo \"${a@a}|${#a[@]}|${a[1]}\"",
                "a|2|y",
            ),
            // A scalar promoted by a later compound append.
            (
                "declare a=x; declare a+=(y); echo \"${a@a}|${#a[@]}|${a[1]}\"",
                "a|2|y",
            ),
            // The attribute pass sees the array bit, refuses +a, keeps the array.
            (
                "declare +a a=(x y); echo \"${a@a}|${#a[@]}|${a[1]}\"",
                "a|2|y",
            ),
            // Function-scope declarations take the same path.
            (
                "f(){ local a=(x y); echo \"${a@a}|${#a[@]}|${a[1]}\"; }; f",
                "a|2|y",
            ),
            (
                "f(){ declare a=(x y); echo \"${a@a}|${#a[@]}|${a[1]}\"; }; f",
                "a|2|y",
            ),
            // A quoted parenthesized STRING is not W_COMPASSIGN: still scalar.
            ("declare a='(x y)'; echo \"${a@a}|${#a[@]}\"", "|1"),
            // An associative target keeps -A and binds the list as key/value.
            (
                "declare -A a=([k]=v); echo \"${a@a}|${#a[@]}|${a[k]}\"",
                "A|1|v",
            ),
        ] {
            let (stdout, stderr, status) = run(&dir, code);
            assert_eq!(stdout, expected, "`{code}` stdout; stderr:\n{stderr}");
            assert_eq!(status, Some(0), "`{code}` rc; stderr:\n{stderr}");
        }
    }

    /// 788.2 — the `+a` refusal is a real diagnostic, not a silent no-op.
    #[test]
    fn case_2_plus_a_is_refused_with_the_array_diagnostic() {
        let dir = case_scratch("788.2");
        let (_, stderr, status) = run(&dir, "declare +a a=(x y)");
        assert_eq!(status, Some(1), "`declare +a` must fail; stderr:\n{stderr}");
        assert!(
            stderr.contains("cannot destroy array variables in this way"),
            "`declare +a a=(x y)` diagnostic; stderr:\n{stderr}"
        );
    }
}

/// rubash#744: a here-string inside a quoted whole-word `$(...)` must have its
/// word quote-removed.
mod issue_744 {
    use super::*;

    /// 744.1 — the quoted-`$()` fast path in command_prepare handed the body
    /// straight to the specialized expander, which kept the quotes
    /// (`p "$(cat <<< 'here')"` printed `['here']`). The body now declines that
    /// shortcut when it carries `<<<`, so it reaches the AST path that already
    /// routes here-string substitutions through the real parser.
    #[test]
    fn case_1_here_string_in_quoted_comsub_is_dequoted() {
        let dir = case_scratch("744.1");

        let script = "p() { printf '[%s]' \"$@\"; printf '\\n'; }\n\
                      p \"$(cat <<< 'here')\"\n\
                      p $(cat <<< 'here')\n\
                      p \"pre$(cat <<< 'here')post\"\n\
                      p \"$(cat <<< \"a b\")\"\n";
        std::fs::write(dir.join("here.sh"), script).expect("write script");
        let out = run_rubash_in(&dir, &["here.sh"], Duration::from_secs(30));
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "[here]\n[here]\n[preherepost]\n[a b]\n",
            "here-string quote removal; stderr:\n{stderr}"
        );
    }
}

/// rubash#703: a heredoc feeding a pipeline inside a double-quoted command
/// substitution failed to parse.
///
/// `echo "X=$(cat <<EOF | cat` + `body` + `EOF` + `)"` died with
/// `command substitution: line 4: unexpected EOF while looking for matching
/// `)'` and printed nothing, while GNU prints `X=body`. The unquoted form,
/// the quoted form without the `X=` prefix, and the quoted form without the
/// pipeline all worked, which narrowed the trigger to the word-expansion
/// comsub collector (`collect_command_substitution_source_ex`): its heredoc
/// delimiter scan did not end the delimiter WORD at the first unquoted
/// blank, so `cat <<EOF | cat` declared the delimiter `EOF | cat`, the `EOF`
/// terminator line never matched, and the collector swallowed the rest of
/// the script. GNU read_token_word ends the word at any unquoted shell
/// metacharacter (parse.y:5688 shellbreak) — the same rule the lexer's own
/// heredoc header scan already applies.
mod issue_703 {
    use super::*;

    fn run_script(case: &str, script: &str) -> (String, String, Option<i32>) {
        let dir = case_scratch(case);
        std::fs::write(dir.join("hd.sh"), script).expect("write script");
        let out = run_rubash_in(&dir, &["hd.sh"], Duration::from_secs(30));
        (
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            out.code,
        )
    }

    /// 703.1 [ratchet] — the issue's exact repro. The delimiter word must end
    /// at the blank before `|`, the `EOF` line must terminate the body, and
    /// the `)` on the following line must close the substitution. Pre-fix the
    /// collector read the delimiter as `EOF | cat`, never matched the
    /// terminator, and reported the unclosed-`)` fatal with no output.
    #[test]
    fn case_1_quoted_comsub_heredoc_pipeline() {
        let (stdout, stderr, code) =
            run_script("703.1", "echo \"X=$(cat <<EOF | cat\nbody\nEOF\n)\"\n");
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(stdout, "X=body\n", "stdout; stderr:\n{stderr}");
        assert!(
            !stderr.contains("unexpected EOF"),
            "unclosed-comsub fatal; stderr:\n{stderr}"
        );
    }

    /// 703.2 [control] — the three shapes the issue report named as already
    /// working must not regress: the unquoted assignment, the quoted
    /// substitution without a word prefix, and the quoted substitution
    /// without the pipeline. All three were green before the fix.
    #[test]
    fn case_2_working_shapes_stay_green() {
        let (stdout, stderr, code) = run_script(
            "703.2",
            "x=$(cat <<EOF | cat\nbody\nEOF\n); echo \"X=$x\"\n\
             echo \"$(cat <<EOF | cat\nbody\nEOF\n)\"\n\
             echo \"X=$(cat <<EOF\nbody\nEOF\n)\"\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "X=body\nbody\nX=body\n",
            "stdout; stderr:\n{stderr}"
        );
    }

    /// 703.3 [ratchet] — the make_cmd.c:602-611 pushback shape with a
    /// pipeline: the `)` sits ON the delimiter line (`EOF)`), so the heredoc
    /// is "delimited by end-of-file", GNU warns, and the substitution still
    /// yields `body`. Two defects hid here: the pre-fix delimiter scan (same
    /// fatal as 703.1), and — once that was fixed — the lexer's IFS_GLUE
    /// `EOF)` marker reaching the real comsub parser, where the terminator
    /// line read `EOF\x1c` and stopped matching, leaking the delimiter into
    /// the output (`X=body` plus a stray `EOF`). Exact stdout pins both.
    #[test]
    fn case_3_eof_paren_pushback_with_pipeline() {
        let (stdout, stderr, code) =
            run_script("703.3", "echo \"X=$(cat <<EOF | cat\nbody\nEOF)\"\n");
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "X=body\n",
            "stray delimiter in output; stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("delimited by end-of-file"),
            "missing GNU here-document warning; stderr:\n{stderr}"
        );
    }

    /// 703.4 [ratchet] — delimiter-word spellings that must still resolve
    /// correctly once the word ends at the first unquoted blank: a blank
    /// between `<<` and the word (`<< EOF`), the adjacent `<<-` tab-strip
    /// dash, and a quoted delimiter carrying its own blank. Each carries a
    /// pipeline after the word, so the pre-fix scan — which read the word to
    /// end of line — declared `EOF | cat` (or `-EOF | cat`, `E O | cat`) and
    /// died unclosed on all three.
    #[test]
    fn case_4_delimiter_spellings_with_pipeline() {
        let (stdout, stderr, code) = run_script(
            "703.4",
            "echo \"X=$(cat << EOF | cat\nbody\nEOF\n)\"\n\
             echo \"Y=$(cat <<-EOF | cat\n\tbody\n\tEOF\n)\"\n\
             echo \"Z=$(cat <<\"E O\" | cat\nbody\nE O\n)\"\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "X=body\nY=body\nZ=body\n",
            "stdout; stderr:\n{stderr}"
        );
    }

    /// 703.5 [ratchet] — the same word-boundary rule with a `;` after the
    /// delimiter: `cat <<EOF; cat` declares `EOF` and runs `cat` as the next
    /// command. Pre-fix the delimiter read `EOF; cat` and the terminator
    /// never matched, so the substitution died unclosed.
    #[test]
    fn case_5_semicolon_after_delimiter_word() {
        let (stdout, stderr, code) =
            run_script("703.5", "echo \"X=$(cat <<EOF; cat\nbody\nEOF\n)\"\n");
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(stdout, "X=body\n", "stdout; stderr:\n{stderr}");
        assert!(
            !stderr.contains("unexpected EOF"),
            "unclosed-comsub fatal; stderr:\n{stderr}"
        );
    }
}

/// rubash#745: two layers, not one.
///
/// Inside a backtick body the backslash is consumed only before `$`, a
/// backtick or another backslash; every other character keeps it (parse.y
/// parse_matched_pair under P_BACKQUOTE). A backtick that sits inside a
/// double-quoted word is a separate layer: lexer::quotes consumes `\"` there,
/// so the two upstream cases that look contradictory both hold —
///
///   recho `echo "(\")"`        keeps `\"`  (quote.tests:123)
///   recho "`echo \"x\"`"        sees a bare quote (exp.tests:146)
///   recho `echo 'a\' b`        keeps `\'`  (comsub.tests)
mod issue_745 {
    use super::*;

    fn run_script(case: &str, script: &str) -> (String, String, Option<i32>) {
        let dir = case_scratch(case);
        std::fs::write(dir.join("bt.sh"), script).expect("write script");
        let out = run_rubash_in(&dir, &["bt.sh"], Duration::from_secs(30));
        (
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            out.code,
        )
    }

    /// 745.1 — a word-final backslash run survives. GNU hands the body to
    /// parse_and_execute with EOF immediately after it, so a lone trailing `\`
    /// stays literal; the re-parse here terminates the text with a newline,
    /// where `\` + newline is a line continuation. Also pins the four-backslash
    /// double-quote shape the old decoder mangled (`"\\\\"` -> `\"` instead of
    /// `\`).
    #[test]
    fn case_1_word_final_backslash_and_four_backslash_shape() {
        let script = "q() { printf '[%s]|%s\\n' \"$1\" \"${#1}\"; }\n\
                      b=`printf '%s' \\\\`; q \"$b\"\n\
                      c=`printf '%s' \"\\\\\\\\\"`; q \"$c\"\n";
        let (stdout, stderr, code) = run_script("745.1", script);
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(stdout, "[\\]|1\n[\\]|1\n", "stderr:\n{stderr}");
    }

    /// 745.2 `[guard]` — the escape set is `$`, backtick and `\`; every other
    /// character keeps its backslash. This is the direction guard: the second
    /// attempt at #745 read the rule as "strip the backslash before anything",
    /// which turned `echo 'a\' b` inside backticks into `echo 'a' b` and broke
    /// `comsub.tests`. The last line is the discriminator — green on the
    /// original engine, red on that wrong fix, green here.
    #[test]
    fn case_2_escape_set_and_dquote_layer() {
        let script = "q() { printf '[%s]|%s\\n' \"$1\" \"${#1}\"; }\n\
                      p() { printf '[%s]' \"$@\"; printf '\\n'; }\n\
                      d=`printf '%s' \\a`; q \"$d\"\n\
                      e=`echo a\\\\b`; q \"$e\"\n\
                      f=`printf '%s' '\\\\'`; q \"$f\"\n\
                      g=$(printf '%s' \\\\); q \"$g\"\n\
                      p `echo 'a\\' b`\n";
        let (stdout, stderr, code) = run_script("745.2", script);
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "[a]|1\n[ab]|2\n[\\]|1\n[\\]|1\n[a\\][b]\n",
            "stderr:\n{stderr}"
        );
    }
}

/// rubash#776: `read -N` binds the record to the first name verbatim — no IFS
/// splitting, no whitespace strip — and leaves the rest empty.
mod issue_776 {
    use super::*;

    fn run(case: &str, script: &str) -> (String, String, Option<i32>) {
        let dir = case_scratch(case);
        std::fs::write(dir.join("in.txt"), "AB\nC").expect("write input");
        std::fs::write(dir.join("n.sh"), script).expect("write script");
        let out = run_rubash_in(&dir, &["n.sh"], Duration::from_secs(30));
        (
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            out.code,
        )
    }

    /// 776.1 — the delimiter the limit stopped on is data under `-N`: `-N3` on
    /// `AB\nC` yields `AB\n`, and the second name stays empty instead of taking
    /// a split field.
    #[test]
    fn case_1_capital_n_keeps_the_delimiter_and_skips_splitting() {
        let script = "read -N3 c < in.txt; printf '<%s>|%s\\n' \"$c\" \"${#c}\"\n\
                      read -N3 a b < in.txt; printf '[%s][%s]\\n' \"$a\" \"$b\"\n\
                      printf 'AB CD' > in2.txt\n\
                      read -N5 x y < in2.txt; printf '[%s][%s]\\n' \"$x\" \"$y\"\n";
        let (stdout, stderr, code) = run("776.1", script);
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "<AB\n>|3\n[AB\n][]\n[AB CD][]\n",
            "stderr:\n{stderr}"
        );
    }

    /// 776.2 — leading/trailing blanks are preserved, and `-n` keeps the
    /// ordinary split/strip behaviour as the control.
    #[test]
    fn case_2_capital_n_keeps_whitespace_lowercase_n_still_strips() {
        let script = "printf '  AB  ' > in3.txt\n\
                      read -N6 s < in3.txt; printf '[%s]\\n' \"$s\"\n\
                      read -n6 t < in3.txt; printf '[%s]\\n' \"$t\"\n";
        let (stdout, stderr, code) = run("776.2", script);
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(stdout, "[  AB  ]\n[AB]\n", "stderr:\n{stderr}");
    }
}

/// rubash#721: `< /dev/fd/N` (or `/proc/self/fd/N`) with N closed is an open
/// failure, not an empty input. GNU opens the alias through the kernel and
/// reports ENOENT, abandoning the command with status 1.
mod issue_721 {
    use super::*;

    fn run(case: &str, code: &str) -> (String, String, Option<i32>) {
        let dir = case_scratch(case);
        std::fs::write(dir.join("live.txt"), "hi\n").expect("write input");
        let out = run_rubash_in(&dir, &["-c", code], Duration::from_secs(30));
        (
            String::from_utf8_lossy(&out.stdout).trim_end().to_string(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            out.code,
        )
    }

    /// 721.1 — the read builtin's alias branch staged an empty mirror, which
    /// read saw as plain EOF: bare rc 1 and no diagnostic.
    #[test]
    fn case_1_read_builtin_reports_closed_alias() {
        let (stdout, stderr, code) = run("721.1", "exec 6<&-\nread -r z < /dev/fd/6\necho rc=$?");
        assert_eq!(code, Some(0), "script rc; stderr:\n{stderr}");
        assert_eq!(stdout, "rc=1", "read rc; stderr:\n{stderr}");
        assert!(
            stderr.contains("/dev/fd/6") && stderr.contains("No such file or directory"),
            "read must report the alias; stderr:\n{stderr}"
        );
    }

    /// 721.2 — the compound fd-0 preflight treated every alias as available,
    /// so `while read ... done < /dev/fd/6` ran zero iterations and reported 0.
    #[test]
    fn case_2_compound_loop_reports_closed_alias() {
        let (stdout, stderr, code) = run(
            "721.2",
            "exec 6<&-\nwhile read -r l; do :; done < /dev/fd/6\necho rc=$?",
        );
        assert_eq!(code, Some(0), "script rc; stderr:\n{stderr}");
        assert_eq!(stdout, "rc=1", "loop rc; stderr:\n{stderr}");
        assert!(
            stderr.contains("/dev/fd/6") && stderr.contains("No such file or directory"),
            "the loop must report the alias; stderr:\n{stderr}"
        );
    }

    /// 721.3 — the /proc spelling takes the same path.
    #[test]
    fn case_3_proc_self_fd_spelling_reports_closed_alias() {
        let (stdout, stderr, code) = run(
            "721.3",
            "exec 6<&-\nread -r z < /proc/self/fd/6\necho rc=$?",
        );
        assert_eq!(code, Some(0), "script rc; stderr:\n{stderr}");
        assert_eq!(stdout, "rc=1", "proc rc; stderr:\n{stderr}");
        assert!(
            stderr.contains("/proc/self/fd/6") && stderr.contains("No such file or directory"),
            "the /proc operand must be reported; stderr:\n{stderr}"
        );
    }

    /// 721.4 `[control]` — a live descriptor still serves its bytes: the check
    /// is "is N open", not "is the word an alias". Green on the pre-fix engine
    /// by design; it exists to catch an over-fix that rejects every alias.
    #[test]
    fn case_4_live_descriptor_still_reads() {
        let (stdout, stderr, code) = run(
            "721.4",
            "exec 6< live.txt\nread -r z < /dev/fd/6\necho \"got=$z rc=$?\"",
        );
        assert_eq!(code, Some(0), "script rc; stderr:\n{stderr}");
        assert_eq!(stdout, "got=hi rc=0", "live fd; stderr:\n{stderr}");
        assert!(stderr.is_empty(), "live fd must stay silent: {stderr:?}");
    }
}

/// rubash#704 R6.2: `command -v`/`-V` use GNU's ANY-found rule
/// (builtins/command.def increments any_found per resolved operand).
mod issue_704 {
    use super::*;

    /// 704.2 — one miss among several operands still exits 0. `type` is the
    /// all-found builtin and is deliberately the opposite — the two shared an
    /// accumulator that made `command -v echo nosuch` return 1.
    #[test]
    fn case_2_command_v_multi_operand_is_any_found() {
        let dir = case_scratch("704.2");

        let code = "command -v echo nosuch_xyz >/dev/null; echo v:$?\n\
                    command -v nosuch_xyz echo >/dev/null; echo v2:$?\n\
                    command -V echo nosuch_xyz >/dev/null 2>&1; echo V:$?\n\
                    command -v nosuch1 nosuch2 >/dev/null; echo miss:$?\n\
                    type echo nosuch_xyz >/dev/null 2>&1; echo type:$?";
        let out = run_rubash_in(&dir, &["-c", code], Duration::from_secs(30));
        let stdout = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        assert_eq!(
            stdout, "v:0\nv2:0\nV:0\nmiss:1\ntype:1",
            "command describe status; stderr:\n{stderr}"
        );
    }

    /// 704.2b — builtins/command.def:95-96 `if (list == 0) return
    /// (EXECUTION_SUCCESS);`: the no-operand early return comes BEFORE the
    /// describe loop, so `command -v`/`-V` with no names succeeds with no
    /// output instead of falling through to the any_found accumulator's
    /// failure. Same rule inside a substitution (the status rides
    /// last_command_substitution_status).
    #[test]
    fn case_2b_command_v_without_operands_succeeds() {
        let dir = case_scratch("704.2b");

        let code = "command -v; echo direct:$?\n\
                    command -V; echo verbose:$?\n\
                    v=$(command -v); echo \"comsub:[${v}] rc:$?\"\n\
                    v=$(command -V); echo \"comsubV:[${v}] rc:$?\"";
        let out = run_rubash_in(&dir, &["-c", code], Duration::from_secs(30));
        let stdout = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        assert_eq!(
            stdout, "direct:0\nverbose:0\ncomsub:[] rc:0\ncomsubV:[] rc:0",
            "no-operand describe status; stderr:\n{stderr}"
        );
        assert!(
            stderr.is_empty(),
            "no-operand forms print nothing on stderr; got:\n{stderr}"
        );
    }

    /// 704.2c — the substitution entry point shares the any-found rule, and
    /// the `-V` per-operand "not found" diagnostic still reaches stderr
    /// while the found operand's description is captured (WSL GNU bash
    /// 5.3.0 probe: `v=$(command -V echo nosuch_xyz)` -> stdout captures
    /// "echo is a shell builtin", stderr gets
    /// "command: nosuch_xyz: not found", rc 0).
    #[test]
    fn case_2c_command_v_in_substitution_uses_any_found() {
        let dir = case_scratch("704.2c");

        let code = "v=$(command -v echo nosuch_xyz); echo \"any:[$v] rc:$?\"\n\
                    v=$(command -v nosuch_one nosuch_two); echo \"miss:[$v] rc:$?\"\n\
                    v=$(command -V echo nosuch_xyz); echo \"verbose:[$v] rc:$?\"";
        let out = run_rubash_in(&dir, &["-c", code], Duration::from_secs(30));
        let stdout = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        assert_eq!(
            stdout, "any:[echo] rc:0\nmiss:[] rc:1\nverbose:[echo is a shell builtin] rc:0",
            "substituted describe status; stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("command: nosuch_xyz: not found"),
            "-V not-found diagnostic must reach stderr; stderr:\n{stderr}"
        );
        assert!(
            !stderr.contains("nosuch_one: not found"),
            "-v (reusable) stays silent about misses; stderr:\n{stderr}"
        );
    }
}

/// rubash#759: `dirs -l` is the LONG form (`$HOME` stays un-collapsed), not the
/// one-per-line form. Only `-p` (and `-v`, already numbered) select one entry
/// per line.
mod issue_759 {
    use super::*;

    /// 759.2 — rubash treated `-l` as `-p`, so `dirs -l` printed one line per
    /// entry where GNU prints one line.
    #[test]
    fn case_2_dirs_long_form_keeps_the_space_separator() {
        let dir = case_scratch("759.2");
        std::fs::create_dir_all(dir.join("a")).expect("create a");
        std::fs::create_dir_all(dir.join("b")).expect("create b");

        let run = |code: &str| {
            let out = run_rubash_in(&dir, &["-c", code], Duration::from_secs(30));
            String::from_utf8_lossy(&out.stdout).trim_end().to_string()
        };

        let long = run("cd a\npushd ../b >/dev/null\ndirs -l");
        assert_eq!(
            long.lines().count(),
            1,
            "`dirs -l` must stay on one line: {long:?}"
        );
        assert!(
            long.contains(' '),
            "`dirs -l` separates with spaces: {long:?}"
        );

        let per_line = run("cd a\npushd ../b >/dev/null\ndirs -p");
        assert_eq!(
            per_line.lines().count(),
            2,
            "`dirs -p` is one entry per line: {per_line:?}"
        );

        let numbered = run("cd a\npushd ../b >/dev/null\ndirs -v");
        assert_eq!(
            numbered.lines().count(),
            2,
            "`dirs -v` is numbered, one per line: {numbered:?}"
        );
        let long_numbered = run("cd a\npushd ../b >/dev/null\ndirs -l -v");
        assert_eq!(
            long_numbered, numbered,
            "`dirs -l -v` renders like `dirs -v`"
        );
    }
}

/// rubash#742: three word-level quote-removal arms in lexer/quotes.rs.
mod issue_742 {
    use super::*;

    fn run(case: &str, script: &str) -> (String, String, Option<i32>) {
        let dir = case_scratch(case);
        std::fs::write(dir.join("q.sh"), script).expect("write script");
        let out = run_rubash_in(&dir, &["q.sh"], Duration::from_secs(30));
        (
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            out.code,
        )
    }

    /// 742.1 — an unbraced `$name` ends at the next backslash. The old scan
    /// cleared `pending_name` without emitting PARAM_NAME_END_MARKER, so the
    /// neighbours glued into one longer name and another variable's value was
    /// returned (`$v\tb` with vtb=QQ printed QQ). The braced form is the
    /// control that pinned it to the unbraced scan.
    #[test]
    fn case_1_unbraced_name_ends_at_backslash() {
        let script = "p() { printf '[%s]' \"$@\"; printf '\\n'; }\n\
                      v=AB; vtb=QQ\n\
                      p $v\\tb\n\
                      q=$v\\tb; p \"$q\"\n\
                      p ${v}\\tb\n";
        let (stdout, stderr, code) = run("742.1", script);
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(stdout, "[ABtb]\n[ABtb]\n[ABtb]\n", "stderr:\n{stderr}");
    }

    /// 742.2 — an unbraced `$name` also ends at the next `$`, so a following
    /// `$'...'` is a new expansion instead of more name bytes.
    #[test]
    fn case_2_unbraced_name_ends_at_dollar() {
        let script = "p() { printf '[%s]' \"$@\"; printf '\\n'; }\n\
                      w='a b'; wb=QQ\n\
                      p a$w$'b'\n\
                      p $w$'b'\n";
        let (stdout, stderr, code) = run("742.2", script);
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(stdout, "[aa][bb]\n[a][bb]\n", "stderr:\n{stderr}");
    }

    /// 742.3 — an escaped blank is quoted data. The generic ASCII-escape arm
    /// pushed the byte bare, so field splitting saw an ordinary space and one
    /// argument became two.
    #[test]
    fn case_3_escaped_blank_stays_one_argument() {
        let script = "p() { printf '[%s]' \"$@\"; printf '\\n'; }\n\
                      v=AB\n\
                      p $v\\ b\n\
                      p ${v}\\ b\n";
        let (stdout, stderr, code) = run("742.3", script);
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(stdout, "[AB b]\n[AB b]\n", "stderr:\n{stderr}");
    }
}

/// rubash#753: after a failglob expansion error GNU abandons the rest of the
/// current logical line; rubash only abandoned the failing command when that
/// command was a compound, so one extra statement ran after the error.
///
/// The issue's repro is the brace group (`case2`): GNU never reaches the
/// `echo` after the `;`, rubash ran it and reported `case2-rc=1`. The
/// subshell (`case1`) and the simple command (`case3`) already matched,
/// which pinned the divergence to the compound-command boundary swallowing
/// the expansion failure instead of letting it unwind to the reader. This
/// is control flow, distinct from #726's diagnostic-bypass defect.
///
/// GNU's rule, as pinned by the probe family behind these cases: the
/// failglob failure abandons the remainder of the current logical line —
/// the same discard the arithmetic/subscript evalerror path already
/// implements (`evalerror_pending` + the reader loop-top check). The
/// discard is CONTAINED by exactly these boundaries: `( )` subshells,
/// pipeline stages, `$( )`, `&` background jobs, `eval`, and `source`; it
/// PENETRATES brace groups, if/then bodies, if conditions,
/// while/until/for/select bodies, case arms, function bodies, `!`, and
/// `time`. A loop body's discard escapes through `done` to the enclosing
/// logical line and aborts the loop; a `for` word-list failure is a plain
/// line abort.
///
/// Fix: a new class flag `evalerror_failglob_pending` (mirroring
/// `evalerror_subscript_pending`'s clear-at-every-boundary discipline) is
/// armed at every failglob `Fail` site alongside `raise_evalerror_abort()`,
/// so the reader-layer loop-top check discards the rest of the line. The
/// eval/source boundaries absorb exactly this class (GNU evalstring.c
/// catches the expansion error locally at the top level of eval/source),
/// which is why the containing boundaries keep their old behavior.
///
/// Two pre-existing divergences found while probing, deliberately out of
/// scope here (both reproduce on the pre-fix binary, so neither is a
/// regression of this change; both are follow-up issue candidates):
/// - `c=([k]=v [""]=x)` — a bad-subscript compound assignment binds
///   partially and GNU CONTINUES the line (`sub-after rc=0`); rubash
///   discards the rest of the line (subscript family).
/// - `${arr[/no/such/*.zzz]}` — the subscript failglob diagnostic reads
///   `arithmetic syntax error` where GNU reads `syntax error` (diagnostic
///   wording family).
mod issue_753 {
    use super::*;

    fn run(case: &str, script: &str) -> (String, String, Option<i32>) {
        let dir = case_scratch(case);
        std::fs::write(dir.join("fg.sh"), script).expect("write script");
        let out = run_rubash_in(&dir, &["fg.sh"], Duration::from_secs(30));
        (
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            out.code,
        )
    }

    /// Same contract as `run`, with a stdin payload. The select shape needs
    /// one input line for its body to execute at all: with
    /// `Stdio::null()` the select EOFs before the first prompt and the body
    /// never runs, which is exactly how the first select probe misread the
    /// boundary (the body "discarding" the line was really the body never
    /// executing).
    fn run_with_stdin(case: &str, script: &str, input: &str) -> (String, String, Option<i32>) {
        let dir = case_scratch(case);
        std::fs::write(dir.join("fg.sh"), script).expect("write script");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rubash"));
        cmd.args(["fg.sh"])
            .current_dir(&dir)
            .env_clear()
            .env("PATH", minimal_path())
            .env("HOME", &dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if cfg!(windows) {
            for name in [
                "SystemRoot",
                "SystemDrive",
                "WINDIR",
                "COMSPEC",
                "PATHEXT",
                "OS",
                "TEMP",
                "TMP",
            ] {
                if let Ok(value) = std::env::var(name) {
                    cmd.env(name, value);
                }
            }
        }
        let mut child = cmd.spawn().expect("spawn rubash");
        std::io::Write::write_all(child.stdin.as_mut().expect("stdin pipe"), input.as_bytes())
            .expect("write stdin");
        drop(child.stdin.take());
        let out = wait_bounded(&mut child, Duration::from_secs(30));
        (
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            out.code,
        )
    }

    /// 753.1 [ratchet] — the issue's exact repro. case1 (subshell) and
    /// case3 (simple command) were already GNU-correct and stay pinned
    /// here; case2 (brace group) is the ratchet: pre-fix rubash printed
    /// `case2-rc=1`, GNU abandons the rest of that logical line and never
    /// reaches the `echo`. stderr pins all three diagnostics with their
    /// line numbers, proving the failures fired rather than the echoes
    /// silently vanishing.
    #[test]
    fn case_1_issue_repro_brace_group_same_line() {
        let (stdout, stderr, code) = run(
            "753.1",
            "shopt -s failglob\n\
             echo \"case1-start\"\n\
             ( set -- /no/such/*.zzz; echo \"case1-inside\" ); echo \"case1-rc=$?\"\n\
             echo \"case2-start\"\n\
             { set -- /no/such/*.zzz; echo \"case2-inside\"; }; echo \"case2-rc=$?\"\n\
             echo \"case3-start\"\n\
             set -- /no/such/*.zzz; echo \"case3-rc=$?\"\n\
             echo \"END\"\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "case1-start\ncase1-rc=1\ncase2-start\ncase3-start\nEND\n",
            "case2-rc must not print; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr,
            "fg.sh: line 3: no match: /no/such/*.zzz\n\
             fg.sh: line 5: no match: /no/such/*.zzz\n\
             fg.sh: line 7: no match: /no/such/*.zzz\n",
            "diagnostics; stdout:\n{stdout}"
        );
    }

    /// 753.2 [ratchet] — a brace group in list position. Pre-fix rubash ran
    /// `A_after_brace_semicolon` (after `;`), `C_or_arm` (the `||` arm —
    /// the group "failed", so the arm was eligible), and `D_after_invert
    /// rc=0` (`!` inverted the aborted group's status); GNU abandons the
    /// line at each failure and prints only `END`. `B_and_arm` is pinned
    /// too but is non-discriminating: the `&&` arm is skipped by status in
    /// both shells, so its absence proves nothing about the discard — it
    /// is here to keep the and/or pair covered as a family.
    #[test]
    fn case_2_brace_group_in_list_position() {
        let (stdout, stderr, code) = run(
            "753.2",
            "shopt -s failglob\n\
             { set -- /no/such/*.zzz; }; echo \"A_after_brace_semicolon\"\n\
             { set -- /no/such/*.zzz; } && echo \"B_and_arm\"\n\
             { set -- /no/such/*.zzz; } || echo \"C_or_arm\"\n\
             ! { set -- /no/such/*.zzz; }; echo \"D_after_invert rc=$?\"\n\
             echo END\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "END\n",
            "list-position arm echoes must not run; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr,
            "fg.sh: line 2: no match: /no/such/*.zzz\n\
             fg.sh: line 3: no match: /no/such/*.zzz\n\
             fg.sh: line 4: no match: /no/such/*.zzz\n\
             fg.sh: line 5: no match: /no/such/*.zzz\n",
            "diagnostics; stdout:\n{stdout}"
        );
    }

    /// 753.3 [ratchet] — the compound bodies that PENETRATE: an if/then
    /// body and the if condition itself, a case arm, and a nested brace
    /// group. Pre-fix rubash printed all five trailing echoes
    /// (`E_after_fi_sameline`, `IFCOND_after`, `G_after_case_sameline`,
    /// `L_nested_inner`, `M_after_nested`); GNU abandons each logical line
    /// at the failure, so only `END` survives. The if-condition shape is
    /// the one the probe family found last: the condition's failglob
    /// aborts the whole `if ... fi` line, not just the condition command.
    #[test]
    fn case_3_if_body_if_condition_case_nested_brace() {
        let (stdout, stderr, code) = run(
            "753.3",
            "shopt -s failglob\n\
             if true; then set -- /no/such/*.zzz; fi; echo \"E_after_fi_sameline\"\n\
             if set -- /no/such/*.zzz; then echo \"cond-T\"; fi; echo \"IFCOND_after\"\n\
             case x in x) set -- /no/such/*.zzz;; esac; echo \"G_after_case_sameline\"\n\
             { { set -- /no/such/*.zzz; }; echo \"L_nested_inner\"; }; echo \"M_after_nested\"\n\
             echo END\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "END\n",
            "penetrating compound bodies; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr,
            "fg.sh: line 2: no match: /no/such/*.zzz\n\
             fg.sh: line 3: no match: /no/such/*.zzz\n\
             fg.sh: line 4: no match: /no/such/*.zzz\n\
             fg.sh: line 5: no match: /no/such/*.zzz\n",
            "diagnostics; stdout:\n{stdout}"
        );
    }

    /// 753.4 [ratchet] — the `for` word list. Bare (`FOR_after`) was
    /// already GNU-correct pre-fix; the in-brace spelling
    /// (`FOR_brace_after`) was not: pre-fix rubash ran the echo after the
    /// brace group closed. GNU abandons the line in both spellings.
    #[test]
    fn case_4_for_word_list_failglob() {
        let (stdout, stderr, code) = run(
            "753.4",
            "shopt -s failglob\n\
             for i in /no/such/*.zzz; do echo \"for-body\"; done; echo \"FOR_after\"\n\
             { for i in /no/such/*.zzz; do echo \"for-body2\"; done; }; echo \"FOR_brace_after\"\n\
             echo END\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "END\n",
            "for word-list abort scope; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr,
            "fg.sh: line 2: no match: /no/such/*.zzz\n\
             fg.sh: line 3: no match: /no/such/*.zzz\n",
            "diagnostics; stdout:\n{stdout}"
        );
    }

    /// 753.5 [ratchet] — the `time` keyword. Pre-fix rubash printed
    /// `I_after_time`; GNU abandons the rest of the line after the timed
    /// group's failglob. stderr is only contains-checked: the `real`/
    /// `user`/`sys` timing lines are machine-dependent bytes and belong to
    /// no golden.
    #[test]
    fn case_5_time_keyword_penetrates() {
        let (stdout, stderr, code) = run(
            "753.5",
            "shopt -s failglob\n\
             time { set -- /no/such/*.zzz; }; echo \"I_after_time\"\n\
             echo END\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "END\n",
            "time must not contain the discard; stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("fg.sh: line 2: no match: /no/such/*.zzz"),
            "diagnostic missing; stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("real") && stderr.contains("user") && stderr.contains("sys"),
            "time report missing; stderr:\n{stderr}"
        );
    }

    /// 753.6 [ratchet] — a function definition, call, and trailing echo on
    /// one logical line. Pre-fix rubash printed `after-func-sameline` (the
    /// body's failglob only aborted the function call); GNU abandons the
    /// line. The next-line spelling is covered by 753.15 and stays green.
    #[test]
    fn case_6_function_def_call_echo_same_line() {
        let (stdout, stderr, code) = run(
            "753.6",
            "shopt -s failglob\n\
             g() { set -- /no/such/*.zzz; echo \"func-inside\"; }; g; echo \"after-func-sameline\"\n\
             echo END\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "END\n",
            "same-line echo after a failing function call must not run; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr, "fg.sh: line 2: no match: /no/such/*.zzz\n",
            "diagnostics; stdout:\n{stdout}"
        );
    }

    /// 753.7 [ratchet] — failglob in a redirection word. Bare
    /// (`REDIR_after`) was already correct pre-fix; the brace-internal
    /// spellings (`BR_after`, `BR_brace_after`) were not. GNU abandons the
    /// line in all three spellings, so no redirect file is ever attempted.
    #[test]
    fn case_7_redirection_word_failglob() {
        let (stdout, stderr, code) = run(
            "753.7",
            "shopt -s failglob\n\
             echo x > /no/such/*.zzz; echo \"REDIR_after\"\n\
             { echo x > /no/such/*.zzz; }; echo \"BR_after\"\n\
             { { echo x > /no/such/*.zzz; }; }; echo \"BR_brace_after\"\n\
             echo END\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "END\n",
            "redirect failglob abort scope; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr,
            "fg.sh: line 2: no match: /no/such/*.zzz\n\
             fg.sh: line 3: no match: /no/such/*.zzz\n\
             fg.sh: line 4: no match: /no/such/*.zzz\n",
            "diagnostics; stdout:\n{stdout}"
        );
    }

    /// 753.8 [ratchet] — failglob in a compound-assignment word. Bare
    /// (`CA_after`) was already correct pre-fix; the brace, if-body,
    /// case-arm, and same-line function spellings were not. GNU abandons
    /// each logical line at the failure.
    #[test]
    fn case_8_compound_assignment_word_failglob() {
        let (stdout, stderr, code) = run(
            "753.8",
            "shopt -s failglob\n\
             a=(/no/such/*.zzz); echo \"CA_after\"\n\
             { a=(/no/such/*.zzz); echo \"in-brace\"; }; echo \"CA_brace_after\"\n\
             if true; then a=(/no/such/*.zzz); fi; echo \"CA_if_after\"\n\
             case x in x) a=(/no/such/*.zzz);; esac; echo \"CA_case_after\"\n\
             f() { a=(/no/such/*.zzz); echo \"in-f\"; }; f; echo \"CA_func_after\"\n\
             echo END\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "END\n",
            "compound-assignment failglob abort scope; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr,
            "fg.sh: line 2: no match: /no/such/*.zzz\n\
             fg.sh: line 3: no match: /no/such/*.zzz\n\
             fg.sh: line 4: no match: /no/such/*.zzz\n\
             fg.sh: line 5: no match: /no/such/*.zzz\n\
             fg.sh: line 6: no match: /no/such/*.zzz\n",
            "diagnostics; stdout:\n{stdout}"
        );
    }

    /// 753.9 [ratchet] — `declare` with an indexed compound operand. GNU
    /// fails the whole operand, binds NOTHING (`declare -p b` reports
    /// `declare: b: not found`), and abandons the line, so neither
    /// `rc=`/`rc2=` echo runs. Pre-fix rubash bound `b=([0]="x")`, bound
    /// `c=()`, and ran both echoes. This is the shape that made the
    /// fix's declare path need a hard failure rather than the partial
    /// binding the bad-subscript class uses.
    #[test]
    fn case_9_declare_indexed_operand_binds_nothing() {
        let (stdout, stderr, code) = run(
            "753.9",
            "shopt -s failglob\n\
             declare b=(x /no/such/*.zzz y); echo \"rc=$?\"\n\
             declare -p b\n\
             declare -a c=(/no/such/*.zzz); echo \"rc2=$?\"\n\
             declare -p c\n\
             echo END\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "END\n",
            "declare operand failure must bind nothing and abandon the line; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr,
            "fg.sh: line 2: no match: /no/such/*.zzz\n\
             fg.sh: line 3: declare: b: not found\n\
             fg.sh: line 4: no match: /no/such/*.zzz\n\
             fg.sh: line 5: declare: c: not found\n",
            "diagnostics; stdout:\n{stdout}"
        );
    }

    /// 753.10 [guard] — `declare -A` (associative) is EXEMPT from failglob
    /// in the compound-operand path: GNU stores the literal key
    /// `["/no/such/*.zzz"]=""` with rc 0 and no diagnostic. The fix must
    /// not over-apply the discard to this shape; this case was green
    /// pre-fix and pins the limit of the new arming.
    #[test]
    fn case_10_declare_associative_is_exempt() {
        let (stdout, stderr, code) = run(
            "753.10",
            "shopt -s failglob\n\
             declare -A d=(/no/such/*.zzz); echo \"rc3=$?\"\n\
             declare -p d\n\
             echo END\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "rc3=0\ndeclare -A d=([\"/no/such/*.zzz\"]=\"\" )\nEND\n",
            "assoc operand must store the literal key; stderr:\n{stderr}"
        );
        assert_eq!(stderr, "", "assoc path must stay silent; stdout:\n{stdout}");
    }

    /// 753.11 [control] — the CONTAINING boundaries, all green pre-fix and
    /// pinned so the new discard cannot leak past them: `( )` subshell
    /// (line continues, rc=1), pipeline stage (line continues, pipeline
    /// rc=0 from `cat`), `&` background job (line continues, rc=0 from the
    /// `&` itself), `eval` (the discard applies INSIDE the eval string —
    /// `eval-inside` never prints — but the caller's line continues), a
    /// bare `$( )` (substitution yields empty, line continues), and a
    /// command-prefix assignment `v=$(...)` (line continues with v empty).
    #[test]
    fn case_11_containing_boundaries_stay_contained() {
        let (stdout, stderr, code) = run(
            "753.11",
            "shopt -s failglob\n\
             ( set -- /no/such/*.zzz ); echo \"SUB_after_subshell rc=$?\"\n\
             set -- /no/such/*.zzz | cat; echo \"PIPE_after rc=$?\"\n\
             set -- /no/such/*.zzz & echo \"BG_after rc=$?\"\n\
             wait\n\
             eval 'set -- /no/such/*.zzz; echo eval-inside'; echo \"EVAL_after\"\n\
             eval 'set -- /no/such/*.zzz'; echo \"EVAL2_after\"\n\
             echo \"$(set -- /no/such/*.zzz; echo sub-in)\"; echo \"COMSUB_after\"\n\
             v=$(set -- /no/such/*.zzz); echo \"ASSIGN_after v=$v\"\n\
             echo END\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout,
            "SUB_after_subshell rc=1\n\
             PIPE_after rc=0\n\
             BG_after rc=0\n\
             EVAL_after\n\
             EVAL2_after\n\
             \n\
             COMSUB_after\n\
             ASSIGN_after v=\n\
             END\n",
            "containing boundaries; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr,
            "fg.sh: line 2: no match: /no/such/*.zzz\n\
             fg.sh: line 3: no match: /no/such/*.zzz\n\
             fg.sh: line 4: no match: /no/such/*.zzz\n\
             fg.sh: line 6: no match: /no/such/*.zzz\n\
             fg.sh: line 7: no match: /no/such/*.zzz\n\
             fg.sh: line 8: no match: /no/such/*.zzz\n\
             fg.sh: line 9: no match: /no/such/*.zzz\n",
            "diagnostics; stdout:\n{stdout}"
        );
    }

    /// 753.12 [control] — loop bodies PENETRATE: the failglob inside a
    /// while/until/for body discards the rest of the body's logical line,
    /// escapes through `done`, and aborts the loop (one diagnostic per
    /// loop, not one per remaining iteration). The next-line echoes
    /// (`W_next`, `U_next`, `F_next`) prove the discard stopped at the
    /// line boundary. Green pre-fix; pinned so the reader-layer change
    /// cannot swallow the following line.
    #[test]
    fn case_12_loop_bodies_penetrate_and_abort() {
        let (stdout, stderr, code) = run(
            "753.12",
            "shopt -s failglob\n\
             i=0; while [ $i -lt 3 ]; do i=$((i+1)); set -- /no/such/*.zzz; echo \"wbody-$i\"; done; echo \"W_same\"\n\
             echo \"W_next\"\n\
             i=0; until [ $i -ge 3 ]; do i=$((i+1)); set -- /no/such/*.zzz; echo \"ubody-$i\"; done; echo \"U_same\"\n\
             echo \"U_next\"\n\
             for i in 1 2; do set -- /no/such/*.zzz; echo \"fbody-$i\"; done; echo \"F_same\"\n\
             echo \"F_next\"\n\
             echo END\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "W_next\nU_next\nF_next\nEND\n",
            "loop-body discard must stop at the line boundary; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr,
            "fg.sh: line 2: no match: /no/such/*.zzz\n\
             fg.sh: line 4: no match: /no/such/*.zzz\n\
             fg.sh: line 6: no match: /no/such/*.zzz\n",
            "one diagnostic per aborted loop; stdout:\n{stdout}"
        );
    }

    /// 753.13 [guard] — a temporary assignment prefix
    /// (`x=/no/such/*.zzz true`) is COMMAND-level containment, not a line
    /// abort: the command does not run, status is 1, GNU prints no
    /// diagnostic at all, and the list continues — including inside a
    /// brace group (`in-brace2` and `TA_brace_after` both print). The fix
    /// must not upgrade this containment to a line discard; green pre-fix,
    /// pinned as the guardrail.
    #[test]
    fn case_13_temporary_assignment_prefix_is_command_level() {
        let (stdout, stderr, code) = run(
            "753.13",
            "shopt -s failglob\n\
             x=/no/such/*.zzz true; echo \"TA_after\"\n\
             { x=/no/such/*.zzz true; echo \"in-brace2\"; }; echo \"TA_brace_after\"\n\
             echo END\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "TA_after\nin-brace2\nTA_brace_after\nEND\n",
            "temp-assignment containment must stay command-level; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr, "",
            "temp-assignment failglob is silent in GNU; stdout:\n{stdout}"
        );
    }

    /// 753.14 [control] — `set -e` still exits at the first failglob with
    /// status 1 and no further output. The errexit exit is decided at the
    /// reader-layer post-command check before the new discard consumption
    /// is reached, so this shape is unchanged by the fix; pinned so a
    /// future reader-layer change cannot silently reorder the two.
    #[test]
    fn case_14_errexit_exits_before_discard() {
        let (stdout, stderr, code) = run(
            "753.14",
            "set -e\n\
             shopt -s failglob\n\
             f() { set -- /no/such/*.zzz; echo in-f; }\n\
             f\n\
             echo \"F-next\"\n\
             echo END\n",
        );
        assert_eq!(code, Some(1), "errexit status; stderr:\n{stderr}");
        assert_eq!(
            stdout, "",
            "errexit must exit before any echo; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr, "fg.sh: line 3: no match: /no/such/*.zzz\n",
            "diagnostics; stdout:\n{stdout}"
        );
    }

    /// 753.15 [control] — the discard is a LINE boundary, not a file
    /// boundary: every command on the following line still runs
    /// (`after-nextline`, `after-if-nextline`, `after-while`,
    /// `after-func-nextline`), and the same-line `echo` after each failing
    /// compound does not. Green pre-fix for the next-line echoes; the
    /// same-line absences were pinned by 753.1-753.8. Together the pair
    /// proves the reader skipped exactly the remainder of one logical
    /// line.
    #[test]
    fn case_15_next_line_commands_still_run() {
        let (stdout, stderr, code) = run(
            "753.15",
            "shopt -s failglob\n\
             { set -- /no/such/*.zzz; echo \"inside\"; }\n\
             echo \"after-nextline\"\n\
             if true; then set -- /no/such/*.zzz; echo \"then-inside\"; fi\n\
             echo \"after-if-nextline\"\n\
             while false; do set -- /no/such/*.zzz; done\n\
             echo \"after-while\"\n\
             h() { set -- /no/such/*.zzz; echo \"func-inside\"; }\n\
             h\n\
             echo \"after-func-nextline\"\n\
             echo END\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "after-nextline\nafter-if-nextline\nafter-while\nafter-func-nextline\nEND\n",
            "next-line commands must survive the discard; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr,
            "fg.sh: line 2: no match: /no/such/*.zzz\n\
             fg.sh: line 4: no match: /no/such/*.zzz\n\
             fg.sh: line 8: no match: /no/such/*.zzz\n",
            "diagnostics; stdout:\n{stdout}"
        );
    }

    /// 753.16 [control] — the select body PENETRATES like the other loop
    /// bodies. Needs a stdin payload: with EOF-on-open stdin the select
    /// menu EOFs before the first prompt and the body never executes,
    /// which is how the first probe misread select as a containing
    /// boundary. With one input line the menu prints (`1) a` / `2) b` /
    /// `#? `), the body's failglob discards the rest of the line
    /// (`select-body` and `SEL_same` never print), and the next line runs.
    /// Green pre-fix; pinned with the other loop bodies.
    #[test]
    fn case_16_select_body_penetrates() {
        let (stdout, stderr, code) = run_with_stdin(
            "753.16",
            "shopt -s failglob\n\
             select v in a b; do set -- /no/such/*.zzz; echo \"select-body\"; break; done; echo \"SEL_same\"\n\
             echo \"SEL_next\"\n",
            "pick\n",
        );
        assert_eq!(code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "SEL_next\n",
            "select body must not contain the discard; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr, "1) a\n2) b\n#? fg.sh: line 2: no match: /no/such/*.zzz\n",
            "menu plus diagnostic; stdout:\n{stdout}"
        );
    }

    /// 753.17 [control] — `source` is a containing boundary at the FILE
    /// seam: the failglob inside the sourced file discards the rest of
    /// that file's logical line (`src-inside` never prints) but the next
    /// line of the sourced file still runs (`src-after`), and the caller's
    /// line — the very `source ...; echo caller-same` that triggered it —
    /// continues (`caller-same`, `caller-next`). This is the GNU
    /// evalstring.c top-level catch the fix mirrors by absorbing the
    /// failglob class at the source boundary. Green pre-fix; pinned so the
    /// reader-layer change cannot make source abort its caller's line.
    #[test]
    fn case_17_source_boundary_contains() {
        let dir = case_scratch("753.17");
        std::fs::write(
            dir.join("fg.sh"),
            "shopt -s failglob\n\
             source inner753.sh; echo \"caller-same\"\n\
             echo \"caller-next\"\n\
             echo END\n",
        )
        .expect("write script");
        std::fs::write(
            dir.join("inner753.sh"),
            "shopt -s failglob\n\
             set -- /no/such/*.zzz; echo \"src-inside\"\n\
             echo \"src-after\"\n",
        )
        .expect("write inner script");
        let out = run_rubash_in(&dir, &["fg.sh"], Duration::from_secs(30));
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.code, Some(0), "rc; stderr:\n{stderr}");
        assert_eq!(
            stdout, "src-after\ncaller-same\ncaller-next\nEND\n",
            "source must contain the discard at the file seam; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr, "inner753.sh: line 2: no match: /no/such/*.zzz\n",
            "diagnostics; stdout:\n{stdout}"
        );
    }
}

/// rubash#759 item 3: `shift` with a bad operand is a USAGE error (2), not
/// a plain failure. GNU builtins/shift.def routes a non-numeric operand
/// through get_numeric_arg (builtins/common.c:501-505 sh_neednumarg,
/// get_numeric_arg returns 0 and shift_builtin answers
/// `return (EX_USAGE)`) and a surplus operand through no_args
/// (`set_exit_status (EX_BADUSAGE)`); only the shift-count-erange path
/// returns EXECUTION_FAILURE.
/// GNU 5.3.0 differential (WSL stdin probes 2026-10-18):
///   ( set -- 1 2; shift x )    -> 2   (was 1)
///   ( set -- 1 2; shift 1 2 )  -> 2   (was 1)
///   ( set -- 1 2; shift -1 )   -> 1
///   set -- 1 2 3; shift 99     -> 1
#[test]
fn shift_bad_operand_status_matches_gnu() {
    let dir = case_scratch("759.shift");
    let run = |code: &str| {
        let out = run_rubash_in(&dir, &["-c", code], Duration::from_secs(30));
        String::from_utf8_lossy(&out.stdout).trim_end().to_string()
    };

    let statuses = run("( set -- 1 2; shift x ) 2>/dev/null; echo \"shiftx:$?\"\n\
         ( set -- 1 2; shift 1 2 ) 2>/dev/null; echo \"shift12:$?\"\n\
         ( set -- 1 2; shift -1 ) 2>/dev/null; echo \"shiftneg:$?\"\n\
         set -- 1 2 3; shift 99 2>/dev/null; echo \"shiftbig:$?\"");
    assert_eq!(
        statuses, "shiftx:2\nshift12:2\nshiftneg:1\nshiftbig:1",
        "shift statuses"
    );

    // The diagnostics themselves keep GNU's wording (only the status
    // changed): `shift: x: numeric argument required` and
    // `shift: too many arguments`.
    let out = run_rubash_in(
        &dir,
        &["-c", "set -- 1 2; shift x; shift 1 2"],
        Duration::from_secs(30),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("shift: x: numeric argument required"),
        "{stderr}"
    );
    assert!(stderr.contains("shift: too many arguments"), "{stderr}");
}

/// rubash#759 item 4: GNU bash has no `setopt`/`unsetopt` builtins — a
/// script probing `command -v setopt` to detect zsh must get the GNU
/// answer. After the removal `setopt x` is command-not-found (127),
/// `builtin setopt x` is not-a-shell-builtin (1), and `enable -a` lists
/// exactly GNU's 61 builtin names.
/// GNU 5.3.0 differential (WSL stdin probe 2026-10-18):
///   command -v setopt    -> rc 1, no output   (was "setopt", rc 0)
///   command -v unsetopt  -> rc 1, no output   (was "unsetopt", rc 0)
///   enable -a | wc -l    -> 61                (was 64 on Windows)
#[test]
fn zsh_option_builtins_are_gone() {
    let dir = case_scratch("759.setopt");

    let out = run_rubash_in(
        &dir,
        &[
            "-c",
            "command -v setopt; echo \"v:$?\"\ncommand -v unsetopt; echo \"v:$?\"\nenable -a",
        ],
        Duration::from_secs(30),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    assert_eq!(lines.next(), Some("v:1"), "command -v setopt misses");
    assert_eq!(lines.next(), Some("v:1"), "command -v unsetopt misses");
    let listed = lines.filter(|line| !line.trim().is_empty()).count();
    assert_eq!(
        listed, 61,
        "enable -a must list exactly GNU's 61 builtin names: {stdout}"
    );

    let out = run_rubash_in(
        &dir,
        &[
            "-c",
            "setopt hist_ignore_dups null_glob; echo \"plain:$?\"\n\
             shopt -q nullglob; echo \"nullglob:$?\"\n\
             builtin unsetopt null_glob; echo \"builtin:$?\"",
        ],
        Duration::from_secs(30),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        stdout.trim_end(),
        "plain:127\nnullglob:1\nbuiltin:1",
        "setopt/unsetopt are plain unknown commands; option state untouched"
    );
    assert!(stderr.contains("setopt: command not found"), "{stderr}");
    assert!(
        stderr.contains("builtin: unsetopt: not a shell builtin"),
        "{stderr}"
    );
}

/// rubash#759 item 4 (sudo): GNU bash has no `sudo` builtin — `command -v
/// sudo` resolves it on PATH like any external (WSL: /usr/bin/sudo). rubash
/// keeps the Windows host-capability builtin behind an embedding-host
/// elevation handler (the docs/builtins.md §3 hidden-builtin contract), so
/// with no handler the name must classify as external everywhere. On a
/// Windows box without sudo.exe that is rc 1 with no output; the invariant
/// under test is "never reports a builtin".
/// GNU 5.3.0 differential (WSL stdin probe 2026-10-18):
///   command -v sudo -> /usr/bin/sudo (external), rc 0
///   type -t sudo    -> file, rc 0
#[test]
fn sudo_is_not_reported_as_a_builtin() {
    let dir = case_scratch("759.sudo");
    let out = run_rubash_in(
        &dir,
        &[
            "-c",
            "command -v sudo; echo \"v:$?\"\ntype -t sudo; echo \"t:$?\"",
        ],
        Duration::from_secs(30),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.lines().any(|line| line == "sudo"),
        "`command -v sudo` must not report the bare builtin name: {stdout}"
    );
    assert!(
        !stdout.contains("sudo is a shell builtin"),
        "`type sudo` must not report a builtin: {stdout}"
    );
}

/// #813 item 2 (the #810 CR F4 coverage gap, arm 1): with no elevation
/// handler installed (the CLI binary's default), `sudo cmd` must take the
/// PATH execution route — a fake `sudo` earlier on PATH actually runs and
/// its status becomes the command's. Before #810 the name was answered by
/// the host builtin with "elevation handler is not configured" (rc 1) and
/// nothing on PATH was consulted. `routes_as_builtin`
/// (src/executor/lookup_paths.rs) is the gate: windows + handler = builtin,
/// everything else = PATH.
#[cfg(windows)]
#[test]
fn sudo_without_handler_executes_the_path_command() {
    let dir = case_scratch("813.sudo.path");
    let bindir = dir.join("fakebin");
    std::fs::create_dir_all(&bindir).expect("create fakebin");
    // cmd.exe batch: emit a marker line, pass the child's status through.
    std::fs::write(
        bindir.join("sudo.cmd"),
        "@echo off\r\necho FAKE_SUDO_RAN %*\r\nexit /b 0\r\n",
    )
    .expect("write fake sudo");
    let script = format!(
        "export PATH='{}':\"$PATH\"\nsudo --preserve-env probe\necho \"rc:$?\"",
        bindir.display()
    );
    let out = run_rubash_in(&dir, &["-c", &script], Duration::from_secs(30));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stdout.contains("FAKE_SUDO_RAN --preserve-env probe"),
        "fake PATH sudo must actually run: stdout={stdout} stderr={stderr}"
    );
    assert!(
        stdout.contains("rc:0"),
        "fake sudo's status must become the command status: {stdout}"
    );
    assert!(
        !stderr.contains("elevation handler"),
        "the no-handler diagnostic must not resurface: {stderr}"
    );
}

/// #813 item 2 (the #810 CR F4 coverage gap, arm 2): `builtin sudo` with no
/// elevation handler falls through the hidden-builtin dispatch to
/// `write_builtin_not_found` (src/executor/builtin_direct_command.rs) —
/// GNU's "not a shell builtin" wording, rc 1.
#[cfg(windows)]
#[test]
fn builtin_sudo_without_handler_reports_not_a_shell_builtin() {
    let dir = case_scratch("813.sudo.builtin");
    let out = run_rubash_in(
        &dir,
        &["-c", "builtin sudo\necho \"rc:$?\""],
        Duration::from_secs(30),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        stdout.trim_end(),
        "rc:1",
        "the builtin fall-through must fail rc 1: stdout={stdout} stderr={stderr}"
    );
    assert!(
        stderr.contains("builtin: sudo: not a shell builtin"),
        "GNU wording expected: {stderr}"
    );
}

/// CR follow-up on rubash#757 (F1): the prompt-pass containment. GNU
/// subst.c:4392-4418 expand_prompt_string runs the expansion with
/// `no_longjmp_on_fatal_error = 1` and RETURNS THE PASSED STRING when the
/// expansion verdicts: the diagnostic stands, the prompt value falls back
/// to the decoded input, and the enclosing command's status is untouched.
/// Before the containment a malformed `${...}` inside PS1 failed the whole
/// assignment (rc 1, variable unset).
/// GNU 5.3.0 differential (WSL stdin probes 2026-10-18, probe-f1/f1b/f1c):
///   PS1='a${1x}b'; p=${PS1@P}  -> stderr `a${1x}b: bad substitution`,
///                                 rc 0, p=`a${1x}b`, next line runs
///   PS1='a$((1x))b'            -> arith diagnostic, rc 0, literal
///   PS1='a${bad'               -> `a${bad: bad substitution`, rc 0, literal
///   PS1='er:${nose?bad} t'     -> stderr `nose: bad`, rc 0, literal text
///   PS1='a$((1/0))b'           -> division diagnostic, rc 0, literal
#[test]
fn prompt_expansion_failure_contained_like_gnu() {
    let dir = case_scratch("757.containment");
    let run = |code: &str| {
        let out = run_rubash_in(&dir, &["-c", code], Duration::from_secs(30));
        (
            String::from_utf8_lossy(&out.stdout).trim_end().to_string(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            out.code,
        )
    };

    // Bad substitution: diagnostic on stderr, value literal, rc 0, tail runs.
    let (stdout, stderr, code) =
        run("PS1='a${1x}b'\np=${PS1@P}\necho \"rc=$? p=[$p]\"\necho after");
    assert_eq!(code, Some(0), "list rc; stderr:\n{stderr}");
    assert_eq!(
        stdout, "rc=0 p=[a${1x}b]\nafter",
        "stdout; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("a${1x}b: bad substitution"),
        "diagnostic; stderr:\n{stderr}"
    );

    // Arithmetic verdict, unterminated brace, `?` operator: same contract.
    let (stdout, stderr, code) = run("PS1='a$((1x))b'\np=${PS1@P}\necho \"arith:$? [$p]\"\n\
         PS1='a${bad'\nq=${PS1@P}\necho \"unterm:$? [$q]\"\n\
         unset p757_nose\nPS1='er:${p757_nose?bad} t'\nr=${PS1@P}\necho \"qmark:$? [$r]\"");
    assert_eq!(code, Some(0), "list rc; stderr:\n{stderr}");
    assert_eq!(
        stdout, "arith:0 [a$((1x))b]\nunterm:0 [a${bad]\nqmark:0 [er:${p757_nose?bad} t]",
        "stdout; stderr:\n{stderr}"
    );
    assert!(stderr.contains("value too great for base"), "{stderr}");
    assert!(stderr.contains("a${bad: bad substitution"), "{stderr}");
    assert!(
        stderr.lines().any(|line| line.contains("p757_nose: bad")),
        "`?` operator reports on stderr: {stderr}"
    );

    // A set variable under `?` still expands (no verdict).
    let (stdout, _stderr, code) =
        run("p757_x=set\nPS1='v${p757_x?nope}w'\np=${PS1@P}\necho \"xset:$? [$p]\"");
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "xset:0 [vsetw]");
}
