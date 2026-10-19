//! Issue rubash#860: the unset empty-span/mask pseudo-diagnostic family
//! (the #847 CR residual a), plus the empty-span WRITE family.
//!
//! GNU bash 5.3.0 baseline (WSL oracle, byte-verified probes 2026-10-19; the
//! full differential is on the PR):
//!
//! 1. mask-before-expand: GNU's expand_word_internal scans left to right, so
//!    in `'A4[$x\ty]'` the `$name` extraction stops at the backslash
//!    (subst.c:10934 param_expand's legal_variable_char loop) and the
//!    unquoted `\t` collapses to `t` AFTER it — the deferred unset pass
//!    (arrayfunc.c:1169 expand_subscript_string) resolves key `ty` and keeps
//!    silent rc0. rubash masked the escape first, folding `$x\ty` into the
//!    variable name `$xty` (empty) and reporting a bogus
//!    `[...]: bad array subscript` + rc1.
//! 2. An escape product is DATA for the `$name` scan: `S6[$\tx]` keys on the
//!    literal text `$tx` (24 74 78), not on the expansion of `tx`.
//! 3. Empty-span writes: `((U8[$'']=1))` / `U7[$'']=1` / `let "C9['']=1"`
//!    reject the EMPTY key with ONE `name['']` diagnostic (arrayfunc.c
//!    err_badarraysub; the `((`-empty-token spellings `""`/`$u`/`[]` take
//!    the `` `name[]': not a valid identifier `` law instead) and store
//!    nothing (assign_array_element_internal arrayfunc.c:402-407). Reads of
//!    a `['']` spelling diagnose the bare NAME once (INDEX_ERROR,
//!    arrayfunc.c:1462-1470); `[]`-class reads keep the pinned twice-`a[]`
//!    law.
//!
//! Fix: `mask_subscript_escapes` puts an escaped name-character behind the
//! PARAM_NAME_END_MARKER boundary (#742's law), and the arith pre-encode
//! pass routes the empty resolution into the token-text class split
//! (`ArithLValue::AssocEmptyKey`).

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

/// Ticket headline: `unset 'A4[$x\ty]'` with x unset — GNU keeps silent rc0
/// and the declared-unset assoc stays invisible.
#[test]
fn unset_mask_empty_span_keeps_silent() {
    let (stdout, stderr, rc) =
        rubash("declare -A A4\nunset 'A4[$x\\ty]'\necho \"rc=$?\"\ndeclare -p A4");
    assert_eq!(stderr, "", "stderr not empty: {stderr}");
    assert_eq!(rc, Some(0));
    assert_eq!(stdout, "rc=0\ndeclare -A A4\n");
}

/// The deferred pass expands `$x` and stops the name at the backslash:
/// with x=Q the key is `Qty` — that cell is deleted, `xty` survives.
#[test]
fn unset_mask_resolves_name_ended_by_backslash() {
    let (stdout, stderr, rc) = rubash(
        "x=Q\ndeclare -A B4; B4[Qty]=1; B4[xty]=2\nunset 'B4[$x\\ty]'\necho \"rc=$?\"\ndeclare -p B4",
    );
    assert_eq!(stderr, "", "stderr not empty: {stderr}");
    assert_eq!(rc, Some(0));
    assert_eq!(stdout, "rc=0\ndeclare -A B4=([xty]=\"2\" )\n");
}

/// Same mask law on the reader: `${R4[$x\ty]}` with x=Q hits the `Qty` cell.
#[test]
fn reader_mask_boundary_hits_stored_cell() {
    let (stdout, stderr, _) =
        rubash("x=Q; declare -A R4; R4[Qty]=5\nprintf '[%s]\\n' \"${R4[$x\\ty]}\"");
    assert_eq!(stderr, "", "stderr not empty: {stderr}");
    assert_eq!(stdout, "[5]\n");
}

