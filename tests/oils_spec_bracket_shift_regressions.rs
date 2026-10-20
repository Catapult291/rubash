//! Oils-spec vendor: spec/builtin-bracket.test.sh.
//!
//! Vendored from the Oils shell specification suite (Apache License 2.0):
//!   https://github.com/oils-for-unix/oils/blob/master/spec/builtin-bracket.test.sh
//!   https://github.com/oils-for-unix/oils/blob/master/spec/toysh-posix.test.sh (shift-is-fatal case)
//!   https://github.com/oils-for-unix/oils/blob/master/spec/builtin-misc.test.sh (shift / shifting-too-far / invalid-shift cases)
//!   https://github.com/oils-for-unix/oils/blob/master/spec/arg-parse.test.sh (shift-extra-arg case)
//!
//! Transcription rules (see task): the `#### case` / `## STDOUT` / `## OK`
//! blocks were rewritten as `Command::new(env!("CARGO_BIN_EXE_rubash"))`
//! assertions in this file's style. `argv.py` prints became
//! `printf '<%s>\n'` lines and `$TMP` fixtures became sandbox-local files.
//! Expected values were measured with the local GNU bash 5.3.20
//! (`bash -c`, file-free sandbox, `LC_ALL=C`) — never copied from an oils
//! `## OK` line, which may record oils behavior instead of bash behavior.
//! Only cases where rubash already matches bash byte-for-byte are kept;
//! the divergences found while probing are listed in the task report.

use std::fs;
use std::process::Command;

