//! Canonical compat-level plumbing.
//!
//! GNU Bash source ownership:
//! - `builtins/shopt.def:654-708 set_compatibility_level()` — `shopt -s/-u
//!   compatNN` clears the sibling flags (mutual exclusion), derives
//!   `shell_compatibility_level` from the remaining flags (priority
//!   31 > 32 > 40 > 41 > 42 > 43 > 44), keeps a 45..52 level across an
//!   unrelated unset (`oldval > 44 && level < default`), and rebinds
//!   `BASH_COMPAT` to the result.
//! - `builtins/shopt.def:710-741 set_compatibility_opts()` — `BASH_COMPAT`
//!   assignment syncs the flags back: levels 50/51/52 (and every level
//!   without a flag: 33-39, 45-49, 53) leave all flags off.
//! - `variables.c:6457-6508 sv_shcompat()` — the `BASH_COMPAT` bind hook:
//!   empty/unset resets to default; only strict `NN` (`44`, `51`) and
//!   `N.N` (`4.2`) spellings parse; anything else (including whitespace
//!   padding like `" 44"` or `"44 "`) and out-of-range values
//!   (`compatval < 31 || compatval > default`) report
//!   `BASH_COMPAT: <val>: compatibility value out of range` and reset the
//!   level to default. The raw value is KEPT (no canonical rebind).
//! - `version.c:54-55` — `default_compatibility_level` (53 for bash-5.3,
//!   via `support/mkversion.sh DEFAULT_COMPAT_LEVEL=${major}${minor}`).
//!
//! Rubash has no integer `shell_compatibility_level` cell, so the level is
//! DERIVED here: a set compatNN shopt flag wins (GNU priority order), else
//! the strict `BASH_COMPAT` parse, else the 5.3 default. Every behavior
//! keyed on the level (`unset` `VA_ALLOWALL`, `test -v` `AV_ATSTARKEYS`,
//! `=~` quoted-RHS mode, ...) reads `compatibility_level()`; every writer
//! goes through `shopt.rs` (`apply_shopt_compat_change` /
//! `sync_compat_flags_from_bash_compat`). No callsite parses `BASH_COMPAT`
//! or reads a compatNN flag directly.

/// GNU `version.c:54` default for bash-5.3 (`DEFAULT_COMPAT_LEVEL` = 53).
pub(crate) const DEFAULT_COMPAT_LEVEL: u32 = 53;

/// GNU `variables.c:6457 MIN_COMPAT_LEVEL`.
pub(crate) const MIN_COMPAT_LEVEL: u32 = 31;

/// The seven GNU `shopt` compat flags in `set_compatibility_level()`'s
/// priority order (`builtins/shopt.def:676-689`). Levels 50/51/52 have no
/// flag (`set_compatibility_opts()` "completeness" arms) — a level in
/// 45..53 without a flag still counts as compat for the `<= 51` checks.
pub(crate) const COMPAT_FLAGS: &[(&str, u32)] = &[
    ("compat31", 31),
    ("compat32", 32),
    ("compat40", 40),
    ("compat41", 41),
    ("compat42", 42),
    ("compat43", 43),
    ("compat44", 44),
];

/// True for the seven GNU compat flag names (no `compat50/51/52`: those are
/// `invalid shell option name`, verified against the 5.3.20 oracle).
pub(crate) fn is_compat_flag(name: &str) -> bool {
    COMPAT_FLAGS.iter().any(|(flag, _)| *flag == name)
}

/// The level a single flag name selects (`None` for non-flag names).
pub(crate) fn level_for_flag(name: &str) -> Option<u32> {
    COMPAT_FLAGS
        .iter()
        .find(|(flag, _)| *flag == name)
        .map(|(_, level)| *level)
}

/// The flag a level selects (`set_compatibility_opts()` switch), or `None`
/// for levels without a flag (33-39, 45-53 — including 50/51/52).
pub(crate) fn flag_for_level(level: u32) -> Option<&'static str> {
    COMPAT_FLAGS
        .iter()
        .find(|(_, flag_level)| *flag_level == level)
        .map(|(flag, _)| *flag)
}

/// True when a builtin `name=value` operand (re)binds BASH_COMPAT: the
/// left-hand side's base name is BASH_COMPAT in any bind shape (scalar,
/// append, element, compound). A bare `declare BASH_COMPAT` declares
/// without binding, so it does not run the hook.
pub(crate) fn binds_bashcompat_operand(operand: &str) -> bool {
    let Some((lhs, _)) = operand.split_once('=') else {
        return false;
    };
    lhs.trim_end_matches('+')
        .split('[')
        .next()
        .is_some_and(|base| base == "BASH_COMPAT")
}

