//! Phase 0 decode-at-boundary API regression tests
//! (niubash host-semantic-layer-elimination plan, rubash side).
//!
//! GNU anchors: locale.c:550 `dump_translatable_strings` prints the
//! `$"..."` content as user text; shell.c:507-509 selects the parse-only
//! dump mode; the carriers decoded here are rubash's port of
//! parse.y:5694-5706 CTLESC/CTLNUL protection and subst.c:4692
//! `dequote_escapes` / subst.c:4807 `dequote_string`.

use rubash::decode_to_visible_text;
use rubash::lexer::{tokenize, TokenKind};

fn word_values(input: &str) -> Vec<String> {
    tokenize(input)
        .iter()
        .filter(|t| t.kind == TokenKind::Word)
        .map(|t| decode_to_visible_text(&t.value))
        .collect()
}

#[test]
fn token_values_decode_to_source_characters() {
    // Quoted characters arrive as carrier bytes in token.value; the public
    // decoder restores what the user typed.
    assert_eq!(
        word_values(r#"echo "a'b" 'c\d' a\*b"#),
        ["echo", "a'b", r"c\d", "a*b"]
    );
}

#[test]
fn locale_string_token_decodes_for_dump_strings() {
    // $"..." lexes as a plain Word (no LocaleString kind); the host picks
    // candidates by raw text and decodes value through this API.
    let tokens = tokenize(r#"echo $"it's \"fine\""#);
    let word = tokens
        .iter()
        .find(|t| t.kind == TokenKind::Word && t.raw.starts_with("$\""))
        .expect("locale string token");
    assert_eq!(decode_to_visible_text(&word.value), "it's \"fine\"");
}

#[test]
fn decode_is_idempotent_on_plain_text() {
    assert_eq!(decode_to_visible_text("declare -p x"), "declare -p x");
}

/// #802: the compound-assignment carrier the parser pushes into
/// `name=(...)` words is a whitelisted carrier of the public decoder. The
/// interactive line editor decodes every AST word BETWEEN parse and
/// execution (host `decoded_words`: easter-egg routing, this-shell
/// dispatch), and that transport text carries `__RUBASH_CA1__` by
/// contract — a debug build used to panic on the leak assertion
/// (locale.rs) the moment the three typed lines of
/// `declare -A name=(...)` completed.
#[test]
fn compound_assignment_carrier_word_decodes_to_visible_text() {
    let script = "declare -A name=(\n[中国]=\"北京\"\n)\n";
    let ast = rubash::parser::parse(&rubash::lexer::tokenize(script));
    let word = ast.commands[0]
        .words
        .iter()
        .find(|word| word.contains("__RUBASH_CA1__"))
        .expect("parser tags the compound assignment word with the carrier");
    assert_eq!(
        decode_to_visible_text(word),
        "name=(\n[中国]=\"北京\"\n)",
        "the carrier is consumed and the typed text survives"
    );
}

/// The carrier may sit mid-word and in user data; the decoder consumes the
/// reserved string everywhere on the visible-text boundary (same reserved
/// stance the assignment boundary takes for a leading occurrence:
/// `x=__RUBASH_CA1__` already stores an empty value on master, so the
/// decoded word shape `x=` is what a display consumer must see).
#[test]
fn compound_assignment_carrier_is_consumed_wherever_it_rides() {
    assert_eq!(decode_to_visible_text("__RUBASH_CA1__(a b)"), "(a b)");
    assert_eq!(decode_to_visible_text("a__RUBASH_CA1__b"), "ab");
    assert_eq!(
        decode_to_visible_text("x=__RUBASH_CA1__"),
        "x=",
        "a leading carrier after `=` decodes to the stored-empty word shape"
    );
    assert_eq!(decode_to_visible_text("x="), "x=");
    assert_eq!(
        decode_to_visible_text("__RUBASH_CA1_nearly__"),
        "__RUBASH_CA1_nearly__",
        "a non-marker lookalike is untouched"
    );
}

/// #824: the other named-string markers pass through verbatim. They are
/// plain typeable ASCII with no entry-side encoding, and their live
/// producers are consumed at the execution boundaries (heredoc body
/// execution, comsub payload decode) before anything is displayed — so on
/// this boundary an occurrence is user data by contract. A debug build
/// used to panic here on the "leaked to output" golden asserts.
#[test]
fn heredoc_and_comsub_marker_text_passes_through_verbatim() {
    assert_eq!(
        decode_to_visible_text("a__RUBASH_HD1__b"),
        "a__RUBASH_HD1__b"
    );
    assert_eq!(
        decode_to_visible_text("__RUBASH_CSB1_41;"),
        "__RUBASH_CSB1_41;"
    );
}

/// #824: a guard/assignment-sentinel codepoint in decode OUTPUT is
/// E400-restored user data (the rubash#735 literal-char escape contract) —
/// the scan consumes every bare guard with its protected char, so what
/// survives is by construction data, never a live guard.
#[test]
fn guard_codepoints_survive_as_restored_user_data() {
    assert_eq!(
        decode_to_visible_text("\u{E400}\u{E314}Z"),
        "\u{E314}Z",
        "E400-escaped U+E314 is user data, restored verbatim"
    );
    assert_eq!(
        decode_to_visible_text("a\u{E400}\u{E301}b"),
        "a\u{E301}b",
        "E400-escaped U+E301 is user data, restored verbatim"
    );
    assert_eq!(
        decode_to_visible_text("\u{E314}Z"),
        "Z",
        "a bare guard pair is still consumed: guard + protected char -> char"
    );
}
