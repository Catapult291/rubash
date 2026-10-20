//! rubash#871: `%f`/`%e`/`%g` must recognize glibc strtod word prefixes.
//!
//! glibc strtod takes the case-insensitive `inf`/`nan` prefix and then the
//! longest valid continuation (`inity` completes `infinity`; a closed paren
//! group extends `nan`), converting the value while the rejected tail still
//! fires the `invalid number` diagnostic. Every expectation below is
//! byte-verified against WSL GNU Bash (`/usr/local/bin/bash`, script-file
//! batteries `target/probe/871.sh` + `871b.sh`). Note the harness `run`
//! returns stdout without a trailing newline, so the `%f` newline from the
//! probes is absent here.

use super::{run, EXECUTION_FAILURE, EXECUTION_SUCCESS};

#[test]
fn inf_word_prefixes_convert_with_suffix_diagnostic() {
    for operand in ["infin", "infx", "infinite", "infinityX"] {
        let (status, stdout, stderr, _) = run(&["%f", operand]);
        assert_eq!(stdout, "inf", "operand {operand}");
        assert_eq!(status, EXECUTION_FAILURE, "operand {operand}");
        assert!(
            stderr.contains(&format!("{operand}: invalid number")),
            "operand {operand}: {stderr:?}"
        );
    }
    // The exact standard forms stay diagnostic-free.
    for operand in ["inf", "infinity", "Inf", "iNfInItY", "INF", "INFINITY"] {
        let (status, stdout, stderr, _) = run(&["%f", operand]);
        assert_eq!(stdout, "inf", "operand {operand}");
        assert_eq!(status, EXECUTION_SUCCESS, "operand {operand}");
        assert!(stderr.is_empty(), "operand {operand}: {stderr:?}");
    }
}

#[test]
fn nan_word_prefixes_convert_with_suffix_diagnostic() {
    for operand in ["nann", "nanx", "NANX", "+nanX", "Nan(abc)def"] {
        let (status, stdout, stderr, _) = run(&["%f", operand]);
        assert_eq!(stdout, "nan", "operand {operand}");
        assert_eq!(status, EXECUTION_FAILURE, "operand {operand}");
        assert!(
            stderr.contains(&format!("{operand}: invalid number")),
            "operand {operand}: {stderr:?}"
        );
    }
    // Closed paren groups are consumed: no diagnostic, still nan.
    for operand in ["nan", "NAN", "nan(2)", "nan()", "nan(abc)", "NAN(ABC)"] {
        let (status, stdout, stderr, _) = run(&["%f", operand]);
        assert_eq!(stdout, "nan", "operand {operand}");
        assert_eq!(status, EXECUTION_SUCCESS, "operand {operand}");
        assert!(stderr.is_empty(), "operand {operand}: {stderr:?}");
    }
}

#[test]
fn inf_paren_tail_stays_rejected() {
    // Only `nan` consumes a paren group; `inf(2)` converts `inf` and rejects
    // `(2)`.
    for operand in ["inf(2)", "infinity(2)", "INF(2)", "nanx(2)", "nan()x"] {
        let (status, stdout, stderr, _) = run(&["%f", operand]);
        let want = if operand.starts_with('n') || operand.starts_with('N') {
            "nan"
        } else {
            "inf"
        };
        assert_eq!(stdout, want, "operand {operand}");
        assert_eq!(status, EXECUTION_FAILURE, "operand {operand}");
        assert!(
            stderr.contains(&format!("{operand}: invalid number")),
            "operand {operand}: {stderr:?}"
        );
    }
}

#[test]
fn exponent_and_general_formats_share_the_rule() {
    let (status, stdout, stderr, _) = run(&["%e", "infiny"]);
    assert_eq!((status, stdout.as_str()), (EXECUTION_FAILURE, "inf"));
    assert!(stderr.contains("infiny: invalid number"));
    let (status, stdout, stderr, _) = run(&["%g", "naN"]);
    assert_eq!((status, stdout.as_str()), (EXECUTION_SUCCESS, "nan"));
    assert!(stderr.is_empty(), "{stderr:?}");
    // Non-words are untouched: still 0 with a diagnostic.
    for operand in ["in", "na"] {
        let (status, stdout, _, _) = run(&["%f", operand]);
        assert_eq!(stdout, "0.000000", "operand {operand:?}");
        assert_eq!(status, EXECUTION_FAILURE, "operand {operand:?}");
    }
    // Empty operand stays 0 with a diagnostic (matches GNU, unchanged).
    let (status, stdout, stderr, _) = run(&["%f", ""]);
    assert_eq!((status, stdout.as_str()), (EXECUTION_FAILURE, "0.000000"));
    assert!(stderr.contains(": invalid number"));
}
