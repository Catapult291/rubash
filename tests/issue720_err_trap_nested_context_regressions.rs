//! Issue rubash#720 — `set -E` ERR-trap nested-context divergences from GNU
//! bash, all deterministic and byte-pinned.
//!
//! Form 1 (nested subshell fire count): GNU executes a user subshell's whole
//! body IN PLACE when that body is a single subshell command —
//! execute_cmd.c:1799-1806 (execute_in_subshell) marks the single
//! cm_subshell/cm_simple body CMD_NO_FORK and execute_cmd.c:662-663
//! dispatches it without make_child/wait_for — so the middle frame never
//! observes the failure and its :769 run_error_trap never runs. `( (false)
//! )` fires twice (the innermost `false`, the outermost waiter), not three
//! times.
//!
//! Form 2 (`f() ( )` flat function body): the body subshell's completion ERR
//! adjudication runs in the frame that waited for its fork — wait_for then
//! run_error_trap keyed on the subshell command (execute_cmd.c:756-778, no
//! &&/|| exemption, LINENO = the subshell's own `)` line per execute_cmd.c:
//! 696 SET_LINE_LINE). The forkless flat region had no boundary fire, so
//! `f() ( false ); f` fired twice where GNU fires three times, and the
//! call-site fire reported `f` where GNU reports the region text
//! `( false )` (the_printed_command_except_trap fixed by the fork preamble,
//! execute_cmd.c:703-710).
//!
//! Form 4 (return-in-until attribution): a `return 7` unwinding through
//! dispatch is the builtin's own completion. GNU's return.def:112-115 jumps
//! only for an out-of-range count; the function boundary stamps
//! last_command_exit_value = 7 and the CALL's gate adjudicates with it.
//! Rubash's dispatcher gate fired with the loop condition's stale status
//! (`until false` leaves $? = 1) and lost the 7.
//!
//! Form 3 (`cat <(false)` process-substitution copy path) is NOT covered
//! here: it is a read/procsub-reaping fix (the PR #715 area) and stays
//! tracked in the issue.
//!
//! Every expectation below is the literal WSL GNU bash 5.3.0(1)-release
//! output captured through the stdin oracle (`wsl -e bash -s < script`).

use std::process::Command;

fn run(source: &str) -> (String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .args(["-c", source])
        .env_remove("BASH_ENV")
        .output()
        .expect("run rubash");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        output.status.code().unwrap_or(-1),
    )
}

// ---------------------------------------------------------------------------
// Form 1 — nested subshell fire count (fork-collapse suppression)
// ---------------------------------------------------------------------------

/// The issue repro: `( (false) )` fires for the innermost `false` and the
/// outermost waiter only — the middle frame's fork is collapsed into the
/// outer child (execute_cmd.c:662-663), so no middle fire.
#[test]
fn nested_subshell_fires_twice_not_three_times() {
    let (out, code) =
        run("set -E; trap 'echo ERR sub=$BASH_SUBSHELL' ERR; ( (false) ); echo rc=$?");
    assert_eq!(out, "ERR sub=2\nERR sub=0\nrc=1\n");
    assert_eq!(code, 0);
}

/// Three levels collapse the same way: every intermediate frame is silent.
#[test]
fn three_level_nesting_fires_innermost_and_top_only() {
    let (out, code) =
        run("set -E; trap 'echo ERR sub=$BASH_SUBSHELL' ERR; ( ( (false) ) ); echo rc=$?");
    assert_eq!(out, "ERR sub=3\nERR sub=0\nrc=1\n");
    assert_eq!(code, 0);
}

/// A body that is NOT a single subshell command keeps the middle fire: GNU
/// forks `(false)` normally there (no CMD_NO_FORK), so the frame that waits
/// for it adjudicates (execute_cmd.c:769).
#[test]
fn non_collapsed_middle_still_fires() {
    let (out, code) =
        run("set -E; trap 'echo ERR sub=$BASH_SUBSHELL' ERR; ( (false); false ); echo rc=$?");
    assert_eq!(out, "ERR sub=2\nERR sub=1\nERR sub=1\nERR sub=0\nrc=1\n");
    assert_eq!(code, 0);
}

/// A brace-group body also keeps the middle fire (cm_group bodies are never
/// marked CMD_NO_FORK at execute_cmd.c:1799).
#[test]
fn brace_group_middle_still_fires() {
    let (out, code) =
        run("set -E; trap 'echo ERR sub=$BASH_SUBSHELL' ERR; ( { (false); } ); echo rc=$?");
    assert_eq!(out, "ERR sub=2\nERR sub=1\nERR sub=0\nrc=1\n");
    assert_eq!(code, 0);
}

