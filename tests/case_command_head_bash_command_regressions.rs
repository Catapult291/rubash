//! `case` printed-command head fidelity (`$BASH_COMMAND` during the case
//! word expansion).
//!
//! GNU `execute_case_command` (execute_cmd.c:3655-3665) resets
//! `command_string_index` and prints only the head via
//! `print_case_command_head` (print_cmd.c:731-739) before expanding the case
//! word (3695-3699), so `the_printed_command_except_trap` — what
//! `$BASH_COMMAND` reads — is exactly `case WORD in ` with WORD still
//! carrying its source quote characters (`case_command->word->word`). The
//! clause list is rendered only for the JOB's `command` field
//! (jobs.c make_job -> print_command), not for the per-execution printed
//! command. Rubash stored the full clause list, so `${BASH_COMMAND}` in the
//! case word saw the whole `case ... esac` text instead of the head.
//!
//! Expected outputs are byte-verified against GNU Bash 5.3.0/5.3.20
//! (script-file probes, 2026-10-21).

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

/// A double-quoted case word keeps its quotes in the head that
/// `$BASH_COMMAND` exposes while the word itself is being expanded.
#[test]
fn case_word_expansion_sees_quoted_head() {
    let (stdout, _stderr, code) = rubash(
        r#"case "${BASH_COMMAND}" in 'case "${BASH_COMMAND}" in '*) echo HEAD;; *) echo OTHER;; esac"#,
    );
    assert_eq!(stdout, "HEAD\n");
    assert_eq!(code, Some(0));
}

/// An unquoted case word exposes the plain head.
#[test]
fn case_word_expansion_sees_unquoted_head() {
    let (stdout, _stderr, code) = rubash(
        "case ${BASH_COMMAND} in 'case ${BASH_COMMAND} in '*) echo HEAD;; *) echo OTHER;; esac",
    );
    assert_eq!(stdout, "HEAD\n");
    assert_eq!(code, Some(0));
}

/// The clause list is not part of the printed head: a pattern that only
/// matches the full `case ... esac` text must not match.
#[test]
fn case_word_expansion_head_has_no_clause_list() {
    let (stdout, _stderr, code) = rubash(
        r#"case "${BASH_COMMAND}" in 'case "${BASH_COMMAND}" in '*) echo HEAD;; *'echo'*) echo FULL;; *) echo OTHER;; esac"#,
    );
    assert_eq!(stdout, "HEAD\n");
    assert_eq!(code, Some(0));
}

/// Inside the matched clause body `$BASH_COMMAND` is the body command (the
/// body's own `set_current_command` overwrites the case head), matching GNU.
#[test]
fn case_body_sees_its_own_command() {
    let (stdout, _stderr, code) = rubash("case x in x) printf '%s\\n' \"$BASH_COMMAND\";; esac");
    assert_eq!(stdout, "printf '%s\\n' \"$BASH_COMMAND\"\n");
    assert_eq!(code, Some(0));
}
