//! rubash#729 — carrier byte-class regressions in the pattern matchers.
//!
//! GNU matches patterns and words as byte strings (subst.c pat_substr,
//! sm_loop.c gmatch walk `unsigned char` buffers). Rubash transport has
//! several spellings for the same byte — the raw-byte marker pair
//! (E000+E0xx), a CTLESC-quoted pair (quote-protected span), a literal C0
//! char (a visible-text decode that re-materialized the byte) and the
//! `__RUBASH_CSB1_xx;` payload. The conditional matcher now normalizes all
//! of them to byte-chars and the POSIX classes ([[:cntrl:]] etc.)
//! classify a byte-char by the byte it carries; these tests pin the faces
//! that were diverging from GNU bash 5.3.0 (probe transcripts in
//! target/issue-suites/results/probe729/).

use std::process::Command;

fn rubash(script: &str) -> (String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .output()
        .expect("run rubash");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn rubash_stdout(script: &str) -> String {
    rubash(script).0
}

#[test]
fn posix_cntrl_class_matches_every_carrier_byte() {
    // GNU iscntrl(byte) is true for 0x01..=0x1f and 0x7f; the bytes that
    // travel pair-encoded (0x0c, 0x11, 0x13, 0x14..=0x1f) must classify by
    // their byte, not by their byte-char codepoint.
    // $'...' spelling: the $(printf) form cannot carry 0x0a (command
    // substitution strips trailing newlines in BOTH shells).
    for byte in 1..=31u32 {
        let script = format!(
            "x=$'\\x{{{:02x}}}'; [[ $x == *[[:cntrl:]]* ]]; echo $?",
            byte
        );
        assert_eq!(
            rubash_stdout(&script),
            "0\n",
            "byte {byte:#04x} must match [[:cntrl:]] (script: {script})"
        );
    }
    let script = "x=$'\x7f'; [[ $x == *[[:cntrl:]]* ]]; echo $?";
    assert_eq!(rubash_stdout(script), "0\n");
}

#[test]
fn posix_cntrl_class_rejects_printable() {
    let (out, _) = rubash("x=$'a'; [[ $x == *[[:cntrl:]]* ]]; echo $?");
    assert_eq!(out, "1\n");
}

#[test]
fn posix_space_class_matches_glibc_c_locale_set() {
    // glibc C locale is_space: space, \t, \n, \v, \f, \r. Rust's
    // is_ascii_whitespace excludes \v — GNU matches it.
    for byte in [0x09u32, 0x0a, 0x0b, 0x0c, 0x0d, 0x20] {
        let script = format!(
            "x=$'\\x{{{:02x}}}'; [[ $x == *[[:space:]]* ]]; echo $?",
            byte
        );
        assert_eq!(
            rubash_stdout(&script),
            "0\n",
            "byte {byte:#04x} must match [[:space:]]"
        );
    }
}

#[test]
fn posix_print_class_rejects_carrier_bytes() {
    let script = "x=$(printf '\\001'); [[ $x == *[[:print:]]* ]]; echo $?";
    assert_eq!(rubash_stdout(script), "1\n");
    let script = "x=$'q'; [[ $x == *[[:print:]]* ]]; echo $?";
    assert_eq!(rubash_stdout(script), "0\n");
}

#[test]
fn bracket_range_covers_carried_bytes() {
    let script =
        "x=$(printf '\\001'); [[ $x == *[$(printf '\\001')-$(printf '\\005')]* ]]; echo $?";
    assert_eq!(rubash_stdout(script), "0\n");
    let script =
        "x=$(printf '\\007'); [[ $x == *[$(printf '\\001')-$(printf '\\005')]* ]]; echo $?";
    assert_eq!(rubash_stdout(script), "1\n");
}

#[test]
fn case_statement_class_sees_carrier_bytes() {
    let script = "case $'\\x1b' in *[[:cntrl:]]*) echo hit;; *) echo miss;; esac";
    assert_eq!(rubash_stdout(script), "hit\n");
}

#[test]
fn pattern_spelling_agreement_pair_vs_ansi_c() {
    // The word carries the pair; the pattern spells the same byte through
    // $'...' (also pair-encoded). Both sides normalize to the same
    // byte-char.
    assert_eq!(
        rubash_stdout("[[ $'\\x01' == *$'\\x01'* ]]; echo $?"),
        "0\n"
    );
    assert_eq!(
        rubash_stdout("[[ $'\\x10' == *$'\\x10'* ]]; echo $?"),
        "0\n"
    );
}

#[test]
fn removal_op_with_carrier_pattern_byte() {
    // ${v#pat} with a pair-spelled pattern byte (this shape routes through
    // remove_matching_prefix_bytes -> case_pattern_matches).
    let script = "v=$'a\\x01c'; echo \"${v#*$'\\x01'}\"";
    assert_eq!(rubash_stdout(script), "c\n");
    let script = "v=$'a\\x01c'; echo \"${v%$'\\x01'*}\"";
    assert_eq!(rubash_stdout(script), "a\n");
}

#[test]
fn patsub_class_replaces_carrier_byte() {
    let script = "v=$'a\\x01c'; echo \"${v//[[:cntrl:]]/X}\"";
    assert_eq!(rubash_stdout(script), "aXc\n");
}

#[test]
fn quoted_word_still_matches_plain_pattern() {
    // The normalization flattens the word side too; a quoted word's
    // CTLESC-quoting must dequote, not byte-char, its data.
    assert_eq!(rubash_stdout("[[ \"a*b\" == a*b* ]]; echo $?"), "0\n");
    assert_eq!(rubash_stdout("[[ \"aXb\" == a*b* ]]; echo $?"), "0\n");
    assert_eq!(rubash_stdout("[[ \"aXb\" == a?b* ]]; echo $?"), "0\n");
}

#[test]
fn bracket_class_names_survive_normalization() {
    // A class-name bracket inside a normalized pattern must still
    // recognize [[:cntrl:]] (not byte-char the class name).
    assert_eq!(
        rubash_stdout("x=$'\\x01'; [[ $x == *[[:cntrl:]]* ]]; echo $?"),
        "0\n"
    );
    // 0x01 is NOT glibc is_space (the space set is \t\n\v\f\r space).
    assert_eq!(
        rubash_stdout("x=$'\\x01'; [[ $x == *[[:space:]]* ]]; echo $?"),
        "1\n"
    );
    // A byte that IS whitespace keeps matching space after normalization.
    assert_eq!(
        rubash_stdout("x=$'\\x0c'; [[ $x == *[[:space:]]* ]]; echo $?"),
        "0\n"
    );
}

/// rubash#735 — user U+E4xx data rides push_literal_char's E400 escape and
/// must never be eaten as a marker, on any path touched by #729.
#[test]
fn user_pua_data_roundtrips_verbatim() {
    // $'\uE401' arrives E400-escaped; output bytes must be the user's
    // UTF-8 (ee 90 81), not a stripped guard.
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("x=$'\\uE401'; printf '%s' \"$x\" | od -An -tx1")
        .output()
        .expect("run rubash");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(stdout.contains("ee 90 81"), "user E401 mangled: {stdout}");
}

#[test]
fn user_pua_length_counts_one_char() {
    // 746 case 735.2: ${#x} for $'\uE301' is 1 (the E400 escape is not a
    // character of the value).
    assert_eq!(rubash_stdout("x=$'\\uE301'; echo ${#x}"), "1\n");
}

#[test]
fn user_e401_not_stripped_by_guard_paths() {
    // The ANSI_C_IFS_GUARD consumer strips BARE E401; the user's
    // E400-escaped E401 must survive the same strips.
    let script = "x=$'\\uE401'; y=${x}; [ -n \"$y\" ] && echo kept";
    assert_eq!(rubash_stdout(script), "kept\n");
}

/// PR #821 rework: an UNQUOTED command substitution inside a `[[ == ]]`
/// glob pattern arrives CSB1-protected (`*__RUBASH_CSB1_01;*`), which is
/// all printable ASCII — literal_pattern_equality / simple_glob_matches
/// admitted it on `pattern.bytes()` alone and verdicted before the
/// byte-spelling normalization gate ran. sides_need_byte_char_matching
/// must run first so the payload decodes to the byte-char the word side
/// carries (GNU bash 5.3.0: the `==` forms below are matches).
#[test]
fn unquoted_comsub_byte_in_conditional_pattern() {
    assert_eq!(
        rubash_stdout("v=$'a\\x01c'; [[ $v == *$(printf '\\001')* ]]; echo $?"),
        "0\n"
    );
    // Variable-carried spelling (the byte rides a word, not the pattern).
    assert_eq!(
        rubash_stdout("v=$'a\\x01c'; x=$(printf '\\001'); [[ $v == *$x* ]]; echo $?"),
        "0\n"
    );
    // `?` measures one logical byte: a pair-encoded byte is two transport
    // chars but one byte-char after normalization (GNU: match).
    assert_eq!(rubash_stdout("x=$'\\x11'; [[ $x == ? ]]; echo $?"), "0\n");
    // Inequality form sees the same gate ordering.
    assert_eq!(
        rubash_stdout("v=$'a\\x01c'; [[ $v != *$(printf '\\001')* ]]; echo $?"),
        "1\n"
    );
}
