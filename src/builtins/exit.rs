//! `exit` builtin.
//!
//! GNU Bash source ownership:
//! - builtins/exit.def (`exit_builtin`)

use std::io::{self, Write};

const EXECUTION_FAILURE: i32 = 1;
const EX_USAGE: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitAction {
    Exit(i32),
    Continue(i32),
}

/// Execute `exit` with arguments after the command name.
pub fn execute(args: &[String], last_status: i32) -> io::Result<ExitAction> {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    execute_with_io(
        args.iter().map(String::as_str),
        last_status,
        &mut stdout,
        &mut stderr,
    )
}

pub(crate) fn execute_with_io<'a, I, W, E>(
    args: I,
    last_status: i32,
    stdout: &mut W,
    stderr: &mut E,
) -> io::Result<ExitAction>
where
    I: IntoIterator<Item = &'a str>,
    W: Write,
    E: Write,
{
    let args: Vec<&str> = args.into_iter().collect();

    if args.is_empty() {
        return Ok(ExitAction::Exit(normalize_status(last_status)));
    }

    if args[0] == "--help" {
        print_help(stdout)?;
        return Ok(ExitAction::Exit(EX_USAGE));
    }

    // GNU builtins/common.c get_exitstat (bash 5.3): ISOPTION(word, '-')
    // matches "--" exactly and a leading `--` is skipped before the status
    // word — `exit --` exits with $?, `exit -- 1` exits 1 (2026-10-18
    // probes X4/X5). `exit -` is not skipped: valid_number("-") fails and
    // it becomes the numeric argument error.
    let rest = if args[0] == "--" {
        &args[1..]
    } else {
        &args[..]
    };

    if rest.is_empty() {
        return Ok(ExitAction::Exit(normalize_status(last_status)));
    }

    let status = match parse_exit_status(rest[0]) {
        Some(status) => status,
        None => {
            // The executor's execute_exit_command_word intercepts this
            // shape with the reader prolog (`bash: line N:` /
            // `<script>: line N:`); this direct-call fallback keeps the
            // canonical shell name for the rare non-reader callers and
            // the unit tests below. The previous helper read
            // __RUBASH_* from the PROCESS environment, which the engine
            // only keeps in its internal env map — it always fell back,
            // dropping the line segment (issue #705 R7.4).
            writeln!(
                stderr,
                "{}exit: {}: numeric argument required",
                "bash: ", rest[0]
            )?;
            return Ok(ExitAction::Exit(EX_USAGE));
        }
    };

    if rest.len() > 1 {
        // Same shape: the executor intercepts the too-many error with the
        // reader prolog before this builtin path runs.
        writeln!(stderr, "{}exit: too many arguments", "bash: ")?;
        return Ok(ExitAction::Continue(EXECUTION_FAILURE));
    }

    Ok(ExitAction::Exit(status))
}

fn print_help<W>(stdout: &mut W) -> io::Result<()>
where
    W: Write,
{
    writeln!(stdout, "exit: exit [n]")?;
    writeln!(stdout, "    Exit the shell.")?;
    writeln!(stdout, "    ")?;
    writeln!(
        stdout,
        "    Exits the shell with a status of N.  If N is omitted, the exit status"
    )?;
    writeln!(stdout, "    is that of the last command executed.")
}

fn parse_exit_status(arg: &str) -> Option<i32> {
    let value = arg.parse::<i128>().ok()?;
    Some(normalize_status(value))
}

pub(crate) fn normalize_status<T>(status: T) -> i32
where
    T: Into<i128>,
{
    status.into().rem_euclid(256) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str], last_status: i32) -> (ExitAction, String) {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let action =
            execute_with_io(args.iter().copied(), last_status, &mut stdout, &mut stderr).unwrap();
        (action, String::from_utf8(stderr).unwrap())
    }

    #[test]
    fn exits_with_last_status_without_arguments() {
        assert_eq!(run(&[], 42).0, ExitAction::Exit(42));
    }

    #[test]
    fn normalizes_numeric_status_to_eight_bits() {
        assert_eq!(run(&["258"], 0).0, ExitAction::Exit(2));
        assert_eq!(run(&["-1"], 0).0, ExitAction::Exit(255));
    }

    #[test]
    fn invalid_numeric_argument_exits_with_usage() {
        let (action, stderr) = run(&["abc"], 0);

        assert_eq!(action, ExitAction::Exit(EX_USAGE));
        assert!(stderr.contains("numeric argument required"));
    }

    #[test]
    fn too_many_arguments_does_not_exit() {
        let (action, stderr) = run(&["1", "2"], 0);

        assert_eq!(action, ExitAction::Continue(EXECUTION_FAILURE));
        assert!(stderr.contains("too many arguments"));
    }
}
