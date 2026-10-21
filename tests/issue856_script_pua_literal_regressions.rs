//! Issue rubash#856: an isolated literal U+E000 in SCRIPT text was decoded
//! as a malformed raw-byte marker pair — the unrepresentable class of the
//! #296 E000 ownership protocol (E000 is the pair introducer U+E000+U+E0xx;
//! a bare occurrence has no legal owner and was silently dropped).
//!
//! Pinned byte-exact divergences vs GNU Bash 5.3.0 (issue discussion, WSL
//! oracle), all through the script-file entry:
//!
//!   v="a<E000>b"        GNU: a<E000>b          rubash: ab
//!   v="a<E000><E001>b"  GNU: a<E000><E001>b    rubash: a\x00b
//!   v="a<E000><E000>b"  GNU: a<E000><E000>b    rubash: a<E000>b
//!   ${v//a/ZZ}          GNU: ZZ<E000>b         rubash: ZZb
//!
//! Fix: `bytes_to_script_text` (substitution_metadata.rs) E400-escapes
//! literal marker-zone chars (U+E000..=U+E401) in valid-UTF-8 script spans
//! via markers::push_literal_char — the same entry contract the stdin path
//! already applies through `bytes_to_shell_text`. Script bytes are fresh
//! source, so every zone char is provably user data; live markers are only
//! ever produced post-lex. (`-c` text is excluded on purpose: respawned
//! `&` children receive serialized AST source over that argv channel and
//! re-escaping it would double-escape transport spelling.)
//!
//! Expected outputs are byte-verified against the GNU baseline pinned in
//! the issue (literal PUA chars pass through verbatim as UTF-8).

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const E000: char = '\u{E000}';
const E001: char = '\u{E001}';
const E400: char = '\u{E400}';

fn run_script_file(script: &str) -> (Vec<u8>, String) {
    let dir = Path::new("target").join("issue856");
    fs::create_dir_all(&dir).expect("mkdir");
    let path = dir.join("probe.sh");
    fs::write(&path, script).expect("write script");
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(&path)
        .output()
        .expect("run rubash");
    (
        output.stdout,
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn run_stdin(script: &str) -> Vec<u8> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn rubash");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(script.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait rubash");
    output.stdout
}

/// The three pinned forms plus the patsub reproduction — GNU keeps every
/// literal PUA glyph verbatim.
#[test]
fn isolated_e000_literal_forms_pass_through() {
    let script = format!(
        "v=\"a{E000}b\"\necho \"$v\"\nv2=\"a{E000}{E001}b\"\necho \"$v2\"\nv3=\"a{E000}{E000}b\"\necho \"$v3\"\necho \"${{v//a/ZZ}}\"\n"
    );
    let expected = format!("a{E000}b\na{E000}{E001}b\na{E000}{E000}b\nZZ{E000}b\n").into_bytes();
    let (stdout, stderr) = run_script_file(&script);
    assert!(stderr.is_empty(), "stderr not empty: {stderr}");
    assert_eq!(stdout, expected, "script-file entry diverged");
}

/// The literal escapes survive every quoting shape of the source word:
/// double-quoted, single-quoted, and unquoted.
#[test]
fn e000_literal_under_all_quoting_forms() {
    let script = format!("echo \"a{E000}b\"\necho 'a{E000}b'\necho a{E000}b\n");
    let expected = format!("a{E000}b\na{E000}b\na{E000}b\n").into_bytes();
    let (stdout, stderr) = run_script_file(&script);
    assert!(stderr.is_empty(), "stderr not empty: {stderr}");
    assert_eq!(stdout, expected);
}

/// A literal U+E400 (the literal-char escape introducer itself) is user
/// data in source text and must survive verbatim, not consume the next
/// char as an escape payload.
#[test]
fn isolated_e400_literal_passes_through() {
    let script = format!("echo \"a{E400}b\"\n");
    let expected = format!("a{E400}b\n").into_bytes();
    let (stdout, stderr) = run_script_file(&script);
    assert!(stderr.is_empty(), "stderr not empty: {stderr}");
    assert_eq!(stdout, expected);
}

/// `source` reads through the same script-entry decoder.
#[test]
fn sourced_file_e000_literal_passes_through() {
    let dir = Path::new("target").join("issue856-source");
    fs::create_dir_all(&dir).expect("mkdir");
    let lib = dir.join("lib.sh");
    fs::write(&lib, format!("v=\"a{E000}b\"\n")).expect("write lib");
    let lib_posix = lib.to_string_lossy().replace('\\', "/");
    let driver = dir.join("driver.sh");
    fs::write(&driver, format!("source {lib_posix}\necho \"$v\"\n")).expect("write driver");
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(&driver)
        .output()
        .expect("run rubash");
    assert_eq!(
        output.stdout,
        format!("a{E000}b\n").into_bytes(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Script-file and stdin entries now agree byte-for-byte on the pinned
/// forms (stdin already escaped zone chars via `bytes_to_shell_text`).
#[test]
fn script_file_matches_stdin_baseline() {
    let script = format!(
        "v=\"a{E000}b\"\necho \"$v\"\nv2=\"a{E000}{E001}b\"\necho \"$v2\"\necho \"${{v//a/ZZ}}\"\n"
    );
    let (file_out, stderr) = run_script_file(&script);
    assert!(stderr.is_empty(), "stderr not empty: {stderr}");
    assert_eq!(file_out, run_stdin(&script), "file vs stdin mismatch");
}

/// Protocol non-regression: the E000 pair carrier still decodes for data
/// produced by real encoders ($'\xNN' escapes, comsub capture), and the
/// ANSI-C \u entry still reaches the same literal codepoint.
#[test]
fn marker_protocol_still_decodes_real_pairs() {
    let (stdout, stderr) = run_script_file(
        "v=$(printf 'a\\x1cb')\nprintf '%s' \"$v\" | od -An -tx1\nx=$'\\uE000'\nprintf '%s' \"$x\" | od -An -tx1\ny=x$'\\xeb'y\nprintf '%s' \"$y\" | od -An -tx1\n",
    );
    assert!(stderr.is_empty(), "stderr not empty: {stderr}");
    let text = String::from_utf8_lossy(&stdout).into_owned();
    assert!(
        text.contains("61 1c 62"),
        "raw-byte pair for 0x1c must decode to the byte: {text}"
    );
    assert!(
        text.contains("ee 80 80"),
        "$'\\uE000' ANSI-C entry must still produce the literal char: {text}"
    );
    assert!(
        text.contains("78 eb 79"),
        "raw-byte pair for 0xeb must decode inside a word: {text}"
    );
}

/// eval re-parse composes: the escaped literal inside a single-quoted eval
/// operand rides the transport text into the re-lexer and still decodes to
/// the literal char once.
#[test]
fn eval_reparse_keeps_escaped_literal() {
    let script = format!("eval 'echo \"a{E000}b\"'\n");
    let expected = format!("a{E000}b\n").into_bytes();
    let (stdout, stderr) = run_script_file(&script);
    assert!(stderr.is_empty(), "stderr not empty: {stderr}");
    assert_eq!(stdout, expected);
}
