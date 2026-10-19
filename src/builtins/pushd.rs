//! pushd module.
//!
//! GNU Bash source ownership:
// - builtins/pushd.def

mod parse;
mod stack;

pub(crate) use stack::{load_stack, save_stack, set_stack_value, stack_value, stack_words};

use parse::{parse_popd_operand, parse_pushd_operand, PopdOperand, PushdOperand};
use stack::{
    is_stack_index, logical_dir_exists, normalize_stack_dir, resolved_index, set_pwd_from_stack,
    stack_index, strip_double_dash,
};
use std::collections::HashMap;
use std::io::{self, Write};

const EXECUTION_SUCCESS: i32 = 0;
const EXECUTION_FAILURE: i32 = 1;
/// GNU error.h EX_USAGE: a builtin's invalid number / usage error exits 2
/// (builtins/common.c get_numeric_arg -> builtin_error("%s: invalid number")
/// + return EX_USAGE). dirs/popd/pushd share that path for every ±N operand.
const EX_USAGE: i32 = 2;
pub(crate) const DIR_STACK: &str = "__RUBASH_DIR_STACK";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackBuiltin {
    Pushd,
    Popd,
    Dirs,
}

pub fn execute(
    builtin: StackBuiltin,
    args: &[String],
    env_vars: &mut HashMap<String, String>,
    diagnostic_prefix: &str,
) -> io::Result<i32> {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    execute_with_io(
        builtin,
        args.iter().map(String::as_str),
        env_vars,
        diagnostic_prefix,
        &mut stdout,
        &mut stderr,
    )
}

