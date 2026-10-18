//! shift module.
//!
//! GNU Bash source ownership:
// - builtins/shift.def

use std::io::{self, Write};

pub enum ShiftAction {
    Complete(i32),
    Shift(usize),
}

/// GNU error.h: EX_USAGE (2) marks a usage error, EXECUTION_FAILURE (1) a
/// plain runtime failure. shift.def routes a non-numeric operand through
/// get_numeric_arg (builtins/common.c:501-505 `sh_neednumarg` + `return 0`
/// → `return (EX_USAGE)`) and a surplus operand through `no_args`
/// (builtins/common.c `set_exit_status (EX_BADUSAGE)`), so both exit 2;
/// only the shift-count-out-of-range erange path returns
/// EXECUTION_FAILURE.
const EX_USAGE: i32 = 2;

pub fn execute(args: &[String]) -> io::Result<ShiftAction> {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    execute_with_io(args, &mut stdout, &mut stderr)
}

pub(crate) fn execute_with_io<W, E>(
    args: &[String],
    stdout: &mut W,
    stderr: &mut E,
) -> io::Result<ShiftAction>
where
    W: Write,
    E: Write,
{
    if args.first().map(String::as_str) == Some("--help") {
        crate::builtins::help::print_shift_help_with_io(stdout)?;
        return Ok(ShiftAction::Complete(2));
    }

    // GNU builtins/shift.def: strip leading `--` argument terminator before
    // processing the shift count. `shift -- 5` is equivalent to `shift 5`.
    let args: &[String] = if args.first().map(String::as_str) == Some("--") {
        &args[1..]
    } else {
        args
    };

    if args.len() > 1 {
        writeln!(stderr, "{}shift: too many arguments", diagnostic_prefix())?;
        // get_numeric_arg (builtins/common.c:511) ends the operand walk with
        // no_args(), whose surplus-argument failure is EX_BADUSAGE.
        return Ok(ShiftAction::Complete(EX_USAGE));
    }

    let amount = match args.first() {
        Some(arg) => match arg.parse::<isize>() {
            Ok(amount) if amount >= 0 => amount as usize,
            Ok(_) => {
                writeln!(
                    stderr,
                    "{}shift: {arg}: shift count out of range",
                    diagnostic_prefix()
                )?;
                return Ok(ShiftAction::Complete(1));
            }
            Err(_) => {
                writeln!(
                    stderr,
                    "{}shift: {arg}: numeric argument required",
                    diagnostic_prefix()
                )?;
                // shift.def: `get_numeric_arg (list, 0, &times) == 0`
                // (non-numeric operand, builtins/common.c:501-505) returns
                // EX_USAGE, not EXECUTION_FAILURE (issue #759 item 3: GNU
                // 5.3.0 `( set -- 1 2; shift x )` exits 2).
                return Ok(ShiftAction::Complete(EX_USAGE));
            }
        },
        None => 1,
    };
    Ok(ShiftAction::Shift(amount))
}

fn diagnostic_prefix() -> String {
    if let (Ok(script), Ok(line)) = (
        std::env::var("__RUBASH_SCRIPT_NAME"),
        std::env::var("__RUBASH_CURRENT_LINE"),
    ) {
        return format!("{script}: line {line}: ");
    }

    "rubash: ".to_string()
}
