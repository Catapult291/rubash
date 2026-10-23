//! tilde module.
//!
//! GNU Bash source ownership:
// - lib/tilde/tilde.c
// - lib/tilde/tilde.h

use std::collections::HashMap;

pub const QUOTED_ASSIGNMENT_VALUE: char = crate::executor::markers::QUOTED_WORD_VALUE_PREFIX;

/// The home directory tilde expansion glues, with GNU's missing-vs-empty
/// distinction preserved. GNU lib/tilde/tilde.c:344-356 (tilde_expand_word):
/// a leading `~` or `~/` is *always* translated to `sh_get_env_value("HOME")`
/// — variables.c get_string_value reads the CURRENT variable cell at every
/// expansion, and an existing-but-EMPTY value is still a value (the glue then
/// yields the bare suffix, so `cd ~` becomes `cd ""` and reports "null
/// directory", builtins/cd.def:338). Only a MISSING variable reaches the
/// fallback chain — tilde.c:349-350 consults APPDATA on _WIN32, and
/// tilde.c:354-356 sh_get_home_dir (variables.c:736) reads the passwd
/// database, which is fixed for the uid and never tracks a reassigned $HOME.
/// Rubash's stand-in for that fallback on Windows is USERPROFILE; on Unix
/// there is none yet, so `None` means "GNU would consult the passwd database
/// here and this host has no entry to offer" and callers keep the word
/// verbatim.
pub(crate) fn tilde_home(env_vars: &HashMap<String, String>) -> Option<String> {
    if let Some(home) = env_vars.get("HOME") {
        return Some(home.clone());
    }
    // USERPROFILE is a Windows-only fallback; it is not a shell variable
    // that can be unset, so the env::var fallback is safe there. Unix
    // never consults it (a stray USERPROFILE exported by cross-platform
    // tooling must not hijack `~`).
    #[cfg(windows)]
    {
        if let Some(home) = env_vars.get("USERPROFILE").filter(|v| !v.is_empty()) {
            return Some(home.clone());
        }
        std::env::var("USERPROFILE")
            .ok()
            .filter(|value| !value.is_empty())
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Same value as `tilde_home`, flattened for callers (readline's ~/.inputrc,
/// the `$(cd`/`pwd` fast path) that want "" when no home source exists.
pub fn home_value(env_vars: &HashMap<String, String>) -> String {
    tilde_home(env_vars).unwrap_or_default()
}

pub fn expand_word_prefix(word: &str, env_vars: &HashMap<String, String>) -> Option<String> {
    let rest = word.strip_prefix('~')?;
    // GNU general.c:1185-1238 bash_tilde_find_word / lib/tilde/tilde.c:361
    // isolate_tilde_prefix: the tilde-prefix ends at the first unquoted `/`
    // (or `:` in an assignment value); the remainder glues verbatim. The
    // prefix — not the whole word — is what the special-expansion hook and
    // getpwnam see, so `~+/x` and `~root/x` share one path (rubash#1232).
    let end = rest.find(['/', ':']).unwrap_or(rest.len());
    let (prefix, tail) = (&rest[..end], &rest[end..]);
    if prefix.is_empty() {
        // A bare `~` or leading `~/` always consults HOME (tilde.c:344-358).
        return Some(format!("{}{}", home_value(env_vars), tail));
    }
    if let Some(value) = expand_special_tilde_prefix(prefix, env_vars) {
        return Some(format!("{value}{tail}"));
    }
    let home = passwd_home_for_user(prefix, env_vars)?;
    Some(format!("{home}{tail}"))
}

/// GNU general.c:1119-1136 bash_special_tilde_expansions: the pre-expansion
/// hook runs before getpwnam and owns `~+` (PWD), `~-` (OLDPWD) and the
/// directory stack forms `~N`, `~+N`, `~-N`. GNU
/// builtins/pushd.def:596-642 get_dirstack_index resolves +N from the top
/// (0 = PWD) and -N from the bottom (0 = last stack element); an
/// out-of-range index returns NULL so the word stays literal.
fn expand_special_tilde_prefix(prefix: &str, env_vars: &HashMap<String, String>) -> Option<String> {
    match prefix {
        "+" => return env_vars.get("PWD").cloned(),
        "-" => return env_vars.get("OLDPWD").cloned(),
        _ => {}
    }
    let (from_right, digits) = match prefix.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, prefix.strip_prefix('+').unwrap_or(prefix)),
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let index: i64 = digits.parse().ok()?;
    // DIRSTACK is stored cwd-first in rubash (GNU's pushd_directory_list is
    // cwd-last); len-1-index is the same element in both conventions.
    let stack = crate::builtins::pushd::load_stack(env_vars);
    let len = stack.len() as i64;
    let logical = if from_right { len - 1 - index } else { index };
    if logical < 0 || logical >= len {
        return None;
    }
    stack.get(logical as usize).cloned()
}

/// GNU lib/tilde/tilde.c:361-410: `~user` and `~user/rest` resolve the user
/// through getpwnam and glue the passwd home directory with the rest of the
/// word. The user name ends at the first unquoted `/` or `:`, and the
/// remainder glues verbatim (GNU: `echo ~root:sub` prints `/root:sub`). A
/// user that cannot be found returns None, and the caller keeps the word
/// verbatim (GNU savestring(filename)).
fn expand_user_tilde_word(word: &str, env_vars: &HashMap<String, String>) -> Option<String> {
    let rest = word.strip_prefix('~')?;
    let (user, tail) = match rest.find(['/', ':']) {
        Some(index) => (&rest[..index], Some(&rest[index..])),
        None => (rest, None),
    };
    if user.is_empty() {
        return None;
    }

    let home = passwd_home_for_user(user, env_vars)?;
    Some(match tail {
        Some(tail) => format!("{home}{tail}"),
        None => home,
    })
}

/// Resolve a `~user` prefix to the user's home directory.
///
/// GNU lib/tilde/tilde.c:329 tilde_expand_word(): after the `~`/`~/`
/// fast path, isolate_tilde_prefix extracts the name and tilde.c:379
/// `getpwnam (username)` supplies pw_dir (tilde.c:403 glues it to the
/// rest of the word). No passwd entry -> the expansion fails and the
/// caller keeps the word verbatim (tilde.c:398 savestring(filename)).
///
/// Unix uses the real passwd database via getpwnam_r (the thread-safe
/// spelling of getpwnam). Non-unix/embedded builds keep the winuxcmd
/// `/etc/passwd` file convention below.
#[cfg(unix)]
pub fn passwd_home_for_user(user: &str, _env_vars: &HashMap<String, String>) -> Option<String> {
    use std::ffi::{CStr, CString};

    // A name with an embedded NUL cannot exist in the passwd database.
    let name = CString::new(user).ok()?;

    // getpwnam_r needs a caller buffer; sysconf(_SC_GETPW_R_SIZE_MAX)
    // sizes it, doubling on ERANGE covers hosts reporting -1/undersized.
    let mut buf_len = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    if buf_len < 1024 {
        buf_len = 1024;
    }

    for _ in 0..8 {
        let mut buf = vec![0u8; buf_len as usize];
        let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
        let mut result: *mut libc::passwd = std::ptr::null_mut();
        let status = unsafe {
            libc::getpwnam_r(
                name.as_ptr(),
                &mut pwd,
                buf.as_mut_ptr().cast(),
                buf.len(),
                &mut result,
            )
        };
        if status == libc::ERANGE {
            buf_len = buf_len.saturating_mul(2);
            continue;
        }
        if status != 0 || result.is_null() {
            // Unknown user: no expansion (GNU failure path keeps the word).
            return None;
        }
        let dir = unsafe { CStr::from_ptr(pwd.pw_dir) };
        return Some(dir.to_string_lossy().into_owned());
    }
    None
}

/// Windows has no system passwd database, so `~user` reads the winuxcmd
/// `/etc/passwd` convention: the file resolves through the same
/// shell_path_to_windows mapping the cd builtin uses, which means
/// `__RUBASH_SHELL_ROOT` / `WINUXSH_ROOT` / `RUBASH_ROOT` decide where
/// `/etc` lives. No passwd file ships with the repository — tests create
/// their own fixture under a temporary root and delete it afterwards.
#[cfg(not(unix))]
pub fn passwd_home_for_user(user: &str, env_vars: &HashMap<String, String>) -> Option<String> {
    let path = crate::executor::path::shell_path_to_windows("/etc/passwd", env_vars);
    let content = std::fs::read_to_string(path).ok()?;
    passwd_home_from_content(&content, user)
}

/// passwd(5) line shape: name:password:uid:gid:gecos:home:shell. The home
/// field is index 5. Comment and empty lines are skipped. A matching entry
/// wins even with an empty home field, mirroring getpwnam handing back
/// whatever pw_dir holds.
fn passwd_home_from_content(content: &str, user: &str) -> Option<String> {
    content
        .lines()
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .find_map(|line| {
            let mut fields = line.split(':');
            let name = fields.next()?;
            let _password = fields.next()?;
            let _uid = fields.next()?;
            let _gid = fields.next()?;
            let _gecos = fields.next()?;
            let home = fields.next()?;
            (name == user).then(|| home.to_string())
        })
}

pub fn expand_assignment_value(value: &str, env_vars: &HashMap<String, String>) -> String {
    let Some(value) = value.strip_prefix(QUOTED_ASSIGNMENT_VALUE) else {
        if !assignment_value_needs_tilde_expansion(value, true) {
            return value.to_string();
        }
        return expand_assignment_tilde_value(value, env_vars, true);
    };

    value.to_string()
}

pub fn strip_assignment_quote_marker(value: &str) -> &str {
    value.strip_prefix(QUOTED_ASSIGNMENT_VALUE).unwrap_or(value)
}

pub fn assignment_value_needs_tilde_expansion(value: &str, expand_after_colon: bool) -> bool {
    let value = strip_assignment_quote_marker(value);
    let bytes = value.as_bytes();
    if assignment_tilde_segment_starts_at(bytes, 0) {
        return true;
    }
    if !expand_after_colon {
        return false;
    }

    bytes
        .iter()
        .enumerate()
        .any(|(index, byte)| *byte == b':' && assignment_tilde_segment_starts_at(bytes, index + 1))
}

fn assignment_tilde_segment_starts_at(bytes: &[u8], start: usize) -> bool {
    // GNU expands every unquoted `~` that starts an assignment value or
    // follows a `:` — `~user` included. The expansion pass itself keeps
    // anything the passwd lookup cannot resolve verbatim (the GNU
    // savestring(filename) fallback), so a generous predicate never turns a
    // `~user` assignment into a literal by accident.
    bytes.get(start) == Some(&b'~')
}

pub fn expand_assignment_tilde_value(
    value: &str,
    env_vars: &HashMap<String, String>,
    expand_after_colon: bool,
) -> String {
    if !expand_after_colon {
        return expand_tilde_segment(value, env_vars);
    }

    let mut output = String::new();
    let mut start = 0;
    for (index, ch) in value.char_indices() {
        if index == 0 || ch != ':' {
            continue;
        }
        output.push_str(&expand_tilde_segment(&value[start..index], env_vars));
        output.push(':');
        start = index + ch.len_utf8();
    }
    output.push_str(&expand_tilde_segment(&value[start..], env_vars));
    output
}

pub(crate) fn expand_tilde_segment(segment: &str, env_vars: &HashMap<String, String>) -> String {
    let Some(rest) = segment.strip_prefix('~') else {
        return segment.to_string();
    };

    // The `:` split already happened in expand_assignment_tilde_value, so
    // only `/` terminates the prefix here (GNU bash_tilde_find_word flags=1
    // is equivalent to this per-segment view).
    let end = rest.find('/').unwrap_or(rest.len());
    let (prefix, tail) = (&rest[..end], &rest[end..]);
    if prefix.is_empty() {
        // GNU glue_prefix_and_suffix (tilde.c:304-317): the $HOME prefix
        // glues even when it is the empty string — `HOME=; a=~/x` binds
        // "/x", not the verbatim word. Only a missing fallback (no HOME and
        // no passwd stand-in) leaves the segment verbatim.
        return match tilde_home(env_vars) {
            Some(home) => format!("{home}{tail}"),
            None => segment.to_string(),
        };
    }

    // GNU general.c:1119-1136: the same special-prefix hook applies inside
    // assignment values (`x=~/a:~+/b`, `x=~+1`) (rubash#1232).
    if let Some(value) = expand_special_tilde_prefix(prefix, env_vars) {
        return format!("{value}{tail}");
    }

    // GNU lib/tilde/tilde.c:361-410: `~user[/rest]` resolves through
    // getpwnam. An unresolvable user keeps the segment verbatim.
    match passwd_home_for_user(prefix, env_vars) {
        Some(home) => format!("{home}{tail}"),
        None => segment.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_home_value_prefers_home_before_userprofile() {
        let env_vars = HashMap::from([
            ("HOME".to_string(), "C:/from-home".to_string()),
            ("USERPROFILE".to_string(), "C:/from-userprofile".to_string()),
        ]);

        assert_eq!(home_value(&env_vars), "C:/from-home");
    }

    /// rubash#754 family, GNU lib/tilde/tilde.c:344-356: a set-but-empty
    /// HOME is a VALUE, not a missing variable. `~` expands to "" and `~/x`
    /// to "/x"; only a MISSING variable reaches the USERPROFILE/passwd
    /// fallback chain.
    #[test]
    fn empty_home_value_is_a_value_not_a_fallback_trigger() {
        let env_vars = HashMap::from([("HOME".to_string(), String::new())]);

        assert_eq!(home_value(&env_vars), "");
        assert_eq!(expand_word_prefix("~", &env_vars), Some(String::new()));
        assert_eq!(expand_word_prefix("~/x", &env_vars), Some("/x".to_string()));
        assert_eq!(expand_tilde_segment("~/x", &env_vars), "/x");

        // The USERPROFILE fallback still answers when HOME is MISSING.
        #[cfg(windows)]
        {
            assert!(!home_value(&HashMap::new()).is_empty());
        }
    }

    #[test]
    fn assignment_tilde_candidate_detects_only_expandable_segments() {
        assert!(!assignment_value_needs_tilde_expansion("plain", true));
        assert!(!assignment_value_needs_tilde_expansion("user~name", true));
        assert!(assignment_value_needs_tilde_expansion("~user/bin", true));
        assert!(assignment_value_needs_tilde_expansion("~", true));
        assert!(assignment_value_needs_tilde_expansion("~/bin", true));
        assert!(assignment_value_needs_tilde_expansion("bin:~/tools", true));
        assert!(assignment_value_needs_tilde_expansion(
            "bin:~user/tools",
            true
        ));
        assert!(!assignment_value_needs_tilde_expansion(
            "bin:~/tools",
            false
        ));
    }

    /// Creates a temporary RUBASH_ROOT with an etc/passwd fixture. No passwd
    /// file is ever committed to the repository — the fixture lives in the
    /// process temp directory and each test removes its own root.
    fn fixture_env(user: &str, home: &str) -> (std::path::PathBuf, HashMap<String, String>) {
        // Cargo runs tests as threads of one process, so a per-process name
        // made concurrent fixtures share (and remove_dir_all) each other's
        // root — observed as an intermittent tilde_user_resolves_passwd_home
        // failure. Give every call its own root.
        use std::sync::atomic::{AtomicU32, Ordering};
        static FIXTURE_SEQ: AtomicU32 = AtomicU32::new(0);
        let unique = FIXTURE_SEQ.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "rubash-tilde-user-{}-{}-{}",
            user,
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(root.join("etc")).unwrap();
        std::fs::write(
            root.join("etc").join("passwd"),
            format!(
                "# comment line\nroot:x:0:0:root:/root:/bin/bash\n{user}:x:1000:1000::{}:/bin/rubash\n",
                home.replace('\\', "/")
            ),
        )
        .unwrap();
        let env_vars = HashMap::from([(
            "RUBASH_ROOT".to_string(),
            root.to_string_lossy().to_string(),
        )]);
        (root, env_vars)
    }

    /// The RUBASH_ROOT /etc/passwd fixture is the non-unix/embedded fallback
    /// path; on unix `~user` resolves through getpwnam_r instead (E9,
    /// tilde.c:329 tilde_expand_word -> getpwnam :379) and never consults
    /// the fixture, so these fixture-value assertions are Windows-only.
    #[cfg(windows)]
    #[test]
    fn tilde_user_resolves_passwd_home() {
        let (root, env) = fixture_env("niu", "/c/Users/niu-home");

        assert_eq!(
            expand_word_prefix("~niu", &env),
            Some("/c/Users/niu-home".to_string())
        );
        assert_eq!(
            expand_word_prefix("~niu/docs", &env),
            Some("/c/Users/niu-home/docs".to_string())
        );
        assert_eq!(expand_word_prefix("~missinguser", &env), None);
        assert_eq!(expand_word_prefix("~missinguser/x", &env), None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn tilde_user_word_colon_tail_glues_verbatim() {
        let (root, env) = fixture_env("niu", "/c/Users/niu-home");

        assert_eq!(
            expand_word_prefix("~niu:sub", &env),
            Some("/c/Users/niu-home:sub".to_string())
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn tilde_user_assignment_segments_expand_and_fall_back() {
        let (root, env) = fixture_env("niu", "/c/Users/niu-home");

        assert_eq!(
            expand_assignment_value("~niu/bin", &env),
            "/c/Users/niu-home/bin"
        );
        assert_eq!(
            expand_assignment_value("bin:~niu/tools", &env),
            "bin:/c/Users/niu-home/tools"
        );
        assert_eq!(expand_assignment_value("~missing/x", &env), "~missing/x");
        assert_eq!(
            expand_assignment_value("a:~missing/x:b", &env),
            "a:~missing/x:b"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    /// Unix wiring (E9, GNU tilde.c:329 tilde_expand_word -> getpwnam :379):
    /// `~root` returns the passwd-database home for THIS host -- computed
    /// here through the same libc call so the test pins the wiring rather
    /// than a platform constant (/root on Linux, /var/root on macOS) -- and
    /// a nonexistent user stays unexpanded.
    #[cfg(unix)]
    #[test]
    fn tilde_user_resolves_via_getpwnam_on_unix() {
        let expected = unsafe {
            let entry = libc::getpwnam(b"root\0".as_ptr().cast());
            assert!(!entry.is_null(), "no passwd entry for root on this host");
            std::ffi::CStr::from_ptr((*entry).pw_dir)
                .to_string_lossy()
                .into_owned()
        };
        assert_eq!(
            expand_word_prefix("~root", &HashMap::new()),
            Some(expected.clone())
        );
        assert_eq!(
            expand_word_prefix("~root/sub", &HashMap::new()),
            Some(format!("{expected}/sub"))
        );
        assert_eq!(expand_word_prefix("~nosuchuser", &HashMap::new()), None);
        assert_eq!(expand_word_prefix("~nosuchuser/x", &HashMap::new()), None);
    }
}