pub(crate) fn execute_with_io<'a, I, W, E>(
    builtin: StackBuiltin,
    args: I,
    env_vars: &mut HashMap<String, String>,
    diagnostic_prefix: &str,
    stdout: &mut W,
    stderr: &mut E,
) -> io::Result<i32>
where
    I: IntoIterator<Item = &'a str>,
    W: Write,
    E: Write,
{
    let args: Vec<&str> = args.into_iter().collect();
    let mut stack = load_stack(env_vars);

    match builtin {
        StackBuiltin::Dirs => {
            let args = strip_double_dash(&args);
            if args.first().copied() == Some("-c") {
                save_stack(env_vars, &[]);
                return Ok(EXECUTION_SUCCESS);
            }
            // GNU dirs.def walks the display flags off the front and they
            // combine (`dirs -l -v` prints the numbered long form); the
            // first remaining word is the optional ±N operand, anything
            // else after the flags is rejected below. A ±-shaped word goes
            // through get_numeric_arg (builtins/common.c): a non-number is
            // `%s: invalid number` + usage, EX_USAGE — and dirs.def's
            // caller turns EX_USAGE into exit status 2. `dirs -l +q` must
            // NOT silently consume the operand (issue #509: it printed the
            // stack and exited 0 where GNU exits 2).
            let mut flag_end = 0;
            let mut printable = false;
            let mut numbered = false;
            while let Some(arg) = args.get(flag_end).copied() {
                match arg {
                    // rubash#759: -l is the LONG form, not the one-per-line
                    // form. GNU dirs.def only widens the rendering (it stops
                    // collapsing $HOME to ~); the separator stays a single
                    // space, and -p alone selects one entry per line. rubash's
                    // stack always stores full paths, so -l has no separate
                    // rendering at all and must not set `printable`:
                    // `dirs -l` printed three lines where GNU prints one.
                    "-l" => {}
                    "-p" => printable = true,
                    "-v" => numbered = true,
                    _ => break,
                }
                flag_end += 1;
            }
            let rest = &args[flag_end..];
            if let Some(arg) = rest.first().copied() {
                if arg.starts_with('+') || arg.starts_with('-') {
                    if !is_stack_index(arg) {
                        writeln!(stderr, "{diagnostic_prefix}dirs: {arg}: invalid number")?;
                        writeln!(stderr, "dirs: usage: dirs [-clpv] [+N] [-N]")?;
                        return Ok(EX_USAGE);
                    }
                    let Some(index) = stack_index(arg, stack.len()) else {
                        writeln!(
                            stderr,
                            "{diagnostic_prefix}dirs: {}: directory stack index out of range",
                            arg.trim_start_matches(['+', '-'])
                        )?;
                        return Ok(EXECUTION_FAILURE);
                    };
                    if numbered {
                        writeln!(stdout, "{index:2}  {}", stack[index])?;
                    } else {
                        writeln!(stdout, "{}", stack[index])?;
                    }
                    return Ok(EXECUTION_SUCCESS);
                }
                writeln!(stderr, "{diagnostic_prefix}dirs: {arg}: invalid option")?;
                writeln!(stderr, "dirs: usage: dirs [-clpv] [+N] [-N]")?;
                return Ok(EX_USAGE);
            }
            if numbered {
                for (index, dir) in stack.iter().enumerate() {
                    writeln!(stdout, "{index:2}  {dir}")?;
                }
            } else if printable {
                for dir in &stack {
                    writeln!(stdout, "{dir}")?;
                }
            } else {
                // load_stack always exposes the cwd as element 0, so the
                // plain (and -l) listing joins the stack; the old dead
                // empty-stack arm only ever echoed $PWD.
                writeln!(stdout, "{}", stack.join(" "))?;
            }
            Ok(EXECUTION_SUCCESS)
        }
        StackBuiltin::Pushd => {
            let operand = parse_pushd_operand(&args, diagnostic_prefix, stderr)?;
            let Some(operand) = operand else {
                // The parse printed its usage diagnostic: GNU pushd.def
                // returns get_numeric_arg's EX_USAGE (2), not a plain
                // builtin failure (issue #509).
                return Ok(EX_USAGE);
            };

            match operand {
                PushdOperand::Swap => {
                    if stack.len() < 2 {
                        writeln!(stderr, "{diagnostic_prefix}pushd: no other directory")?;
                        return Ok(EXECUTION_FAILURE);
                    }
                    stack.swap(0, 1);
                    set_pwd_from_stack(env_vars, &stack, true);
                }
                PushdOperand::Index {
                    index,
                    from_right,
                    no_cd,
                } => {
                    let index =
                        resolved_index(index, from_right, stack.len()).unwrap_or(usize::MAX);
                    if index >= stack.len() {
                        writeln!(
                            stderr,
                            "{diagnostic_prefix}pushd: {}: directory stack index out of range",
                            args.last().copied().unwrap_or_default()
                        )?;
                        return Ok(EXECUTION_FAILURE);
                    }
                    if no_cd {
                        save_stack(env_vars, &stack);
                        return Ok(EXECUTION_SUCCESS);
                    }
                    stack.rotate_left(index);
                    set_pwd_from_stack(env_vars, &stack, true);
                }
                PushdOperand::Dir { dir, no_cd } => {
                    // GNU pushd.def:296-302 delegates the Dir operand to
                    // cd_builtin (`j = ((flags & NOCD) == 0) ? cd_builtin
                    // (...) : EXECUTION_SUCCESS`), so an operand that
                    // expanded to the empty string inherits cd.def:335's
                    // verdict — `builtin_error ("null directory")` +
                    // EXECUTION_FAILURE, with the prolog naming pushd, the
                    // executing builtin. (`HOME=; pushd ~` used to silently
                    // succeed as a no-op, rubash#754 family.)
                    if dir.is_empty() && !no_cd {
                        writeln!(stderr, "{diagnostic_prefix}pushd: null directory")?;
                        return Ok(EXECUTION_FAILURE);
                    }
                    // pushd.def:301: with -n, cd_builtin is never called and
                    // `add_dirstack_element (savestring (list->word->word))`
                    // pushes the RAW operand word unconditionally — no
                    // normalization, no existence probe (that probe lives on
                    // the cd path only). An empty expansion therefore stays
                    // "" on the stack (`HOME=; pushd -n ~` yields
                    // `<cwd> ""`), and `pushd -n ./sub` stores "./sub".
                    let resolved_dir = if no_cd {
                        dir.clone()
                    } else {
                        let resolved = normalize_stack_dir(&dir, env_vars);
                        if !logical_dir_exists(&resolved, env_vars) {
                            writeln!(
                                stderr,
                                "{diagnostic_prefix}pushd: {dir}: No such file or directory"
                            )?;
                            return Ok(EXECUTION_FAILURE);
                        }
                        resolved
                    };
                    let old_pwd = stack
                        .first()
                        .cloned()
                        .or_else(|| env_vars.get("PWD").cloned())
                        .unwrap_or_else(|| "/".to_string());
                    if stack.is_empty() {
                        stack.push(old_pwd.clone());
                    }
                    if no_cd && !stack.is_empty() {
                        stack.insert(1, resolved_dir);
                    } else {
                        stack.insert(0, resolved_dir.clone());
                        env_vars.insert("OLDPWD".to_string(), old_pwd);
                        env_vars.insert("PWD".to_string(), resolved_dir);
                    }
                }
            }

            save_stack(env_vars, &stack);
            writeln!(stdout, "{}", stack.join(" "))?;
            Ok(EXECUTION_SUCCESS)
        }
        StackBuiltin::Popd => {
            let operand = parse_popd_operand(&args, diagnostic_prefix, stderr)?;
            let Some(operand) = operand else {
                // Same EX_USAGE contract as pushd above (issue #509).
                return Ok(EX_USAGE);
            };

            // GNU pushd.def:383-391 resolves every ±N operand to one array
            // slot before branching, and slot 0 (`popd`, `popd +0`,
            // `popd -N` naming the bottom of the display) is the top-of
            // -stack branch. The empty-stack verdict is pushd_error's
            // (pushd.def:540-546): "directory stack empty" whenever the
            // offset is 0, "directory stack index out of range" otherwise.
            let (index, no_cd) = match operand {
                PopdOperand::Top { no_cd } => (0usize, no_cd),
                PopdOperand::Index {
                    index,
                    from_right,
                    no_cd,
                } => match resolved_index(index, from_right, stack.len()) {
                    Some(index) => (index, no_cd),
                    None => {
                        popd_range_error(
                            stack.len(),
                            args.last().copied().unwrap_or_default(),
                            diagnostic_prefix,
                            stderr,
                        )?;
                        return Ok(EXECUTION_FAILURE);
                    }
                },
            };

            if index == 0 {
                if stack.len() <= 1 {
                    writeln!(stderr, "{diagnostic_prefix}popd: directory stack empty")?;
                    return Ok(EXECUTION_FAILURE);
                }
                // With -n the element is freed and nothing else happens
                // (pushd.def:397: cd_to_string is skipped entirely).
                if no_cd {
                    stack.remove(1);
                    save_stack(env_vars, &stack);
                    writeln!(stdout, "{}", stack.join(" "))?;
                    return Ok(EXECUTION_SUCCESS);
                }
                // pushd.def:394-403: cd_to_string(new top) runs BEFORE the
                // element is freed, so a failed chdir leaves the stack, PWD
                // and the display untouched — cd_builtin's diagnostic (under
                // the invoking builtin's prolog: `popd: word: strerror`) and
                // its EXECUTION_FAILURE are the only effects (rubash#830;
                // GNU bash 5.3.0 probes: rc 1, `dirs` still shows the entry).
                let target = stack[1].clone();
                let Some(chdir) = popd_chdir(&target, env_vars, stderr, diagnostic_prefix)? else {
                    return Ok(EXECUTION_FAILURE);
                };
                // Success: cd_builtin's bindpwd refreshed OLDPWD/PWD and the
                // display promotes the chdir target to the cwd slot — the
                // same net stack the GNU free() of the popped word leaves.
                let old_pwd = env_vars
                    .get("PWD")
                    .cloned()
                    .unwrap_or_else(|| "/".to_string());
                env_vars.insert("OLDPWD".to_string(), old_pwd);
                stack.remove(0);
                env_vars.insert("PWD".to_string(), stack[0].clone());
                match chdir {
                    // A real chdir drops the physical-pwd override so
                    // `pwd -P` follows the real process cwd (cd.rs does the
                    // same after a chdir).
                    PopdChdir::Real => {
                        env_vars.remove("__RUBASH_PHYSICAL_PWD");
                    }
                    // cd's Windows-host bridge for the POSIX system names
                    // (cd.rs logical_posix_test_dir) pins the physical pwd to
                    // the logical name instead of chdir(2); popd's inner cd
                    // inherits that verdict verbatim.
                    PopdChdir::Bridged => {
                        env_vars.insert("__RUBASH_PHYSICAL_PWD".to_string(), stack[0].clone());
                    }
                }
                save_stack(env_vars, &stack);
                writeln!(stdout, "{}", stack.join(" "))?;
                return Ok(EXECUTION_SUCCESS);
            }

            // Any other slot (pushd.def:404-421): the element is freed and
            // the remainder shifted; no chdir ever runs and PWD stays as it
            // was (GNU 5.3.0 probe: `popd +1` leaves pwd -P and $PWD alone).
            stack.remove(index);
            save_stack(env_vars, &stack);
            writeln!(stdout, "{}", stack.join(" "))?;
            Ok(EXECUTION_SUCCESS)
        }
    }
}