/// An escape product directly after `$` is data, not a name start: the key
/// is the literal `$tx` (GNU 24 74 78), even with `tx` set. The `tr -d ' \n'`
/// eats od's newline, so the listing has no trailing newline (GNU face).
#[test]
fn escaped_char_stays_out_of_dollar_name() {
    let (stdout, stderr, _) = rubash(
        "tx=ZZ; declare -A S6; S6[$\\tx]=1\nprintf '%s' \"${!S6[@]}\" | od -An -tx1 | tr -d ' \\n'",
    );
    assert_eq!(stderr, "", "stderr not empty: {stderr}");
    assert_eq!(stdout, "247478");
}

/// `((U8[$'']=1))`: GNU rejects the empty key with ONE `U8['']` diagnostic
/// (the span was decoded and sh_single_quote-requoted at parse) and stores
/// nothing — the declared assoc stays empty-visible.
#[test]
fn arith_empty_span_write_rejected() {
    let (stdout, stderr, rc) =
        rubash("declare -A U8; ((U8[$'']=1)); echo \"rc=$?\"; declare -p U8");
    assert_eq!(
        stderr, "bash: line 1: U8['']: bad array subscript\n",
        "wrong stderr"
    );
    assert_eq!(rc, Some(0));
    assert_eq!(stdout, "rc=0\ndeclare -A U8\n");
}

/// The literal `''` spelling takes the same law (`C2['']`, once, no store).
#[test]
fn arith_empty_sq_write_rejected() {
    let (stdout, stderr, _) = rubash("declare -A C2; ((C2['']=1)); declare -p C2");
    assert_eq!(stderr, "bash: line 1: C2['']: bad array subscript\n");
    assert_eq!(stdout, "declare -A C2\n");
}

/// The dq/param/empty token spellings take the `name[]` law instead:
/// `` `C3[]': not a valid identifier `` at the bind, once.
#[test]
fn arith_empty_token_write_invalid_identifier() {
    let (stderr, rc) = {
        let (o, e, c) = rubash("declare -A C3; ((C3[\"\"]=1)); echo rc=$?");
        assert_eq!(o, "rc=0\n");
        (e, c)
    };
    assert_eq!(stderr, "bash: line 1: ((: `C3[]': not a valid identifier\n");
    assert_eq!(rc, Some(0));
}

/// Plain assignment `U7[$'']=1`: the diagnostic names the parse-cooked
/// subscript `U7['']`, nothing is stored, and GNU's `declare -p U7` goes
/// silent afterwards (byte-verified WSL oracle: empty stdout, one stderr
/// line, rc 1) — rubash matches that face.
#[test]
fn plain_empty_span_write_rejected() {
    let (stdout, stderr, _) = rubash("declare -A U7; U7[$'']=1; declare -p U7");
    assert_eq!(stderr, "bash: line 1: U7['']: bad array subscript\n");
    assert_eq!(stdout, "");
}

/// `let "C9['']=1"`: same empty-key law under the EXP_EXPANDED entry.
#[test]
fn let_empty_sq_write_rejected() {
    let (stdout, stderr, _) = rubash("declare -A C9; let \"C9['']=1\"; declare -p C9");
    assert_eq!(stderr, "bash: line 1: C9['']: bad array subscript\n");
    assert_eq!(stdout, "declare -A C9\n");
}

/// `let "Ca[$u]=1"` with u empty: the empty-token law under let's own label.
#[test]
fn let_empty_param_write_invalid_identifier() {
    let (_, stderr, _) = rubash("u=\ndeclare -A Ca; let \"Ca[$u]=1\"");
    assert_eq!(
        stderr, "bash: line 2: let: `Ca[]': not a valid identifier\n",
        "wrong stderr"
    );
}