fn sandbox(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rubash-oils-{}-{}-{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos(),
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create sandbox dir");
    dir
}

fn run(script: &str) -> (i32, String, String) {
    let dir = sandbox("case");
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .current_dir(&dir)
        .env_remove("BASH_ENV")
        .env_remove("ENV")
        .env("HOME", &dir)
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .env("TMPDIR", &dir)
        .output()
        .expect("spawn rubash");
    let _ = fs::remove_dir_all(&dir);
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn shift_basic() {
    // Oils `####` case: shift/basic
    let (status, stdout, stderr) = run(r#"set -- 1 2 3 4
shift
echo "$@"
shift 2
echo "$@""#);
    assert_eq!(status, 0, "shift/basic status");
    assert_eq!(
        stdout,
        r"2 3 4
4
",
        "shift/basic stdout"
    );
    assert_eq!(stderr, r"", "shift/basic stderr");
}

#[test]
fn shift_invalid_arg() {
    // Oils `####` case: shift/invalid-arg
    let (status, stdout, stderr) = run(r"shift ZZZ");
    assert_eq!(status, 2, "shift/invalid-arg status");
    assert_eq!(stdout, r"", "shift/invalid-arg stdout");
    assert_eq!(
        stderr,
        r"bash: line 1: shift: ZZZ: numeric argument required
",
        "shift/invalid-arg stderr"
    );
}

#[test]
fn shift_too_far() {
    // Oils `####` case: shift/too-far
    let (status, stdout, stderr) = run(r"set -- 1
shift 2");
    assert_eq!(status, 1, "shift/too-far status");
    assert_eq!(stdout, r"", "shift/too-far stdout");
    assert_eq!(stderr, r"", "shift/too-far stderr");
}

#[test]
fn shift_top_level_fresh() {
    // Oils `####` case: shift/top-level-fresh
    let (status, stdout, stderr) = run(r"shift; echo hello");
    assert_eq!(status, 0, "shift/top-level-fresh status");
    assert_eq!(
        stdout,
        r"hello
",
        "shift/top-level-fresh stdout"
    );
    assert_eq!(stderr, r"", "shift/top-level-fresh stderr");
}

#[test]
fn test_a_alias_e() {
    // Oils `####` case: test/a-alias-e
    let (status, stdout, stderr) = run(r"[ -a / ]
echo status=$?
[ -a /nonexistent ]
echo status=$?");
    assert_eq!(status, 0, "test/a-alias-e status");
    assert_eq!(
        stdout,
        r"status=0
status=1
",
        "test/a-alias-e stdout"
    );
    assert_eq!(stderr, r"", "test/a-alias-e stderr");
}

#[test]
fn test_a_eq_noninteger() {
    // Oils `####` case: test/a-eq-noninteger
    let (status, stdout, stderr) = run(r"[ a -eq a ]
echo status=$?");
    assert_eq!(status, 0, "test/a-eq-noninteger status");
    assert_eq!(
        stdout,
        r"status=2
",
        "test/a-eq-noninteger stdout"
    );
    assert_eq!(
        stderr,
        r"bash: line 1: [: a: integer expected
",
        "test/a-eq-noninteger stderr"
    );
}

#[test]
fn test_ambiguity_and() {
    // Oils `####` case: test/ambiguity-and
    let (status, stdout, stderr) = run(r"[ -z -a -a ]
echo status=$?");
    assert_eq!(status, 0, "test/ambiguity-and status");
    assert_eq!(
        stdout,
        r"status=0
",
        "test/ambiguity-and stdout"
    );
    assert_eq!(stderr, r"", "test/ambiguity-and stderr");
}

#[test]
fn test_ambiguity_bracket() {
    // Oils `####` case: test/ambiguity-bracket
    let (status, stdout, stderr) = run(r"[ -z -a ] ]
echo status=$?");
    assert_eq!(status, 0, "test/ambiguity-bracket status");
    assert_eq!(
        stdout,
        r"status=0
",
        "test/ambiguity-bracket stdout"
    );
    assert_eq!(stderr, r"", "test/ambiguity-bracket stderr");
}

#[test]
fn test_bracket_syntax_errors() {
    // Oils `####` case: test/bracket-syntax-errors
    let (status, stdout, stderr) = run(r"[
echo status=$?
test
echo status=$?
[ -n x
echo status=$?
[ -n x ] y
echo status=$?
[ -n x y
echo status=$?");
    assert_eq!(status, 0, "test/bracket-syntax-errors status");
    assert_eq!(
        stdout,
        r"status=2
status=1
status=2
status=2
status=2
",
        "test/bracket-syntax-errors stdout"
    );
    assert_eq!(
        stderr,
        r"bash: line 1: [: missing `]'
bash: line 5: [: missing `]'
bash: line 7: [: missing `]'
bash: line 9: [: missing `]'
",
        "test/bracket-syntax-errors stderr"
    );
}

#[test]
fn test_bug_regression_ipv6() {
    // Oils `####` case: test/bug-regression-ipv6
    let (status, stdout, stderr) = run(r#"test "$ipv6" = "yes" -a "$ipv6lib" != "none"
echo status=$?"#);
    assert_eq!(status, 0, "test/bug-regression-ipv6 status");
    assert_eq!(
        stdout,
        r"status=1
",
        "test/bug-regression-ipv6 stdout"
    );
    assert_eq!(stderr, r"", "test/bug-regression-ipv6 stderr");
}

#[test]
fn test_compare_literal_dash_f() {
    // Oils `####` case: test/compare-literal-dash-f
    let (status, stdout, stderr) = run(r"var=-f
[ $var = -f ] && echo true
[ '-f' = $var ] && echo true");
    assert_eq!(status, 0, "test/compare-literal-dash-f status");
    assert_eq!(
        stdout,
        r"true
true
",
        "test/compare-literal-dash-f stdout"
    );
    assert_eq!(stderr, r"", "test/compare-literal-dash-f stderr");
}

#[test]
fn test_dash_d_file() {
    // Oils `####` case: test/dash-d-file
    let (status, stdout, stderr) = run(r"mkdir -p sandbox_d
test -d sandbox_d
echo status=$?
test -d sandbox_d/__nonexistent_Z_Z__
echo status=$?");
    assert_eq!(status, 0, "test/dash-d-file status");
    assert_eq!(
        stdout,
        r"status=0
status=1
",
        "test/dash-d-file stdout"
    );
    assert_eq!(stderr, r"", "test/dash-d-file stderr");
}

#[test]
fn test_dash_ef() {
    // Oils `####` case: test/dash-ef
    let (status, stdout, stderr) = run(r"touch left_f right_f
ln -f left_f hardlink_f
test left_f -ef left_f && echo same
test left_f -ef hardlink_f && echo same
test left_f -ef right_f || echo different
test __nonexistent -ef right_f || echo different");
    assert_eq!(status, 0, "test/dash-ef status");
    assert_eq!(
        stdout,
        r"same
different
different
",
        "test/dash-ef stdout"
    );
    assert_eq!(
        stderr,
        r"ln: failed to create hard link 'hardlink_f' => 'left_f': Permission denied
",
        "test/dash-ef stderr"
    );
}

#[test]
fn test_dash_s_file() {
    // Oils `####` case: test/dash-s-file
    let (status, stdout, stderr) = run(r"test -s __nonexistent
echo status=$?
: > empty_f
test -s empty_f
echo status=$?
echo nonempty > nonempty_f
test -s nonempty_f
echo status=$?");
    assert_eq!(status, 0, "test/dash-s-file status");
    assert_eq!(
        stdout,
        r"status=1
status=1
status=0
",
        "test/dash-s-file stdout"
    );
    assert_eq!(stderr, r"", "test/dash-s-file stderr");
}

#[test]
fn test_dash_symlink() {
    // Oils `####` case: test/dash-symlink
    let (status, stdout, stderr) = run(r"mkdir -p linkdir
touch linkdir/zz
ln -s -f linkdir/zz linkdir/symlink
ln -s -f linkdir/__nonexistent_ZZ__ linkdir/dangling
test -L linkdir/zz || echo no
test -h linkdir/zz || echo no
test -f linkdir/symlink && echo is-file
test -L linkdir/symlink && echo symlink
test -h linkdir/symlink && echo symlink
test -L linkdir/dangling && echo dangling
test -h linkdir/dangling  && echo dangling
test -f linkdir/dangling  || echo 'dangling is not file'");
    assert_eq!(status, 0, "test/dash-symlink status");
    assert_eq!(
        stdout,
        r"no
no
symlink
symlink
dangling
dangling
dangling is not file
",
        "test/dash-symlink stdout"
    );
    assert_eq!(stderr, r"", "test/dash-symlink stderr");
}

#[test]
fn test_eq_alias() {
    // Oils `####` case: test/eq-alias
    let (status, stdout, stderr) = run(r"[ a = a ] && echo true
[ a == a ] && echo true");
    assert_eq!(status, 0, "test/eq-alias status");
    assert_eq!(
        stdout,
        r"true
true
",
        "test/eq-alias stdout"
    );
    assert_eq!(stderr, r"", "test/eq-alias stderr");
}

#[test]
fn test_eq_no_glob() {
    // Oils `####` case: test/eq-no-glob
    let (status, stdout, stderr) = run(r"[ abc = 'a*' ]
echo status=$?
[ abc == 'a*' ]
echo status=$?");
    assert_eq!(status, 0, "test/eq-no-glob status");
    assert_eq!(
        stdout,
        r"status=1
status=1
",
        "test/eq-no-glob stdout"
    );
    assert_eq!(stderr, r"", "test/eq-no-glob stderr");
}

#[test]
fn test_extra_args() {
    // Oils `####` case: test/extra-args
    let (status, stdout, stderr) = run(r"test -n x ]
echo status=$?
test -n x y
echo status=$?");
    assert_eq!(status, 0, "test/extra-args status");
    assert_eq!(
        stdout,
        r"status=2
status=2
",
        "test/extra-args stdout"
    );
    assert_eq!(
        stderr,
        r"bash: line 1: test: x: binary operator expected
bash: line 3: test: x: binary operator expected
",
        "test/extra-args stderr"
    );
}

#[test]
fn test_four_args() {
    // Oils `####` case: test/four-args
    let (status, stdout, stderr) = run(r"[ ! foo = foo ]
echo status=$?
[ \( -z foo \) ]
echo status=$?");
    assert_eq!(status, 0, "test/four-args status");
    assert_eq!(
        stdout,
        r"status=1
status=1
",
        "test/four-args stdout"
    );
    assert_eq!(stderr, r"", "test/four-args stderr");
}

#[test]
fn test_looks_like_octal() {
    // Oils `####` case: test/looks-like-octal
    let (status, stdout, stderr) = run(r"echo $(( 083 ))
echo status=$?
echo $(( -083 ))
echo status=$?
echo
[ 083 -eq 83 ]
echo status=$?
[ -083 -eq -83 ]
echo status=$?");
    assert_eq!(status, 0, "test/looks-like-octal status");
    assert_eq!(
        stdout,
        r"status=1
status=1

status=0
status=0
",
        "test/looks-like-octal stdout"
    );
    assert_eq!(
        stderr,
        r#"bash: line 1: 083: value too great for base (error token is "083")
bash: line 3: -083: value too great for base (error token is "083")
"#,
        "test/looks-like-octal stderr"
    );
}

#[test]
fn test_lparen_foo_runtime_error() {
    // Oils `####` case: test/lparen-foo-runtime-error
    let (status, stdout, stderr) = run(r"[ '(' foo ]
echo status=$?");
    assert_eq!(status, 0, "test/lparen-foo-runtime-error status");
    assert_eq!(
        stdout,
        r"status=2
",
        "test/lparen-foo-runtime-error stdout"
    );
    assert_eq!(
        stderr,
        r"bash: line 1: [: (: unary operator expected
",
        "test/lparen-foo-runtime-error stderr"
    );
}

#[test]
fn test_n_flag() {
    // Oils `####` case: test/n-flag
    let (status, stdout, stderr) = run(r"test -n 'a'  && echo true
test -n ''   || echo false");
    assert_eq!(status, 0, "test/n-flag status");
    assert_eq!(
        stdout,
        r"true
false
",
        "test/n-flag stdout"
    );
    assert_eq!(stderr, r"", "test/n-flag stderr");
}

#[test]
fn test_negative_numbers() {
    // Oils `####` case: test/negative-numbers
    let (status, stdout, stderr) = run(r"[ $((-42)) -le 0 ]
echo status=$?
[ $((-1)) -le 0 ]
echo status=$?
echo
[ -1 -le 0 ]
echo status=$?
[ -42 -le 0 ]
echo status=$?
echo
test -1 -le 0
echo status=$?
test -42 -le 0
echo status=$?");
    assert_eq!(status, 0, "test/negative-numbers status");
    assert_eq!(
        stdout,
        r"status=0
status=0

status=0
status=0

status=0
status=0
",
        "test/negative-numbers stdout"
    );
    assert_eq!(stderr, r"", "test/negative-numbers stderr");
}

#[test]
fn test_negative_octal_etc() {
    // Oils `####` case: test/negative-octal-etc
    let (status, stdout, stderr) = run(r"[ -0 -eq 0 ]
echo zero=$?
[ -0123 -eq -83 ]
echo octal=$?
[ -0xff -eq -255 ]
echo hex=$?
[ -64#a -eq -10 ]
echo baseN=$?");
    assert_eq!(status, 0, "test/negative-octal-etc status");
    assert_eq!(
        stdout,
        r"zero=0
octal=1
hex=2
baseN=2
",
        "test/negative-octal-etc stdout"
    );
    assert_eq!(
        stderr,
        r"bash: line 5: [: -0xff: integer expected
bash: line 7: [: -64#a: integer expected
",
        "test/negative-octal-etc stderr"
    );
}

#[test]
fn test_no_octal_in_bracket() {
    // Oils `####` case: test/no-octal-in-bracket
    let (status, stdout, stderr) = run(r"echo $(( 073 ))
echo $(( -073 ))
echo
[ 073 -eq 73 ]
echo status=$?
[ -073 -eq -73 ]
echo status=$?
echo
[ 0xff -eq 255 ]
echo hex=$?
[ 64#a -eq 10 ]
echo baseN=$?");
    assert_eq!(status, 0, "test/no-octal-in-bracket status");
    assert_eq!(
        stdout,
        r"59
-59

status=0
status=0

hex=2
baseN=2
",
        "test/no-octal-in-bracket stdout"
    );
    assert_eq!(
        stderr,
        r"bash: line 9: [: 0xff: integer expected
bash: line 11: [: 64#a: integer expected
",
        "test/no-octal-in-bracket stderr"
    );
}

#[test]
fn test_no_recursive_arith() {
    // Oils `####` case: test/no-recursive-arith
    let (status, stdout, stderr) = run(r#"[ 1+2 -eq 3 ]
echo status=$?
s='1+2'
[ "$s" -eq 3 ]
echo status=$?"#);
    assert_eq!(status, 0, "test/no-recursive-arith status");
    assert_eq!(
        stdout,
        r"status=2
status=2
",
        "test/no-recursive-arith stdout"
    );
    assert_eq!(
        stderr,
        r"bash: line 1: [: 1+2: integer expected
bash: line 4: [: 1+2: integer expected
",
        "test/no-recursive-arith stderr"
    );
}

#[test]
fn test_not_and() {
    // Oils `####` case: test/not-and
    let (status, stdout, stderr) = run(r"[ -z '' -a ! -z x ]
echo status=$?");
    assert_eq!(status, 0, "test/not-and status");
    assert_eq!(
        stdout,
        r"status=0
",
        "test/not-and stdout"
    );
    assert_eq!(stderr, r"", "test/not-and stderr");
}

#[test]
fn test_nt_ot_missing() {
    // Oils `####` case: test/nt-ot-missing
    let (status, stdout, stderr) = run(r"[ present -nt absent ] || exit 1
[ absent -ot present ] || exit 2");
    assert_eq!(status, 1, "test/nt-ot-missing status");
    assert_eq!(stdout, r"", "test/nt-ot-missing stdout");
    assert_eq!(stderr, r"", "test/nt-ot-missing stderr");
}

#[test]
fn test_o_option() {
    // Oils `####` case: test/o-option
    let (status, stdout, stderr) = run(r"test -o nounset
echo status=$?
set -o nounset
test -o nounset
echo status=$?
test -o _bad_name_
echo status=$?");
    assert_eq!(status, 0, "test/o-option status");
    assert_eq!(
        stdout,
        r"status=1
status=0
status=1
",
        "test/o-option stdout"
    );
    assert_eq!(stderr, r"", "test/o-option stderr");
}

#[test]
fn test_one_arg_empty() {
    // Oils `####` case: test/one-arg-empty
    let (status, stdout, stderr) = run(r"test 'a'  && echo true
test ''   || echo false");
    assert_eq!(status, 0, "test/one-arg-empty status");
    assert_eq!(
        stdout,
        r"true
false
",
        "test/one-arg-empty stdout"
    );
    assert_eq!(stderr, r"", "test/one-arg-empty stderr");
}

#[test]
fn test_one_arg_specials() {
    // Oils `####` case: test/one-arg-specials
    let (status, stdout, stderr) = run(r"[ = ]
echo status=$?
[ ] ]
echo status=$?
[ '!' ]
echo status=$?
[ '(' ]
echo status=$?");
    assert_eq!(status, 0, "test/one-arg-specials status");
    assert_eq!(
        stdout,
        r"status=0
status=0
status=0
status=0
",
        "test/one-arg-specials stdout"
    );
    assert_eq!(stderr, r"", "test/one-arg-specials stderr");
}

#[test]
fn test_op_variable() {
    // Oils `####` case: test/op-variable
    let (status, stdout, stderr) = run(r"op='='
[ a $op a ] && echo true
[ a $op b ] || echo false");
    assert_eq!(status, 0, "test/op-variable status");
    assert_eq!(
        stdout,
        r"true
false
",
        "test/op-variable stdout"
    );
    assert_eq!(stderr, r"", "test/op-variable stderr");
}

#[test]
fn test_or() {
    // Oils `####` case: test/or
    let (status, stdout, stderr) = run(r"[ -z x -o ! -z x ]
echo status=$?");
    assert_eq!(status, 0, "test/or status");
    assert_eq!(
        stdout,
        r"status=0
",
        "test/or stdout"
    );
    assert_eq!(stderr, r"", "test/or stderr");
}

#[test]
fn test_overflow_t() {
    // Oils `####` case: test/overflow-t
    let (status, stdout, stderr) = run(r"test -t 12345678910
echo status=$?");
    assert_eq!(status, 0, "test/overflow-t status");
    assert_eq!(
        stdout,
        r"status=1
",
        "test/overflow-t stdout"
    );
    assert_eq!(stderr, r"", "test/overflow-t stderr");
}

#[test]
fn test_parens() {
    // Oils `####` case: test/parens
    let (status, stdout, stderr) = run(r"[ -z '' -a '(' ! -z x ')' ]
echo status=$?");
    assert_eq!(status, 0, "test/parens status");
    assert_eq!(
        stdout,
        r"status=0
",
        "test/parens stdout"
    );
    assert_eq!(stderr, r"", "test/parens stderr");
}

#[test]
fn test_t_invalid() {
    // Oils `####` case: test/t-invalid
    let (status, stdout, stderr) = run(r"[ -t invalid ]
echo status=$?");
    assert_eq!(status, 0, "test/t-invalid status");
    assert_eq!(
        stdout,
        r"status=2
",
        "test/t-invalid stdout"
    );
    assert_eq!(
        stderr,
        r"bash: line 1: [: invalid: integer expected
",
        "test/t-invalid stderr"
    );
}

#[test]
fn test_three_args() {
    // Oils `####` case: test/three-args
    let (status, stdout, stderr) = run(r"[ foo = '' ]
echo status=$?
[ foo -a '' ]
echo status=$?
[ foo -o '' ]
echo status=$?
[ ! -z foo ]
echo status=$?
[ \( foo \) ]
echo status=$?");
    assert_eq!(status, 0, "test/three-args status");
    assert_eq!(
        stdout,
        r"status=1
status=1
status=0
status=0
status=0
",
        "test/three-args stdout"
    );
    assert_eq!(stderr, r"", "test/three-args stderr");
}

#[test]
fn test_two_args_z() {
    // Oils `####` case: test/two-args-z
    let (status, stdout, stderr) = run(r"[ -z = ]
echo status=$?
[ -z ] ]
echo status=$?
[ -z '!' ]
echo status=$?
[ -z '(' ]
echo status=$?");
    assert_eq!(status, 0, "test/two-args-z status");
    assert_eq!(
        stdout,
        r"status=1
status=1
status=1
status=1
",
        "test/two-args-z stdout"
    );
    assert_eq!(stderr, r"", "test/two-args-z stderr");
}

#[test]
fn test_unquoted_empty_var() {
    // Oils `####` case: test/unquoted-empty-var
    let (status, stdout, stderr) = run(r"empty=''
[ $empty = '' ] && echo true");
    assert_eq!(status, 2, "test/unquoted-empty-var status");
    assert_eq!(stdout, r"", "test/unquoted-empty-var stdout");
    assert_eq!(
        stderr,
        r"bash: line 2: [: =: unary operator expected
",
        "test/unquoted-empty-var stderr"
    );
}

#[test]
fn test_v_dynamic_scope() {
    // Oils `####` case: test/v-dynamic-scope
    let (status, stdout, stderr) = run(r"f() {
  local f_var=0
  g
}
g() {
  test -v f_var
  echo dynamic=$?
  test -v g
  echo dynamic=$?
  test -v nonexistent
  echo dynamic=$?
}
f");
    assert_eq!(status, 0, "test/v-dynamic-scope status");
    assert_eq!(
        stdout,
        r"dynamic=0
dynamic=1
dynamic=1
",
        "test/v-dynamic-scope stdout"
    );
    assert_eq!(stderr, r"", "test/v-dynamic-scope stderr");
}

#[test]
fn test_v_variable() {
    // Oils `####` case: test/v-variable
    let (status, stdout, stderr) = run(r"test -v nonexistent
echo global=$?
g=1
test -v g
echo global=$?");
    assert_eq!(status, 0, "test/v-variable status");
    assert_eq!(
        stdout,
        r"global=1
global=0
",
        "test/v-variable stdout"
    );
    assert_eq!(stderr, r"", "test/v-variable stderr");
}

#[test]
fn test_z_two_token_lookahead() {
    // Oils `####` case: test/z-two-token-lookahead
    let (status, stdout, stderr) = run(r"[ -z ] && echo true
[ -z '>' ] || echo false
[ -z '>' -- ] && echo true");
    assert_eq!(status, 0, "test/z-two-token-lookahead status");
    assert_eq!(
        stdout,
        r"true
false
true
",
        "test/z-two-token-lookahead stdout"
    );
    assert_eq!(stderr, r"", "test/z-two-token-lookahead stderr");
}

#[test]
fn test_zero_args() {
    // Oils `####` case: test/zero-args
    let (status, stdout, stderr) = run(r"[ ] || echo false");
    assert_eq!(status, 0, "test/zero-args status");
    assert_eq!(
        stdout,
        r"false
",
        "test/zero-args stdout"
    );
    assert_eq!(stderr, r"", "test/zero-args stderr");
}
