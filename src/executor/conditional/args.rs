use super::super::pattern_contains_glob;
use super::pattern::{
    case_pattern_matches, case_pattern_matches_nocase, sides_need_byte_char_matching,
};

pub(in crate::executor) fn is_conditional_file_unary(op: &str) -> bool {
    matches!(
        op,
        "-a" | "-b"
            | "-c"
            | "-d"
            | "-e"
            | "-f"
            | "-g"
            | "-h"
            | "-L"
            | "-k"
            | "-p"
            | "-r"
            | "-s"
            | "-S"
            | "-t"
            | "-u"
            | "-w"
            | "-x"
            | "-O"
            | "-G"
            | "-N"
    )
}

pub(in crate::executor) fn is_conditional_file_binary(op: &str) -> bool {
    matches!(op, "-nt" | "-ot" | "-ef")
}

/// The arithmetic-comparison operators (`test.c:357 arithcomp`). One owner
/// for the six spellings so the `[[ ]]` dispatch and the metadata pre-pass
/// cannot drift apart.
pub(in crate::executor) fn is_conditional_numeric_op(op: &str) -> bool {
    matches!(op, "-eq" | "-ne" | "-lt" | "-le" | "-gt" | "-ge")
}

pub(in crate::executor) fn conditional_logical_index(args: &[String], op: &str) -> Option<usize> {
    let end = conditional_effective_len(args);
    let mut depth = 0usize;
    for index in (0..end).rev() {
        match args[index].as_str() {
            ")" => depth += 1,
            "(" => depth = depth.saturating_sub(1),
            value if value == op && depth == 0 && index > 0 && index + 1 < end => {
                return Some(index);
            }
            _ => {}
        }
    }
    None
}

pub(in crate::executor) fn conditional_outer_parentheses(args: &[String]) -> Option<&[String]> {
    let end = conditional_effective_len(args);
    if end < 2 || args.first().map(String::as_str) != Some("(") {
        return None;
    }

    let mut depth = 0usize;
    for (index, arg) in args[..end].iter().enumerate() {
        match arg.as_str() {
            "(" => depth += 1,
            ")" => {
                depth = depth.checked_sub(1)?;
                if depth == 0 && index != end - 1 {
                    return None;
                }
            }
            _ => {}
        }
    }

    (depth == 0 && args[end - 1] == ")").then_some(&args[1..end - 1])
}

pub(in crate::executor) fn conditional_regex_operands(args: &[String]) -> Option<(&str, String)> {
    let end = conditional_effective_len(args);
    let op = args[..end].iter().position(|word| word == "=~")?;
    if op != 1 || op + 1 >= end {
        return None;
    }

    Some((args[0].as_str(), args[op + 1..end].join("")))
}

/// True when [`reassemble_extglob_args`] would actually change the token
/// list: some word ending in an extglob operator is immediately followed by
/// a bare `(` token (the exact merge trigger in that function). When false
/// the reassembly is a pure copy, so callers can borrow the original slice
/// and skip the whole-Vec clone (`execute_conditional` runs per `[[ ]]`
/// evaluation).
pub(in crate::executor) fn needs_extglob_reassembly(args: &[String]) -> bool {
    args.windows(2).any(|pair| {
        pair[1] == "("
            && pair[0] != "!"
            && pair[0].ends_with(|c: char| matches!(c, '*' | '?' | '+' | '@' | '!'))
    })
}