/// Unter `set -e` the collapsed middle stays silent while the top still
/// adjudicates before the ERREXIT jump (execute_cmd.c:769 precedes :777).
#[test]
fn errexit_collapsed_nesting_fires_top_before_exit() {
    let (out, code) =
        run("set -E; trap 'echo ERR sub=$BASH_SUBSHELL' ERR; set -e; ( (false) ); echo never");
    assert_eq!(out, "ERR sub=2\nERR sub=0\n");
    assert_eq!(code, 1);
}

// ---------------------------------------------------------------------------
// Form 2 — `f() ( )` flat region boundary fire
// ---------------------------------------------------------------------------

/// The issue repro: the flat body's completion ERR fire exists (GNU :769 in
/// the caller frame), so the count is three, not two.
#[test]
fn flat_function_body_fires_three_levels() {
    let (out, code) = run("set -E; trap 'echo ERR' ERR; f() ( false ); f; echo rc=$?");
    assert_eq!(out, "ERR\nERR\nERR\nrc=1\n");
    assert_eq!(code, 0);
}

/// A brace-body function already fired the same three shape and must stay
/// correct (the body subshell is a real node there).
#[test]
fn brace_body_function_stays_correct() {
    let (out, code) = run("set -E; trap 'echo ERR' ERR; g(){ (false); }; g; echo rc=$?");
    assert_eq!(out, "ERR\nERR\nERR\nrc=1\n");
    assert_eq!(code, 0);
}

/// The flat region's fire reports the region text as BASH_COMMAND (the fork
/// preamble pins the_printed_command_except_trap, execute_cmd.c:703-710) and
/// the close-paren line as $LINENO (execute_cmd.c:696); the inner `false`
/// reports its own line.
#[test]
fn flat_region_fire_reports_region_text_and_close_line() {
    let source = concat!(
        "set -E\n",
        "trap 'echo ERR sub=$BASH_SUBSHELL lin=$LINENO cmd=$BASH_COMMAND' ERR\n",
        "f() (\n",
        "  false\n",
        ")\n",
        "f\n",
        "echo rc=$?",
    );
    let (out, code) = run(source);
    assert_eq!(
        out,
        concat!(
            "ERR sub=1 lin=4 cmd=false\n",
            "ERR sub=0 lin=5 cmd=( false )\n",
            "ERR sub=0 lin=6 cmd=( false )\n",
            "rc=1\n"
        )
    );
    assert_eq!(code, 0);
}

/// A flat region whose body is itself a subshell collapses that inner
/// aggregate (form 1) but still fires the region and the call.
#[test]
fn nested_flat_function_body_fires_region_and_call() {
    let (out, code) =
        run("set -E; trap 'echo ERR sub=$BASH_SUBSHELL' ERR; g() ( (false) ); g; echo rc=$?");
    assert_eq!(out, "ERR sub=2\nERR sub=0\nERR sub=0\nrc=1\n");
    assert_eq!(code, 0);
}

// ---------------------------------------------------------------------------
// Form 4 — return-in-until attribution
// ---------------------------------------------------------------------------

/// The issue repro: the ERR trap fires with the return's own status and the
/// shell exits with it — the loop condition's stale 1 must not surface.
#[test]
fn return_in_loop_body_fires_with_own_status() {
    let (out, code) =
        run("set -e; trap 'echo ERR:$?' ERR; f(){ return 7; }; until false; do f; done");
    assert_eq!(out, "ERR:7\n");
    assert_eq!(code, 7);
}

/// `while` behaves the same.
#[test]
fn return_in_while_body_fires_with_own_status() {
    let (out, code) = run("set -e; trap 'echo ERR:$?' ERR; f(){ return 7; }; while :; do f; done");
    assert_eq!(out, "ERR:7\n");
    assert_eq!(code, 7);
}

/// Without errexit the plain call still fires with the return's status and
/// keeps running (GNU: ERR:7 lin=<def line>, rc=7 printed).
#[test]
fn return_plain_call_fires_with_own_status() {
    let (out, code) = run("set -E; trap 'echo ERR:$?' ERR; f(){ return 7; }; f; echo rc=$?");
    assert_eq!(out, "ERR:7\nrc=7\n");
    assert_eq!(code, 0);
}

// ---------------------------------------------------------------------------
// Form 3 — `cat <(false)` process-substitution copy path: NOT covered; the
// divergence stays tracked in rubash#720 (read/procsub reaping area).
// ---------------------------------------------------------------------------