/// `((Cd[$u]+=1))`: the compound-operator read diagnoses `Cd[]` twice, the
/// bind adds the identifier failure, and nothing is stored.
#[test]
fn arith_plus_eq_empty_param_no_store() {
    let (stdout, stderr, _) = rubash("u=\ndeclare -A Cd; Cd[x]=5; ((Cd[$u]+=1)); declare -p Cd");
    assert_eq!(
        stderr,
        "bash: line 2: Cd[]: bad array subscript\n\
         bash: line 2: Cd[]: bad array subscript\n\
         bash: line 2: ((: `Cd[]': not a valid identifier\n"
    );
    assert_eq!(stdout, "declare -A Cd=([x]=\"5\" )\n");
}

/// `((Cc[$'']+=1))`: the compound-operator read diagnoses the bare name
/// (INDEX_ERROR) and the bind diagnoses `Cc['']`; no second cell.
#[test]
fn arith_plus_eq_empty_span_two_diags_no_store() {
    let (stdout, stderr, _) = rubash("declare -A Cc; Cc[x]=5; ((Cc[$'']+=1)); declare -p Cc");
    assert_eq!(
        stderr,
        "bash: line 1: Cc: bad array subscript\n\
         bash: line 1: Cc['']: bad array subscript\n"
    );
    assert_eq!(stdout, "declare -A Cc=([x]=\"5\" )\n");
}

/// Reads: a `['']` spelling diagnoses the bare name ONCE (INDEX_ERROR,
/// arrayfunc.c:1462-1470) and yields 0.
#[test]
fn arith_empty_span_read_bare_name_once() {
    let (stdout, stderr, _) = rubash("declare -A C; C[x]=1\nz=$((C[$'']))\necho \"$z\"");
    assert_eq!(stderr, "bash: line 2: C: bad array subscript\n");
    assert_eq!(stdout, "0\n");
}

/// Reads: the `[]`-class token keeps the pinned twice law (unchanged).
#[test]
fn arith_empty_token_read_twice_law() {
    let (stdout, stderr, _) = rubash("u=\ndeclare -A C4\nv=$((C4[$u]))\necho \"$v\"");
    assert_eq!(
        stderr,
        "bash: line 3: C4[]: bad array subscript\n\
         bash: line 3: C4[]: bad array subscript\n"
    );
    assert_eq!(stdout, "0\n");
}

/// The multi-span token text is preserved verbatim in the diagnostic
/// (`''''''` = two empty spans rides the arith token as syntax; GNU keeps
/// all six quotes in the `['''''']` word).
#[test]
fn arith_multi_span_token_text_preserved() {
    let (_, stderr, _) = rubash("declare -A De; ((De['''''']=1))");
    assert_eq!(stderr, "bash: line 1: De['''''']: bad array subscript\n");
}

/// Non-regression (the #828 family): a NONEMPTY span still stores the clean
/// decoded key and the variable spelling reads it back.
#[test]
fn nonempty_span_write_still_clean() {
    let (stdout, stderr, _) =
        rubash("k=$'x\\ty'; declare -A A; ((A[$'x\\ty']=1)); printf '[%s]\\n' \"${A[$k]}\"");
    assert_eq!(stderr, "", "stderr not empty: {stderr}");
    assert_eq!(stdout, "[1]\n");
}

/// Non-regression (the #847 family): the unset deferred operand pass still
/// quote-removes a quoted-source span (`$` literal, content verbatim), so
/// the `$'...'-written cell SURVIVES (WSL oracle: count stays 1). The raw
/// `od -An -tx1` listing of the surviving key `x\ty` is ` 78 09 79\n`.
#[test]
fn unset_deferred_span_quote_removal_unchanged() {
    let (stdout, stderr, _) = rubash(
        "declare -A A1; ((A1[$'x\\ty']=1)); unset 'A1[$'\"'\"'x\\ty'\"'\"']'; printf '%s' \"${!A1[@]}\" | od -An -tx1",
    );
    assert_eq!(stderr, "", "stderr not empty: {stderr}");
    assert_eq!(stdout, " 78 09 79\n");
}