/// Reassemble extglob patterns that the lexer split into separate tokens.
/// For example, ["a*", "(", "a", ")"] becomes ["a*(a)"].
pub(in crate::executor) fn reassemble_extglob_args(args: &[String]) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        // Detect extglob: word ending with *?+@! followed by "("
        if i + 1 < args.len()
            && args[i + 1] == "("
            && !(args[i] == "!" || args[i] == "!")
            && args[i].ends_with(|c: char| matches!(c, '*' | '?' | '+' | '@' | '!'))
        {
            // Merge everything before the extglob with adjacent non-operator words
            // If the previous result entry is a non-operator word, merge with it
            let mut merged = if let Some(prev) = result.last() {
                if !is_conditional_operator(prev)
                    && prev != "[["
                    && prev != "]]"
                    && prev != "("
                    && prev != ")"
                {
                    result.pop().unwrap()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            merged.push_str(&args[i]); // prefix like "a" before the operator
            merged.push('(');
            i += 2; // skip the word and "("
            let mut depth = 1usize;
            while i < args.len() && depth > 0 {
                match args[i].as_str() {
                    "(" => {
                        depth += 1;
                        merged.push_str(&args[i]);
                    }
                    ")" => {
                        depth -= 1;
                        merged.push(')');
                    }
                    _ => {
                        if depth > 0 {
                            merged.push_str(&args[i]);
                        }
                    }
                }
                i += 1;
            }
            // Merge trailing non-operator, non-bracket words into the pattern
            while i < args.len()
                && args[i] != "]]"
                && args[i] != "&&"
                && args[i] != "||"
                && !is_conditional_operator(&args[i])
                && args[i] != "("
                && args[i] != ")"
            {
                merged.push_str(&args[i]);
                i += 1;
            }
            result.push(merged);
        } else {
            result.push(args[i].clone());
            i += 1;
        }
    }
    result
}

pub(in crate::executor) fn is_conditional_operator(s: &str) -> bool {
    matches!(
        s,
        "==" | "!="
            | "="
            | "=~"
            | "<"
            | ">"
            | "-eq"
            | "-ne"
            | "-lt"
            | "-le"
            | "-gt"
            | "-ge"
            | "-nt"
            | "-ot"
            | "-ef"
            | "-n"
            | "-z"
            | "-v"
            | "-R"
            | "-o"
            | "-a"
            | "-b"
            | "-c"
            | "-d"
            | "-e"
            | "-f"
            | "-g"
            | "-h"
            | "-L"
            | "-k"
            | "-p"
            | "-r"
            | "-s"
            | "-S"
            | "-t"
            | "-u"
            | "-w"
            | "-x"
            | "-O"
            | "-G"
            | "-N"
            | "-!"
    )
}
pub(in crate::executor) fn conditional_effective_len(args: &[String]) -> usize {
    args.len() - usize::from(args.last().map(String::as_str) == Some("]]"))
}

pub(in crate::executor) fn conditional_pattern_or_string_matches(
    left: &str,
    right: &str,
    nocase: bool,
) -> bool {
    // The byte-spelling gate also guards the no-glob literal compare
    // (rubash#729): a comsub payload sandwiched between literals
    // (`[[ $v == a$(printf '\001')c ]]`) carries no glob syntax, so a
    // pattern-only check would hand the `U+E319<hex>;` protocol spelling
    // to plain equality against the word's literal-byte spelling and
    // never reach normalization. GNU verdicts both operands as byte
    // strings (execute_cmd.c:4043 patmatch -> cond_expand_word ->
    // shmatch.c xstrmatch -> sm_loop.c internal_strmatch on unsigned
    // char buffers); on a glob-free pattern case_pattern_matches
    // degenerates to the same literal compare with both spellings
    // normalized first.
    if pattern_contains_glob(right) || sides_need_byte_char_matching(right, left) {
        if nocase {
            case_pattern_matches_nocase(right, left)
        } else {
            case_pattern_matches(right, left)
        }
    } else if nocase {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}

pub(in crate::executor) fn simple_grep_pattern_matches(line: &str, pattern: &str) -> bool {
    if let Some(pattern) = pattern.strip_prefix('^') {
        line.starts_with(pattern)
    } else {
        line.contains(pattern)
    }
}

pub(in crate::executor) fn restore_numeric_decimal_regex_escapes(pattern: &str) -> String {
    pattern
        .replace("([0-9]*).([0-9]+)", "([0-9]*)\\.([0-9]+)")
        .replace("([0-9]+)(.([0-9]+))?", "([0-9]+)(\\.([0-9]+))?")
        .replace("(.*).(.*)", "(.*)\\.(.*)")
}