/// GNU pushd_error (pushd.def:540-546): an out-of-range operand names
/// "directory stack empty" while the offset is 0 and sh_erange's "directory
/// stack index out of range" otherwise.
fn popd_range_error<E: Write>(
    stack_len: usize,
    operand: &str,
    diagnostic_prefix: &str,
    stderr: &mut E,
) -> io::Result<()> {
    if stack_len <= 1 {
        writeln!(stderr, "{diagnostic_prefix}popd: directory stack empty")
    } else {
        writeln!(
            stderr,
            "{diagnostic_prefix}popd: {operand}: directory stack index out of range"
        )
    }
}

/// How the top-of-stack popd's inner cd (GNU cd_to_string) resolved.
enum PopdChdir {
    /// set_current_dir moved the shell process.
    Real,
    /// cd's Windows-host bridge (cd.rs logical_posix_test_dir): with no
    /// configured logical root, `/bin`-class POSIX names are not physical
    /// directories, so cd rewrites PWD and pins __RUBASH_PHYSICAL_PWD
    /// without a chdir — popd's inner cd inherits the same verdict.
    Bridged,
}

/// The top-of-stack popd chdir, GNU cd_to_string (pushd.def:559-572): a
/// cd_builtin call with the raw stack word. Returns None when the chdir
/// failed (diagnostic already printed).
fn popd_chdir<E: Write>(
    target: &str,
    env_vars: &mut HashMap<String, String>,
    stderr: &mut E,
    diagnostic_prefix: &str,
) -> io::Result<Option<PopdChdir>> {
    if target.is_empty() {
        // cd.def:331-336 judges an operand that expanded to "" before any
        // path work; through cd_to_string the prolog names popd.
        writeln!(stderr, "{diagnostic_prefix}popd: null directory")?;
        return Ok(None);
    }
    // cd.rs's Windows-host bridge, same predicate and same match list: on an
    // unconfigured host the POSIX system names have no physical directory,
    // and cd reports success by rewriting the logical state only.
    if cfg!(windows)
        && !crate::executor::path::shell_root_configured(env_vars)
        && matches!(target, "/" | "/bin" | "/etc" | "/tmp" | "/usr")
    {
        return Ok(Some(PopdChdir::Bridged));
    }
    let native = crate::executor::path::shell_path_to_windows(target, env_vars);
    if let Err(error) = std::env::set_current_dir(&native) {
        // cd.def:427: builtin_error("%s: %s", dirname, strerror(e)) reports
        // the stored word, not the Windows-mapped path.
        writeln!(
            stderr,
            "{diagnostic_prefix}popd: {target}: {}",
            crate::posix_errors::message(&error)
        )?;
        stderr.flush()?;
        return Ok(None);
    }
    Ok(Some(PopdChdir::Real))
}
