//! suspend module.
//!
//! GNU Bash source ownership:
// - builtins/suspend.def suspend_builtin (`killpg (shell_pgrp, SIGSTOP)`
//   on -f; `sh_nojobs` failure without job control otherwise)

use std::io::{self, Write};

const EXECUTION_SUCCESS: i32 = 0;
const EXECUTION_FAILURE: i32 = 1;
const EX_USAGE: i32 = 2;

pub fn execute_with_io<E>(
    args: &[String],
    diagnostic_prefix: &str,
    stderr: &mut E,
) -> io::Result<i32>
where
    E: Write,
{
    let mut force = false;
    for arg in args {
        if arg == "--" {
            break;
        }
        if !arg.starts_with('-') || arg == "-" {
            break;
        }
        for option in arg[1..].chars() {
            match option {
                'f' => force = true,
                other => {
                    writeln!(
                        stderr,
                        "{diagnostic_prefix}suspend: -{other}: invalid option"
                    )?;
                    writeln!(stderr, "suspend: usage: suspend [-f]")?;
                    return Ok(EX_USAGE);
                }
            }
        }
    }

    if force {
        // GNU builtins/suspend.def suspend_builtin: with -f the login-shell
        // and no-job-control guards are skipped and the shell stops its own
        // process group — `killpg (shell_pgrp, SIGSTOP)` — returning SUCCESS
        // once continued (rubash#1097). shell_pgrp is this process's group
        // (jobs.c:4737 `shell_pgrp = getpgid (0)`); SIGSTOP/SIGCONT ride the
        // kernel defaults (never in kill.rs's managed set), so no trap
        // bookkeeping is needed. Non-unix has no SIGSTOP: keep the
        // historical success no-op there.
        #[cfg(unix)]
        {
            // SAFETY: getpgid/killpg with our own pgrp are pure queries.
            let pgrp = unsafe { libc::getpgid(0) };
            if pgrp > 0 {
                unsafe {
                    libc::killpg(pgrp, libc::SIGSTOP);
                }
            }
        }
        return Ok(EXECUTION_SUCCESS);
    }

    writeln!(
        stderr,
        "{diagnostic_prefix}suspend: cannot suspend: no job control"
    )?;
    Ok(EXECUTION_FAILURE)
}