/// GNU `variables.c:6484-6494 sv_shcompat()` strict spelling: exactly two
/// digits (`44`) or `d.d` (`4.4`). No trimming, no sign, no extra chars —
/// `" 44"`, `"44 "`, `"044"`, `"+44"` all fail. Range-checked
/// `MIN_COMPAT_LEVEL..=DEFAULT_COMPAT_LEVEL` (`variables.c:6503`).
/// Returns `None` for anything GNU rejects (caller resets to default).
pub(crate) fn parse_bash_compat_value(raw: &str) -> Option<u32> {
    let bytes = raw.as_bytes();
    let (tens, ones) = match bytes {
        [tens, ones] => (*tens, *ones),
        [tens, b'.', ones] => (*tens, *ones),
        _ => return None,
    };
    if !tens.is_ascii_digit() || !ones.is_ascii_digit() {
        return None;
    }
    let level = u32::from(tens - b'0') * 10 + u32::from(ones - b'0');
    if (MIN_COMPAT_LEVEL..=DEFAULT_COMPAT_LEVEL).contains(&level) {
        Some(level)
    } else {
        None
    }
}

/// The single derived `shell_compatibility_level`: a set compatNN flag wins
/// (GNU priority order — the states `set_compatibility_level()` produces
/// are always mutually exclusive, but the scan order keeps a hand-built
/// multi-flag state deterministic too), else the strict `BASH_COMPAT`
/// parse with an invalid value resetting to default (like `sv_shcompat()`
/// does to the integer while keeping the raw string), else the default.
/// Empty/unset `BASH_COMPAT` is the default (`sv_shcompat()` early arms).
pub(crate) fn derived_level(is_flag_set: impl Fn(&str) -> bool, bash_compat: Option<&str>) -> u32 {
    for (flag, level) in COMPAT_FLAGS {
        if is_flag_set(flag) {
            return *level;
        }
    }
    match bash_compat {
        None => DEFAULT_COMPAT_LEVEL,
        Some(raw) if raw.is_empty() => DEFAULT_COMPAT_LEVEL,
        Some(raw) => parse_bash_compat_value(raw).unwrap_or(DEFAULT_COMPAT_LEVEL),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_spellings_parse() {
        assert_eq!(parse_bash_compat_value("44"), Some(44));
        assert_eq!(parse_bash_compat_value("4.4"), Some(44));
        assert_eq!(parse_bash_compat_value("31"), Some(31));
        assert_eq!(parse_bash_compat_value("3.1"), Some(31));
        assert_eq!(parse_bash_compat_value("51"), Some(51));
        assert_eq!(parse_bash_compat_value("5.1"), Some(51));
        assert_eq!(parse_bash_compat_value("53"), Some(53));
        assert_eq!(parse_bash_compat_value("5.3"), Some(53));
        assert_eq!(parse_bash_compat_value("45"), Some(45));
    }

    #[test]
    fn loose_spellings_reject() {
        // GNU sv_shcompat: no trimming, no signs, exact lengths only.
        for raw in [
            "", "4", "444", "4.44", "44 ", " 44", "+44", "-44", "4,4", "4a", "a4", "5.3 ",
        ] {
            assert_eq!(parse_bash_compat_value(raw), None, "{raw:?}");
        }
    }

    #[test]
    fn out_of_range_rejects() {
        for raw in ["30", "3.0", "54", "5.4", "99", "00"] {
            assert_eq!(parse_bash_compat_value(raw), None, "{raw:?}");
        }
    }

    #[test]
    fn flags_win_over_bash_compat_in_priority_order() {
        let level = derived_level(|flag| flag == "compat44", Some("31"));
        assert_eq!(level, 44);
        let level = derived_level(|flag| flag == "compat31" || flag == "compat44", Some("53"));
        assert_eq!(level, 31);
        let level = derived_level(|_| false, Some("44"));
        assert_eq!(level, 44);
        let level = derived_level(|_| false, Some("99"));
        assert_eq!(level, DEFAULT_COMPAT_LEVEL);
        let level = derived_level(|_| false, Some(""));
        assert_eq!(level, DEFAULT_COMPAT_LEVEL);
        let level = derived_level(|_| false, None);
        assert_eq!(level, DEFAULT_COMPAT_LEVEL);
    }

    #[test]
    fn flagless_levels_map_to_no_flag() {
        for level in [33, 39, 45, 50, 51, 52, 53] {
            assert_eq!(flag_for_level(level), None, "{level}");
        }
        assert_eq!(flag_for_level(44), Some("compat44"));
        assert_eq!(flag_for_level(31), Some("compat31"));
    }
}
