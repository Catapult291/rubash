//! Differential soundness net for the splits_unquoted fast gate (#995).
//! Asserts the gate never reports "no split" when the real splitter
//! would change the word, over an adversarial byte corpus x IFS states.
//! Deterministic seed; ~0.2s.

#[cfg(test)]
mod soundness {
    use crate::executor::Executor;

    fn gate_skips(expanded: &str, ifs: Option<&str>) -> bool {
        // FIX CANDIDATE: never skip on empty expanded (the word may vanish).
        !expanded.is_empty()
            && ifs.is_none_or(|s| s == " \t\n")
            && !expanded.bytes().any(|b| matches!(b, b' ' | b'\t' | b'\n'))
            && !expanded.contains('\u{1c}')
            && !expanded.contains('\u{E401}')
    }

    fn full_would_change(exec: &Executor, expanded: &str) -> bool {
        let fields = exec.field_split_values(expanded);
        fields.len() != 1 || fields.first().is_some_and(|f| f != expanded)
    }

    #[test]
    fn splits_gate_no_false_negative() {
        let alpha: &[char] = &[
            ' ', '\t', '\n', '\r', '\\', 'a', '$', '`', '"', '\'', '(', ')', '[', ']', '*', '?',
            '!', '+', '@', '=', '/', '.', '-', '_', ':', ',', ';', '&', '|', '<', '>', '#', '%',
            '~', '^', 'é', '\u{1c}', '\u{E401}', '\u{E000}', '0',
        ];
        let mut violations: Vec<String> = Vec::new();
        // deterministic PRNG (xorshift)
        let mut state: u64 = 0x1234_5678_9abc_def0;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        // structured seeds first
        let mut corpus: Vec<String> = vec![
            String::new(),
            " ".to_string(),
            "\u{1c}".to_string(),
            "a\u{1c}".to_string(),
            "\u{1c}a".to_string(),
            "\u{E401}".to_string(),
            "a\u{E401}".to_string(),
            "\u{E000}41".to_string(),
            "\u{E000}".to_string(),
            "\\".to_string(),
            "\r".to_string(),
            "\r\n".to_string(),
        ];
        for _ in 0..40000 {
            let len = (next() % 5) as usize;
            let s: String = (0..len)
                .map(|_| alpha[(next() % alpha.len() as u64) as usize])
                .collect();
            corpus.push(s);
        }
        let ifs_states: &[Option<&str>] =
            &[None, Some(" \t\n"), Some(":"), Some(""), Some(" \t\n\r")];
        for ifs in ifs_states {
            let mut exec = Executor::new();
            {
                // set IFS state: Executor::new seeds a default; override per case
                let env = &mut exec.shell_state.env_vars;
                match ifs {
                    None => {
                        env.remove("IFS");
                    }
                    Some(v) => {
                        env.insert("IFS".to_string(), v.to_string());
                    }
                }
            }
            let ifs_view: Option<&str> = match ifs {
                None => None,
                Some(v) => Some(v),
            };
            // NOTE: field_split_values reads IFS from exec env; gate uses ifs_view.
            // For None we removed IFS so both see absent.
            for s in &corpus {
                if full_would_change(&exec, s) && gate_skips(s, ifs_view) {
                    violations.push(format!("ifs={ifs_view:?} expanded={s:?}"));
                    if violations.len() > 15 {
                        break;
                    }
                }
            }
            if violations.len() > 15 {
                break;
            }
        }
        assert!(
            violations.is_empty(),
            "gate false-negatives (full changes, gate skips):\n{}",
            violations.join("\n")
        );
    }
}
