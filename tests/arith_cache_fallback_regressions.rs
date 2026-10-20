//! Arithmetic cache-fallback regressions (GNU bash 5.3 baseline,
//! byte-verified against local bash 5.3.20 2026-10-20; WSL 5.3.0 re-check
//! by the main session).
//!
//! These pin the value semantics a parse cache must NOT change (landed
//! alongside `fix/arith-parse-cache`, which caches `CmpLT` + `PostInc`
//! parses in `src/executor/arithmetic/mod.rs`):
//!
//! - dynamic names (`RANDOM`) re-evaluate on every expansion;
//! - assoc cells read numerically but keep their literal spelling;
//! - plain scalar reads never write back, while compound assignment and
//!   pre/post-inc do;
//! - assoc subscripts are literal strings (never arithmetic);
//! - `08` is a base error in arithmetic but decimal 8 in `test`;
//! - `set -u` on an unbound name inside arithmetic is fatal (kept LAST so
//!   a harness that sources rather than forks stays unpolluted).

use std::process::Command;

fn rubash(script: &str) -> (String, String, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .env("LC_ALL", "C.UTF-8")
        .output()
        .expect("run rubash");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

/// A1: `RANDOM` re-evaluates per expansion — ten draws are never all equal.
#[test]
fn a1_random_is_never_cached() {
    let (stdout, stderr, _) = rubash(
        "seen=''; same=1; prev=''; \
         for i in 1 2 3 4 5 6 7 8 9 10; do \
           v=$RANDOM; \
           if [ \"$prev\" != '' ] && [ \"$v\" != \"$prev\" ]; then same=0; fi; \
           prev=$v; \
         done; \
         echo \"same=$same\"",
    );
    assert_eq!(stderr, "", "{stderr:?}");
    assert_eq!(stdout, "same=0\n", "ten RANDOM draws must vary");
}

/// A2: assoc cells participate numerically (`3+4` + 1 == 8) while the
/// stored spelling stays literal.
#[test]
fn a2_assoc_cell_reads_numeric_keeps_spelling() {
    let (stdout, stderr, _) =
        rubash("declare -A A; A[k]='3+4'; echo \"sum=$(( A[k] + 1 ))\"; echo \"kept=${A[k]}\"");
    assert_eq!(stderr, "", "{stderr:?}");
    assert_eq!(stdout, "sum=8\nkept=3+4\n");
}

/// A4: a plain `$(( x ))` read evaluates without writing back, while
/// `(( s += 1 ))` evaluates AND writes back.
#[test]
fn a4_scalar_read_has_no_writeback_but_compound_assign_does() {
    let (stdout, stderr, _) = rubash("x='1+2'; echo \"val=$(( x ))\"; echo \"x=$x\"");
    assert_eq!(stderr, "", "{stderr:?}");
    assert_eq!(stdout, "val=3\nx=1+2\n");

    let (stdout, stderr, _) = rubash("s='1+2'; (( s += 1 )); echo \"s=$s\"");
    assert_eq!(stderr, "", "{stderr:?}");
    assert_eq!(stdout, "s=4\n");

    let (stdout, stderr, _) = rubash("z='1+2'; echo \"v=$(( z++ ))\"; echo \"z=$z\"");
    assert_eq!(stderr, "", "{stderr:?}");
    assert_eq!(stdout, "v=3\nz=4\n");
}

/// A5: assoc subscripts are literal strings — `1+2` never folds to `3`.
#[test]
fn a5_assoc_subscript_is_literal() {
    let (stdout, stderr, _) =
        rubash("declare -A C; C[1+2]=hit; echo \"lit=${C[1+2]}\"; echo \"num=${C[3]:-EMPTY}\"");
    assert_eq!(stderr, "", "{stderr:?}");
    assert_eq!(stdout, "lit=hit\nnum=EMPTY\n");
}

/// A6: `08`对照 — arithmetic rejects it as octal (`value too great for
/// base`), while `test` reads it as decimal 8.
#[test]
fn a6_leading_zero_octal_vs_test_decimal() {
    let (stdout, stderr, code) = rubash("echo $(( 08 ))");
    assert!(
        stderr.contains("value too great for base"),
        "arith must diagnose the bad octal: {stderr:?}"
    );
    assert!(stdout.is_empty(), "{stdout:?}");
    assert_eq!(code, Some(1));

    let (stdout, stderr, _) = rubash("test 08 -eq 8; echo \"rc=$?\"");
    assert_eq!(stderr, "", "{stderr:?}");
    assert_eq!(stdout, "rc=0\n");
}

/// A7: numeric-looking assoc keys stay distinct strings (`01` vs `1`),
/// and `+=` on an assoc cell concatenates (`1` + `2` == `12`).
#[test]
fn a7_numeric_assoc_keys_stay_strings() {
    let (stdout, stderr, _) =
        rubash("declare -A D; D[01]=a; D[1]=b; echo \"01=${D[01]} one=${D[1]}\"");
    assert_eq!(stderr, "", "{stderr:?}");
    assert_eq!(stdout, "01=a one=b\n");

    let (stdout, stderr, _) = rubash("declare -A E; E[x]=1; E[x]+=2; echo \"c=${E[x]}\"");
    assert_eq!(stderr, "", "{stderr:?}");
    assert_eq!(stdout, "c=12\n");
}

/// A3 (LAST): an unbound name inside arithmetic under `set -u` is fatal —
/// `unbound variable` on stderr, nonzero exit. Kept last so a sourcing
/// harness never inherits `set -u`.
#[test]
fn a3_nounset_unbound_in_arith_is_fatal() {
    let (stdout, stderr, code) =
        rubash("set -u; echo $(( NO_SUCH_VAR_XYZ_999 + 1 )); echo SURVIVED");
    assert!(
        stderr.contains("unbound variable"),
        "nounset must diagnose: {stderr:?}"
    );
    assert_eq!(stdout, "", "the shell must die before echo: {stdout:?}");
    assert_ne!(code, Some(0));
}
