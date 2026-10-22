//! wait module.
//!
//! GNU Bash source ownership:
// - builtins/wait.def

use std::io::{self, Write};

const EXECUTION_SUCCESS: i32 = 0;
const EXECUTION_FAILURE: i32 = 1;
const EX_USAGE: i32 = 2;
const EXECUTION_NOTFOUND: i32 = 127;

pub fn execute_with_io<E>(
    args: &[String],
    diagnostic_prefix: &str,
    stderr: &mut E,
) -> io::Result<i32>
where
    E: Write,
{
    let mut index = 0;
    let mut wait_any = false;
    while let Some(arg) = args.get(index) {
        if arg == "--" {
            index += 1;
            break;
        }
        if !arg.starts_with('-') || arg == "-" {
            break;
        }

        for (offset, option) in arg[1..].char_indices() {
            match option {
                'f' => {}
                'n' => wait_any = true,
                'p' => {
                    let value_start = 1 + offset + option.len_utf8();
                    if value_start < arg.len() {
                        break;
                    }
                    index += 1;
                    if args.get(index).is_none() {
                        return Ok(EXECUTION_SUCCESS);
                    }
                    break;
                }
                other => {
                    writeln!(stderr, "{diagnostic_prefix}wait: -{other}: invalid option")?;
                    writeln!(stderr, "wait: usage: wait [-fn] [-p var] [id ...]")?;
                    return Ok(EX_USAGE);
                }
            }
        }
        index += 1;
    }

    let operands = &args[index..];
    if operands.is_empty() {
        return Ok(if wait_any {
            EXECUTION_NOTFOUND
        } else {
            EXECUTION_SUCCESS
        });
    }

    let mut status = EXECUTION_SUCCESS;
    // GNU wait.def set_waitlist / jobs.c get_job_spec: an operand is a pid
    // only when it is a nonempty digit string; the empty string (like any
    // other non-numeric text) is "`': not a pid or valid job spec" with
    // EXECUTION_FAILURE, never the 127 "not a child" arm (rubash#1118 —
    // `chars().all(..)` on an empty operand is vacuously true and took the
    // digit arm).
    for operand in operands {
        if operand.starts_with('%') {
            writeln!(stderr, "{diagnostic_prefix}wait: {operand}: no such job")?;
            status = EXECUTION_NOTFOUND;
        } else if !operand.is_empty() && operand.chars().all(|ch| ch.is_ascii_digit()) {
            writeln!(
                stderr,
                "{diagnostic_prefix}wait: pid {operand} is not a child of this shell"
            )?;
            status = EXECUTION_NOTFOUND;
        } else {
            writeln!(
                stderr,
                "{diagnostic_prefix}wait: `{operand}': not a pid or valid job spec"
            )?;
            status = EXECUTION_FAILURE;
        }
    }

    Ok(status)
}
