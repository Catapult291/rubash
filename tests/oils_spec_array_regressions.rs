//! Oils-spec vendor: spec/array.test.sh.
//!
//! Vendored from the Oils shell specification suite (Apache License 2.0):
//!   https://github.com/oils-for-unix/oils/blob/master/spec/array.test.sh
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
fn array_all_quoted() {
    // Oils `####` case: array/all-quoted
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
printf '<%s>\n' "${a[@]}""#);
    assert_eq!(status, 0, "array/all-quoted status");
    assert_eq!(
        stdout,
        r"<1>
<2 3>
",
        "array/all-quoted stdout"
    );
    assert_eq!(stderr, r"", "array/all-quoted stderr");
}

#[test]
fn array_all_unquoted() {
    // Oils `####` case: array/all-unquoted
    let (status, stdout, stderr) = run(r"a=(1 '2 3')
printf '<%s>\n' ${a[@]}");
    assert_eq!(status, 0, "array/all-unquoted status");
    assert_eq!(
        stdout,
        r"<1>
<2>
<3>
",
        "array/all-unquoted stdout"
    );
    assert_eq!(stderr, r"", "array/all-unquoted stderr");
}

#[test]
fn array_append_sparse() {
    // Oils `####` case: array/append-sparse
    let (status, stdout, stderr) = run(r#"a=()
(( a[99]=1 ))
b=()
(( b[33]=2 ))
(( b[66]=3 ))
a+=( "${b[@]}" )
printf '<%s>\n' "${a[@]}"
printf '<%s>\n' "${a[99]}" "${a[100]}" "${a[101]}""#);
    assert_eq!(status, 0, "array/append-sparse status");
    assert_eq!(
        stdout,
        r"<1>
<2>
<3>
<1>
<2>
<3>
",
        "array/append-sparse stdout"
    );
    assert_eq!(stderr, r"", "array/append-sparse stderr");
}

#[test]
fn array_assign_in_cmd_position() {
    // Oils `####` case: array/assign-in-cmd-position
    let (status, stdout, stderr) = run(r"ls foo=(1 2)");
    assert_eq!(status, 2, "array/assign-in-cmd-position status");
    assert_eq!(stdout, r"", "array/assign-in-cmd-position stdout");
    assert_eq!(
        stderr,
        r"bash: -c: line 1: syntax error near unexpected token `('
bash: -c: line 1: `ls foo=(1 2)'
",
        "array/assign-in-cmd-position stderr"
    );
}

#[test]
fn array_atq_sparse() {
    // Oils `####` case: array/atq-sparse
    let (status, stdout, stderr) = run(r#"a[0]=x
a[2]=y
echo "quoted = (${a[@]@Q})""#);
    assert_eq!(status, 0, "array/atq-sparse status");
    assert_eq!(
        stdout,
        r"quoted = ('x' 'y')
",
        "array/atq-sparse stdout"
    );
    assert_eq!(stderr, r"", "array/atq-sparse stderr");
}

#[test]
fn array_brace_expansion() {
    // Oils `####` case: array/brace-expansion
    let (status, stdout, stderr) = run(r#"a=(-{a,b} {c,d}-)
echo "${a[@]}""#);
    assert_eq!(status, 0, "array/brace-expansion status");
    assert_eq!(
        stdout,
        r"-a -b c- d-
",
        "array/brace-expansion stdout"
    );
    assert_eq!(stderr, r"", "array/brace-expansion stderr");
}

#[test]
fn array_declare_empty_append() {
    // Oils `####` case: array/declare-empty-append
    let (status, stdout, stderr) = run(r#"declare -a myarray
printf '<%s>\n' "${myarray[@]}"
myarray+=('x')
printf '<%s>\n' "${myarray[@]}"
f() {
  local -a myarray
  printf '<%s>\n' "${myarray[@]}"
  myarray+=('x')
  printf '<%s>\n' "${myarray[@]}"
}
f"#);
    assert_eq!(status, 0, "array/declare-empty-append status");
    assert_eq!(
        stdout,
        r"<>
<x>
<>
<x>
",
        "array/declare-empty-append stdout"
    );
    assert_eq!(stderr, r"", "array/declare-empty-append stderr");
}

#[test]
fn array_declare_then_append() {
    // Oils `####` case: array/declare-then-append
    let (status, stdout, stderr) = run(r#"declare -a array
array+=(a)
array+=(b c)
printf '<%s>\n' "${array[@]}""#);
    assert_eq!(status, 0, "array/declare-then-append status");
    assert_eq!(
        stdout,
        r"<a>
<b>
<c>
",
        "array/declare-then-append stdout"
    );
    assert_eq!(stderr, r"", "array/declare-then-append stderr");
}

#[test]
fn array_dollar_xy_index() {
    // Oils `####` case: array/dollar-xy-index
    let (status, stdout, stderr) = run(r"x=1
y=2
a[$x$y]=foo
echo ${a[12]}
echo ${#a[@]}");
    assert_eq!(status, 0, "array/dollar-xy-index status");
    assert_eq!(
        stdout,
        r"foo
1
",
        "array/dollar-xy-index stdout"
    );
    assert_eq!(stderr, r"", "array/dollar-xy-index stderr");
}

#[test]
fn array_dtest_v_arith_vars() {
    // Oils `####` case: array/dtest-v-arith-vars
    let (status, stdout, stderr) = run(r"typeset -a array
array=('' nonempty)
zero=0
[[ -v array[zero+0] ]]
echo zero=$?
[[ -v array[zero+1] ]]
echo one=$?
[[ -v array[zero+2] ]]
echo two=$?
echo ---
i='0+0'
[[ -v array[i] ]]
echo zero=$?
i='0+1'
[[ -v array[i] ]]
echo one=$?
i='0+2'
[[ -v array[i] ]]
echo two=$?");
    assert_eq!(status, 0, "array/dtest-v-arith-vars status");
    assert_eq!(
        stdout,
        r"zero=0
one=0
two=1
---
zero=0
one=0
two=1
",
        "array/dtest-v-arith-vars stdout"
    );
    assert_eq!(stderr, r"", "array/dtest-v-arith-vars stderr");
}

#[test]
fn array_dtest_v_indexed() {
    // Oils `####` case: array/dtest-v-indexed
    let (status, stdout, stderr) = run(r"array=(1 2 3)
[[ -v array[1] ]]
echo status=$?
[[ -v array[4] ]]
echo status=$?");
    assert_eq!(status, 0, "array/dtest-v-indexed status");
    assert_eq!(
        stdout,
        r"status=0
status=1
",
        "array/dtest-v-indexed stdout"
    );
    assert_eq!(stderr, r"", "array/dtest-v-indexed stderr");
}

#[test]
fn array_dtest_v_negative() {
    // Oils `####` case: array/dtest-v-negative
    let (status, stdout, stderr) = run(r"a[0]=x
a[5]=y
a[10]=z
[[ -v a[-1] ]] && echo 'a has -1'
[[ -v a[-2] ]] && echo 'a has -2'
[[ -v a[-5] ]] && echo 'a has -5'
[[ -v a[-6] ]] && echo 'a has -6'
[[ -v a[-10] ]] && echo 'a has -10'
[[ -v a[-11] ]] && echo 'a has -11'");
    assert_eq!(status, 0, "array/dtest-v-negative status");
    assert_eq!(
        stdout,
        r"a has -1
a has -6
a has -11
",
        "array/dtest-v-negative stdout"
    );
    assert_eq!(stderr, r"", "array/dtest-v-negative stderr");
}

#[test]
fn array_dynamic_rhs() {
    // Oils `####` case: array/dynamic-rhs
    let (status, stdout, stderr) = run(r"declare -a array
array=(zero one two three)
echo ${array[1+2]}
code='1+2'
echo ${array[$code]}");
    assert_eq!(status, 0, "array/dynamic-rhs status");
    assert_eq!(
        stdout,
        r"three
three
",
        "array/dynamic-rhs stdout"
    );
    assert_eq!(stderr, r"", "array/dynamic-rhs stderr");
}

#[test]
fn array_empty_string_elem() {
    // Oils `####` case: array/empty-string-elem
    let (status, stdout, stderr) = run(r#"empty=('')
printf '<%s>\n' "${empty[@]}""#);
    assert_eq!(status, 0, "array/empty-string-elem status");
    assert_eq!(
        stdout,
        r"<>
",
        "array/empty-string-elem stdout"
    );
    assert_eq!(stderr, r"", "array/empty-string-elem stderr");
}

#[test]
fn array_from_argv() {
    // Oils `####` case: array/from-argv
    let (status, stdout, stderr) = run(r#"set -- 'a b' 'c'
array1=('x y' 'z')
array2=("$@")
printf '<%s>\n' "${array1[@]}" "${array2[@]}""#);
    assert_eq!(status, 0, "array/from-argv status");
    assert_eq!(
        stdout,
        r"<x y>
<z>
<a b>
<c>
",
        "array/from-argv stdout"
    );
    assert_eq!(stderr, r"", "array/from-argv stderr");
}

#[test]
fn array_glob_in_init() {
    // Oils `####` case: array/glob-in-init
    let (status, stdout, stderr) = run(r#"touch y.Y yy.Y
a=(*.Y)
printf '<%s>\n' "${a[@]}""#);
    assert_eq!(status, 0, "array/glob-in-init status");
    assert_eq!(
        stdout,
        r"<y.Y>
<yy.Y>
",
        "array/glob-in-init stdout"
    );
    assert_eq!(stderr, r"", "array/glob-in-init stderr");
}

#[test]
fn array_index_arith() {
    // Oils `####` case: array/index-arith
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
i=5
printf '<%s>\n' "${a[i-4]}""#);
    assert_eq!(status, 0, "array/index-arith status");
    assert_eq!(
        stdout,
        r"<2 3>
",
        "array/index-arith stdout"
    );
    assert_eq!(stderr, r"", "array/index-arith stderr");
}

#[test]
fn array_index_cmdsub() {
    // Oils `####` case: array/index-cmdsub
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
printf '<%s>\n' "${a[$(echo 1)]}""#);
    assert_eq!(status, 0, "array/index-cmdsub status");
    assert_eq!(
        stdout,
        r"<2 3>
",
        "array/index-cmdsub stdout"
    );
    assert_eq!(stderr, r"", "array/index-cmdsub stderr");
}

#[test]
fn array_index_var() {
    // Oils `####` case: array/index-var
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
i=1
printf '<%s>\n' "${a[$i]}""#);
    assert_eq!(status, 0, "array/index-var status");
    assert_eq!(
        stdout,
        r"<2 3>
",
        "array/index-var stdout"
    );
    assert_eq!(stderr, r"", "array/index-var stderr");
}

#[test]
fn array_indices_bang() {
    // Oils `####` case: array/indices-bang
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
printf '<%s>\n' "${!a[@]}""#);
    assert_eq!(status, 0, "array/indices-bang status");
    assert_eq!(
        stdout,
        r"<0>
<1>
",
        "array/indices-bang stdout"
    );
    assert_eq!(stderr, r"", "array/indices-bang stderr");
}

#[test]
fn array_interpolate() {
    // Oils `####` case: array/interpolate
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
a=(0 "${a[@]}" '4 5')
printf '<%s>\n' "${a[@]}""#);
    assert_eq!(status, 0, "array/interpolate status");
    assert_eq!(
        stdout,
        r"<0>
<1>
<2 3>
<4 5>
",
        "array/interpolate stdout"
    );
    assert_eq!(stderr, r"", "array/interpolate stderr");
}

#[test]
fn array_iteration() {
    // Oils `####` case: array/iteration
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
for v in "${a[@]}"; do
  echo $v
done"#);
    assert_eq!(status, 0, "array/iteration status");
    assert_eq!(
        stdout,
        r"1
2 3
",
        "array/iteration stdout"
    );
    assert_eq!(stderr, r"", "array/iteration stderr");
}

#[test]
fn array_len_after_unset() {
    // Oils `####` case: array/len-after-unset
    let (status, stdout, stderr) = run(r"a=(0 1 2 3)
unset a[-1]
echo len=${#a[@]}
unset a[-1]
echo len=${#a[@]}");
    assert_eq!(status, 0, "array/len-after-unset status");
    assert_eq!(
        stdout,
        r"len=3
len=2
",
        "array/len-after-unset stdout"
    );
    assert_eq!(stderr, r"", "array/len-after-unset stderr");
}

#[test]
fn array_len_after_unset_neg() {
    // Oils `####` case: array/len-after-unset-neg
    let (status, stdout, stderr) = run(r#"a=(x)
a[9]=y
echo "len ${#a[@]};"
unset -v 'a[-1]'
echo "len ${#a[@]};"
echo "last ${a[@]: -1};""#);
    assert_eq!(status, 0, "array/len-after-unset-neg status");
    assert_eq!(
        stdout,
        r"len 2;
len 1;
last x;
",
        "array/len-after-unset-neg stdout"
    );
    assert_eq!(stderr, r"", "array/len-after-unset-neg stderr");
}

#[test]
fn array_len_index_transform() {
    // Oils `####` case: array/len-index-transform
    let (status, stdout, stderr) = run(r#"a=('123' '456')
echo "${#a[0]}" "${#a[0]/1/xxx}""#);
    assert_eq!(status, 1, "array/len-index-transform status");
    assert_eq!(stdout, r"", "array/len-index-transform stdout");
    assert_eq!(
        stderr,
        r"bash: line 2: ${#a[0]/1/xxx}: bad substitution
",
        "array/len-index-transform stderr"
    );
}

#[test]
fn array_len_of_elem() {
    // Oils `####` case: array/len-of-elem
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
echo "${#a[1]}""#);
    assert_eq!(status, 0, "array/len-of-elem status");
    assert_eq!(
        stdout,
        r"3
",
        "array/len-of-elem stdout"
    );
    assert_eq!(stderr, r"", "array/len-of-elem stderr");
}

#[test]
fn array_local_array() {
    // Oils `####` case: array/local-array
    let (status, stdout, stderr) = run(r#"f() {
  local a=(1 '2 3')
  printf '<%s>\n' "${a[0]}"
}
f"#);
    assert_eq!(status, 0, "array/local-array status");
    assert_eq!(
        stdout,
        r"<1>
",
        "array/local-array stdout"
    );
    assert_eq!(stderr, r"", "array/local-array stderr");
}

#[test]
fn array_multi_subscript() {
    // Oils `####` case: array/multi-subscript
    let (status, stdout, stderr) = run(r#"a=('123' '456')
printf '<%s>\n' "${a[0]}" "${a[0][0]}""#);
    assert_eq!(status, 1, "array/multi-subscript status");
    assert_eq!(stdout, r"", "array/multi-subscript stdout");
    assert_eq!(
        stderr,
        r"bash: line 2: ${a[0][0]}: bad substitution
",
        "array/multi-subscript stderr"
    );
}

#[test]
fn array_multiline() {
    // Oils `####` case: array/multiline
    let (status, stdout, stderr) = run(r#"a=(
1
'2 3'
)
printf '<%s>\n' "${a[@]}""#);
    assert_eq!(status, 0, "array/multiline status");
    assert_eq!(
        stdout,
        r"<1>
<2 3>
",
        "array/multiline stdout"
    );
    assert_eq!(stderr, r"", "array/multiline stderr");
}

#[test]
fn array_nameref_bang() {
    // Oils `####` case: array/nameref-bang
    let (status, stdout, stderr) = run(r#"foo=bar
a=('1 2' foo '2 3')
printf '<%s>\n' "${!a[1]}""#);
    assert_eq!(status, 0, "array/nameref-bang status");
    assert_eq!(
        stdout,
        r"<bar>
",
        "array/nameref-bang stdout"
    );
    assert_eq!(stderr, r"", "array/nameref-bang stderr");
}

#[test]
fn array_neg_index_assign_err() {
    // Oils `####` case: array/neg-index-assign-err
    let (status, stdout, stderr) = run(r"a=()
a[-1]=1");
    assert_eq!(status, 1, "array/neg-index-assign-err status");
    assert_eq!(stdout, r"", "array/neg-index-assign-err stdout");
    assert_eq!(
        stderr,
        r"bash: line 2: a[-1]: bad array subscript
",
        "array/neg-index-assign-err stderr"
    );
}

#[test]
fn array_neg_offset_slice() {
    // Oils `####` case: array/neg-offset-slice
    let (status, stdout, stderr) = run(r#"a=(1 2 3 4)
echo "a=(${a[*]})"
echo "begin=-1 -> (${a[*]: -1})"
echo "begin=-2 -> (${a[*]: -2})"
echo "begin=-3 -> (${a[*]: -3})"
echo "begin=-4 -> (${a[*]: -4})"
echo "begin=-5 -> (${a[*]: -5})""#);
    assert_eq!(status, 0, "array/neg-offset-slice status");
    assert_eq!(
        stdout,
        r"a=(1 2 3 4)
begin=-1 -> (4)
begin=-2 -> (3 4)
begin=-3 -> (2 3 4)
begin=-4 -> (1 2 3 4)
begin=-5 -> ()
",
        "array/neg-offset-slice stdout"
    );
    assert_eq!(stderr, r"", "array/neg-offset-slice stderr");
}

#[test]
fn array_negative_index() {
    // Oils `####` case: array/negative-index
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
printf '<%s>\n' "${a[-1]}" "${a[-2]}" "${a[-5]}""#);
    assert_eq!(status, 0, "array/negative-index status");
    assert_eq!(
        stdout,
        r"<2 3>
<1>
<>
",
        "array/negative-index stdout"
    );
    assert_eq!(
        stderr,
        r"bash: line 2: a: bad array subscript
",
        "array/negative-index stderr"
    );
}

#[test]
fn array_negative_sparse() {
    // Oils `####` case: array/negative-sparse
    let (status, stdout, stderr) = run(r#"a=(0 1 2 3 4)
unset a[1]
unset a[4]
echo "${a[@]}"
echo -1 ${a[-1]}
echo -2 ${a[-2]}
echo -3 ${a[-3]}
echo -4 ${a[-4]}
echo -5 ${a[-5]}
a[-1]+=0
echo ${a[@]}
(( a[-1] += 42 ))
echo ${a[@]}"#);
    assert_eq!(status, 0, "array/negative-sparse status");
    assert_eq!(
        stdout,
        r"0 2 3
-1 3
-2 2
-3
-4 0
-5
0 2 30
0 2 72
",
        "array/negative-sparse stdout"
    );
    assert_eq!(
        stderr,
        r"bash: line 9: a: bad array subscript
",
        "array/negative-sparse stderr"
    );
}

#[test]
fn array_negative_sparse_unset() {
    // Oils `####` case: array/negative-sparse-unset
    let (status, stdout, stderr) = run(r"a=(0 1)
unset 'a[-1]'
a+=(2 3)
echo ${a[0]} $((a[0]))
echo ${a[1]} $((a[1]))
echo ${a[2]} $((a[2]))
echo ${a[3]} $((a[3]))");
    assert_eq!(status, 0, "array/negative-sparse-unset status");
    assert_eq!(
        stdout,
        r"0 0
2 2
3 3
0
",
        "array/negative-sparse-unset stdout"
    );
    assert_eq!(stderr, r"", "array/negative-sparse-unset stderr");
}

#[test]
fn array_nounset_empty() {
    // Oils `####` case: array/nounset-empty
    let (status, stdout, stderr) = run(r#"set -o nounset
empty=()
printf '<%s>\n' "${empty[@]}"
echo status=$?"#);
    assert_eq!(status, 0, "array/nounset-empty status");
    assert_eq!(
        stdout,
        r"<>
status=0
",
        "array/nounset-empty stdout"
    );
    assert_eq!(stderr, r"", "array/nounset-empty stderr");
}

#[test]
fn array_num_elements() {
    // Oils `####` case: array/num-elements
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
echo "${#a[@]}" ${#a[@]}"#);
    assert_eq!(status, 0, "array/num-elements status");
    assert_eq!(
        stdout,
        r"2 2
",
        "array/num-elements stdout"
    );
    assert_eq!(stderr, r"", "array/num-elements stderr");
}

#[test]
fn array_oob_index() {
    // Oils `####` case: array/oob-index
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
printf '<%s>\n' "${a[3]}""#);
    assert_eq!(status, 0, "array/oob-index status");
    assert_eq!(
        stdout,
        r"<>
",
        "array/oob-index stdout"
    );
    assert_eq!(stderr, r"", "array/oob-index stderr");
}

#[test]
fn array_retrieve_index() {
    // Oils `####` case: array/retrieve-index
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
printf '<%s>\n' "${a[1]}""#);
    assert_eq!(status, 0, "array/retrieve-index status");
    assert_eq!(
        stdout,
        r"<2 3>
",
        "array/retrieve-index stdout"
    );
    assert_eq!(stderr, r"", "array/retrieve-index stderr");
}

#[test]
fn array_self_as_index_lhs() {
    // Oils `####` case: array/self-as-index-lhs
    let (status, stdout, stderr) = run(r#"a[a]=42
a[a]=99
printf '<%s>\n' "${a[@]}" "${a[0]}" "${a[42]}" "${a[99]}""#);
    assert_eq!(status, 0, "array/self-as-index-lhs status");
    assert_eq!(
        stdout,
        r"<42>
<99>
<42>
<99>
<>
",
        "array/self-as-index-lhs stdout"
    );
    assert_eq!(stderr, r"", "array/self-as-index-lhs stderr");
}

#[test]
fn array_self_as_index_rhs() {
    // Oils `####` case: array/self-as-index-rhs
    let (status, stdout, stderr) = run(r"a=(1 2 3)
(( x = a[a] ))
echo $x");
    assert_eq!(status, 0, "array/self-as-index-rhs status");
    assert_eq!(
        stdout,
        r"2
",
        "array/self-as-index-rhs stdout"
    );
    assert_eq!(stderr, r"", "array/self-as-index-rhs stderr");
}

#[test]
fn array_set_elem() {
    // Oils `####` case: array/set-elem
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
a[0]=9
printf '<%s>\n' "${a[@]}""#);
    assert_eq!(status, 0, "array/set-elem status");
    assert_eq!(
        stdout,
        r"<9>
<2 3>
",
        "array/set-elem stdout"
    );
    assert_eq!(stderr, r"", "array/set-elem stderr");
}

#[test]
fn array_set_elem_arrayref() {
    // Oils `####` case: array/set-elem-arrayref
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
i=(0 1)
a[${i[1]}]=9
printf '<%s>\n' "${a[@]}""#);
    assert_eq!(status, 0, "array/set-elem-arrayref status");
    assert_eq!(
        stdout,
        r"<1>
<9>
",
        "array/set-elem-arrayref stdout"
    );
    assert_eq!(stderr, r"", "array/set-elem-arrayref stderr");
}

#[test]
fn array_set_elem_to_array() {
    // Oils `####` case: array/set-elem-to-array
    let (status, stdout, stderr) = run(r#"a=(1 2)
a[0]=(3 4)
echo "status=$?""#);
    assert_eq!(status, 0, "array/set-elem-to-array status");
    assert_eq!(
        stdout,
        r"status=1
",
        "array/set-elem-to-array stdout"
    );
    assert_eq!(
        stderr,
        r"bash: line 2: a[0]: cannot assign list to array member
",
        "array/set-elem-to-array stderr"
    );
}

#[test]
fn array_set_elem_varref() {
    // Oils `####` case: array/set-elem-varref
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
i=0
a[$i]=9
printf '<%s>\n' "${a[@]}""#);
    assert_eq!(status, 0, "array/set-elem-varref status");
    assert_eq!(
        stdout,
        r"<9>
<2 3>
",
        "array/set-elem-varref stdout"
    );
    assert_eq!(stderr, r"", "array/set-elem-varref stderr");
}

#[test]
fn array_singleton_copy_decay() {
    // Oils `####` case: array/singleton-copy-decay
    let (status, stdout, stderr) = run(r#"a=( '12 3' )
b=( "${a[@]}" )
c="${a[@]}"
d=${a[*]}
echo ${#a[0]} ${#b[0]}
echo ${#a[@]} ${#b[@]}
echo ${#c[0]} ${#d[0]}
echo ${#c[@]} ${#d[@]}"#);
    assert_eq!(status, 0, "array/singleton-copy-decay status");
    assert_eq!(
        stdout,
        r"4 4
1 1
4 4
1 1
",
        "array/singleton-copy-decay stdout"
    );
    assert_eq!(stderr, r"", "array/singleton-copy-decay stderr");
}

#[test]
fn array_singleton_default() {
    // Oils `####` case: array/singleton-default
    let (status, stdout, stderr) = run(r#"single=('')
printf '<%s>\n' ${single[@]:-none} x "${single[@]:-none}""#);
    assert_eq!(status, 0, "array/singleton-default status");
    assert_eq!(
        stdout,
        r"<none>
<x>
<none>
",
        "array/singleton-default stdout"
    );
    assert_eq!(stderr, r"", "array/singleton-default stderr");
}

#[test]
fn array_slice() {
    // Oils `####` case: array/slice
    let (status, stdout, stderr) = run(r#"a=(1 2 3)
printf '<%s>\n' "${a[@]:1:2}""#);
    assert_eq!(status, 0, "array/slice status");
    assert_eq!(
        stdout,
        r"<2>
<3>
",
        "array/slice stdout"
    );
    assert_eq!(stderr, r"", "array/slice stderr");
}

#[test]
fn array_slice_arith() {
    // Oils `####` case: array/slice-arith
    let (status, stdout, stderr) = run(r#"a=(1 2 3)
i=5
printf '<%s>\n' "${a[@]:i-4:2}""#);
    assert_eq!(status, 0, "array/slice-arith status");
    assert_eq!(
        stdout,
        r"<2>
<3>
",
        "array/slice-arith stdout"
    );
    assert_eq!(stderr, r"", "array/slice-arith stderr");
}

#[test]
fn array_slice_neg_begin() {
    // Oils `####` case: array/slice-neg-begin
    let (status, stdout, stderr) = run(r#"a=(1 2 3 4 5)
printf '<%s>\n' "${a[@]:(-4)}""#);
    assert_eq!(status, 0, "array/slice-neg-begin status");
    assert_eq!(
        stdout,
        r"<2>
<3>
<4>
<5>
",
        "array/slice-neg-begin stdout"
    );
    assert_eq!(stderr, r"", "array/slice-neg-begin stderr");
}

#[test]
fn array_slice_sparse() {
    // Oils `####` case: array/slice-sparse
    let (status, stdout, stderr) = run(r#"(( a[33]=1 ))
(( a[66]=2 ))
(( a[99]=2 ))
printf '<%s>\n' "${a[@]:15:2}""#);
    assert_eq!(status, 0, "array/slice-sparse status");
    assert_eq!(
        stdout,
        r"<1>
<2>
",
        "array/slice-sparse stdout"
    );
    assert_eq!(stderr, r"", "array/slice-sparse stderr");
}

#[test]
fn array_sparse_create() {
    // Oils `####` case: array/sparse-create
    let (status, stdout, stderr) = run(r#"a=()
(( a[99]=1 ))
echo len=${#a[@]}
printf '<%s>\n' "${a[@]}"
echo "unset=${a[33]}"
echo len-of-unset=${#a[33]}"#);
    assert_eq!(status, 0, "array/sparse-create status");
    assert_eq!(
        stdout,
        r"len=1
<1>
unset=
len-of-unset=0
",
        "array/sparse-create stdout"
    );
    assert_eq!(stderr, r"", "array/sparse-create stderr");
}

#[test]
fn array_sparse_implicit() {
    // Oils `####` case: array/sparse-implicit
    let (status, stdout, stderr) = run(r#"(( a[99]=1 ))
echo len=${#a[@]}
printf '<%s>\n' "${a[@]}"
echo "unset=${a[33]}"
echo len-of-unset=${#a[33]}"#);
    assert_eq!(status, 0, "array/sparse-implicit status");
    assert_eq!(
        stdout,
        r"len=1
<1>
unset=
len-of-unset=0
",
        "array/sparse-implicit stdout"
    );
    assert_eq!(stderr, r"", "array/sparse-implicit stderr");
}

#[test]
fn array_sparse_indices_bang() {
    // Oils `####` case: array/sparse-indices-bang
    let (status, stdout, stderr) = run(r#"a=()
(( a[99]=1 ))
printf '<%s>\n' "${!a[@]}""#);
    assert_eq!(status, 0, "array/sparse-indices-bang status");
    assert_eq!(
        stdout,
        r"<99>
",
        "array/sparse-indices-bang stdout"
    );
    assert_eq!(stderr, r"", "array/sparse-indices-bang stderr");
}

#[test]
fn array_star_quoted() {
    // Oils `####` case: array/star-quoted
    let (status, stdout, stderr) = run(r#"a=(1 '2 3')
printf '<%s>\n' "${a[*]}""#);
    assert_eq!(status, 0, "array/star-quoted status");
    assert_eq!(
        stdout,
        r"<1 2 3>
",
        "array/star-quoted stdout"
    );
    assert_eq!(stderr, r"", "array/star-quoted stderr");
}

#[test]
fn array_star_unquoted() {
    // Oils `####` case: array/star-unquoted
    let (status, stdout, stderr) = run(r"a=(1 '2 3')
printf '<%s>\n' ${a[*]}");
    assert_eq!(status, 0, "array/star-unquoted status");
    assert_eq!(
        stdout,
        r"<1>
<2>
<3>
",
        "array/star-unquoted stdout"
    );
    assert_eq!(stderr, r"", "array/star-unquoted stderr");
}

#[test]
fn array_str_at_noop() {
    // Oils `####` case: array/str-at-noop
    let (status, stdout, stderr) = run(r"s='abc'
echo ${s[@]}
echo ${s[*]}");
    assert_eq!(status, 0, "array/str-at-noop status");
    assert_eq!(
        stdout,
        r"abc
abc
",
        "array/str-at-noop stdout"
    );
    assert_eq!(stderr, r"", "array/str-at-noop stderr");
}

#[test]
fn array_strip_quoted() {
    // Oils `####` case: array/strip-quoted
    let (status, stdout, stderr) = run(r#"files=('foo.c' 'sp ace.h' 'bar.c')
printf '<%s>\n' "${files[@]%.c}""#);
    assert_eq!(status, 0, "array/strip-quoted status");
    assert_eq!(
        stdout,
        r"<foo>
<sp ace.h>
<bar>
",
        "array/strip-quoted stdout"
    );
    assert_eq!(stderr, r"", "array/strip-quoted stderr");
}

#[test]
fn array_strip_unquoted() {
    // Oils `####` case: array/strip-unquoted
    let (status, stdout, stderr) = run(r"files=('foo.c' 'sp ace.h' 'bar.c')
printf '<%s>\n' ${files[@]%.c}");
    assert_eq!(status, 0, "array/strip-unquoted status");
    assert_eq!(
        stdout,
        r"<foo>
<sp>
<ace.h>
<bar>
",
        "array/strip-unquoted stdout"
    );
    assert_eq!(stderr, r"", "array/strip-unquoted stderr");
}

#[test]
fn array_test_v_arith() {
    // Oils `####` case: array/test-v-arith
    let (status, stdout, stderr) = run(r"array=(1 2 3 '')
test -v 'array[1+1]'
echo status=$?
test -v 'array[4+1]'
echo status=$?
echo
echo dbracket
[[ -v array[1+1] ]]
echo status=$?
[[ -v array[4+1] ]]
echo status=$?");
    assert_eq!(status, 0, "array/test-v-arith status");
    assert_eq!(
        stdout,
        r"status=0
status=1

dbracket
status=0
status=1
",
        "array/test-v-arith stdout"
    );
    assert_eq!(stderr, r"", "array/test-v-arith stderr");
}

#[test]
fn array_test_v_indexed() {
    // Oils `####` case: array/test-v-indexed
    let (status, stdout, stderr) = run(r"array=(1 2 3 '')
test -v 'array[1]'
echo set=$?
test -v 'array[3]'
echo empty=$?
test -v 'array[4]'
echo unset=$?");
    assert_eq!(status, 0, "array/test-v-indexed status");
    assert_eq!(
        stdout,
        r"set=0
empty=0
unset=1
",
        "array/test-v-indexed stdout"
    );
    assert_eq!(stderr, r"", "array/test-v-indexed stderr");
}

#[test]
fn array_tilde_expansion() {
    // Oils `####` case: array/tilde-expansion
    let (status, stdout, stderr) = run(r#"HOME=/home/bob
a=(~/src ~/git)
echo "${a[@]}""#);
    assert_eq!(status, 0, "array/tilde-expansion status");
    assert_eq!(
        stdout,
        r"/home/bob/src /home/bob/git
",
        "array/tilde-expansion stdout"
    );
    assert_eq!(stderr, r"", "array/tilde-expansion stderr");
}

#[test]
fn array_unset_neg_oob() {
    // Oils `####` case: array/unset-neg-oob
    let (status, stdout, stderr) = run(r"a=(1)
unset -v 'a[-2]'");
    assert_eq!(status, 1, "array/unset-neg-oob status");
    assert_eq!(stdout, r"", "array/unset-neg-oob stdout");
    assert_eq!(
        stderr,
        r"bash: line 2: unset: [-2]: bad array subscript
",
        "array/unset-neg-oob stderr"
    );
}

#[test]
fn array_wordsplitting_in_init() {
    // Oils `####` case: array/wordsplitting-in-init
    let (status, stdout, stderr) = run(r#"array=('1 2' $(echo '3 4'))
printf '<%s>\n' "${array[@]}""#);
    assert_eq!(status, 0, "array/wordsplitting-in-init status");
    assert_eq!(
        stdout,
        r"<1 2>
<3>
<4>
",
        "array/wordsplitting-in-init stdout"
    );
    assert_eq!(stderr, r"", "array/wordsplitting-in-init stderr");
}
