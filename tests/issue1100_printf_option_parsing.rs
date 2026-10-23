//! Issue rubash#1100 regression: leading-dash `printf` formats follow
//! GNU internal_getopt(list, "v:") (builtins/bashgetopt.c) +
//! printf.def:296-337 — option words scan character-wise, the first
//! unrecognized char is reported as `-<c>` (`---` -> `--`, `-%s` ->
//! `-%`), `-v` consumes rest-of-word or the next word unconditionally
//! and validates its identifier immediately, and `--` ends options.

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

fn assert_invalid_option(args: &str, reported: &str) {
    let (stdout, stderr, code) = rubash(&format!("printf {args}"));
    assert_eq!(stdout, "", "printf {args} stdout");
    assert_eq!(code, Some(2), "printf {args} status");
    assert_eq!(
        stderr,
        format!(
            "bash: line 1: printf: -{reported}: invalid option\n\
             printf: usage: printf [-v var] format [arguments]\n"
        ),
        "printf {args} stderr"
    );
}

#[test]
fn leading_dash_format_reports_the_first_option_char() {
    // `---` scans `-` first -> `--`; `-%s` -> `-%`.
    assert_invalid_option("'--- %s ---\\n' x", "-");
    assert_invalid_option("'-%s\\n' hi", "%");
    assert_invalid_option("--v 'a\\n'", "-");
    assert_invalid_option("-xv 'a\\n'", "x");
    assert_invalid_option("-9 'f\\n'", "9");
}

#[test]
fn option_scanning_resumes_after_a_dash_v_argument() {
    // getopt keeps scanning the word list after -v consumed its name.
    assert_invalid_option("-v a -x 'f\\n'", "x");
    assert_invalid_option("-vx -y 'f\\n'", "y");
}

#[test]
fn dash_v_takes_rest_of_word_or_next_word_as_name() {
    // -v<name> inline form and `-v name` both bind; last -v wins.
    let (_, _, code) = rubash("printf -vq 'F\\n'");
    assert_eq!(code, Some(0));
    let (stdout, _, _) = rubash("printf -v a -v b 'F=%s\\n' hi; echo \"a=[$a] b=[$b]\"");
    assert_eq!(stdout, "a=[] b=[F=hi\n]\n");
}

#[test]
fn dash_v_consumes_dash_words_as_its_argument() {
    // `v:` takes the next word unconditionally — `-x` here is a NAME,
    // so the diagnostic is the identifier check, not an option error.
    let (_, stderr, code) = rubash("printf -v -x 'F\\n'");
    assert_eq!(code, Some(2));
    assert_eq!(
        stderr,
        "bash: line 1: printf: `-x': not a valid identifier\n"
    );
}

#[test]
fn invalid_identifier_check_fires_before_later_option_errors() {
    // printf.def:306-322 validates inside the `case 'v'` arm — the -x
    // identifier diagnostic wins over the later -y option error.
    let (_, stderr, code) = rubash("printf -v -x -y 'f\\n'");
    assert_eq!(code, Some(2));
    assert_eq!(
        stderr,
        "bash: line 1: printf: `-x': not a valid identifier\n"
    );
}

#[test]
fn dash_v_without_argument_gets_usage() {
    let (_, stderr, code) = rubash("printf -v");
    assert_eq!(code, Some(2));
    assert_eq!(
        stderr,
        "bash: line 1: printf: -v: option requires an argument\n\
         printf: usage: printf [-v var] format [arguments]\n"
    );
}

#[test]
fn lone_dash_is_the_format_not_an_option() {
    // NOTOPT(s): a word of exactly `-` ends option scanning.
    let (stdout, _, code) = rubash("printf - 'a\\n'");
    assert_eq!(stdout, "-");
    assert_eq!(code, Some(0));
}

#[test]
fn double_dash_ends_options() {
    let (stdout, _, code) = rubash("printf -- '-%s\\n' hi");
    assert_eq!(stdout, "-hi\n");
    assert_eq!(code, Some(0));
    // `--` with nothing after it is still a usage error.
    let (_, stderr, code) = rubash("printf --");
    assert_eq!(code, Some(2));
    assert_eq!(
        stderr,
        "printf: usage: printf [-v var] format [arguments]\n"
    );
}

#[test]
fn dash_dash_help_prints_builtin_help_and_returns_usage() {
    // ISHELP -> CASE_HELPOPT (builtins/common.h:27-41).
    let (stdout, _, code) = rubash("printf --help");
    assert!(stdout.starts_with("printf: printf [-v var] format [arguments]"));
    assert_eq!(code, Some(2));
}
