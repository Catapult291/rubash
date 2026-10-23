use super::*;

impl Executor {
    pub(in crate::executor) fn execute_kill(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        if let Some(status) = self.execute_tracked_background_kill(cmd)? {
            return Ok(status);
        }

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = crate::builtins::kill::execute_with_io_posix(
            &cmd.words[1..],
            &mut stdout,
            &mut stderr,
            self.posix_mode_enabled(),
        )?;
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }

    fn execute_tracked_background_kill(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<Option<i32>, ExecuteError> {
        let Some(request) = kill_request(&cmd.words[1..]) else {
            return Ok(None);
        };
        if request.operands.is_empty() {
            return Ok(None);
        }

        let should_handle = request.operands.iter().any(|operand| {
            operand.starts_with('%')
                || operand
                    .parse::<u32>()
                    .ok()
                    .is_some_and(|pid| self.shell_state.job_table.pid_to_job.contains_key(&pid))
        });
        if !should_handle {
            let mut stderr = Vec::new();
            let mut status = 0;
            for operand in request.operands {
                let Some(pid) = operand.parse::<u32>().ok() else {
                    // Not a u32 pid or job spec — fall through to kill::execute_with_io
                    // for negative pids (Unix process groups) and other formats.
                    return Ok(None);
                };
                if request.check_only && pid == 0 {
                    continue;
                }
                if !process_exists(pid) {
                    writeln!(
                        stderr,
                        "{}kill: ({pid}) - No such process",
                        self.diagnostic_prefix()
                    )?;
                    status = 1;
                }
            }
            self.write_buffered_builtin_output(cmd, &[], &stderr)?;
            if status != 0 || request.check_only {
                return Ok(Some(status));
            }
            return Ok(None);
        }

        let mut stderr = Vec::new();
        let mut status = 0;
        for operand in request.operands {
            let Some(pid) = self.resolve_background_job(&operand) else {
                writeln!(
                    stderr,
                    "{}kill: {operand}: no such job",
                    self.diagnostic_prefix()
                )?;
                // Bash posix-mode quirk (rubash#1054): a jobspec that
                // names no job still prints the diagnostic, but the kill
                // builtin's status stays 0 in posix mode. An invalid PID
                // operand keeps status 1 in both modes.
                if !(operand.starts_with('%') && self.posix_mode_enabled()) {
                    status = 1;
                }
                continue;
            };

            if request.check_only {
                // GNU kill -0 is a real liveness probe (kill(2) with sig 0
                // returns ESRCH for a dead pid), not an unconditional yes.
                // A pid the job table already marked Completed is dead even
                // before the process object is reaped — except a live
                // procsub placeholder (rubash#1022): its status is recorded
                // at registration while the forked zombie stays
                // unreaped (signallable) until an explicit wait or sweep,
                // exactly like GNU's just-exited procsub.
                let job_dead = self
                    .shell_state
                    .job_table
                    .completed_statuses
                    .contains_key(&pid)
                    && !self
                        .shell_state
                        .job_table
                        .procsub_placeholders
                        .contains(&pid);
                if job_dead || !process_exists(pid) {
                    writeln!(
                        stderr,
                        "{}kill: ({pid}) - No such process",
                        self.diagnostic_prefix()
                    )?;
                    status = 1;
                }
                continue;
            }

            let signal_args = vec![format!("-{}", request.signal), pid.to_string()];
            let mut signal_stderr = Vec::new();
            let signal_status = crate::builtins::kill::execute_with_io(
                &signal_args,
                &mut std::io::sink(),
                &mut signal_stderr,
            )?;
            if signal_status != 0 {
                status = signal_status;
                stderr.extend(signal_stderr);
                continue;
            }
            if request.signal == 18 {
                self.shell_state.job_table.mark_running(pid);
            } else if matches!(request.signal, 19 | 20) {
                self.shell_state.job_table.mark_stopped(pid);
            } else if operand.starts_with('%') && self.jobspec_death_is_unreported(request.signal) {
                // rubash#796: a jobspec kill of a DONT_REPORT signal is the
                // kill builtin's own doing, and GNU's non-interactive reader
                // suppresses its completion notice outright (jobs.c
                // notify_of_job_status, the DONT_REPORT_SIGINT/SIGTERM/
                // SIGPIPE + trapped arm: nothing printed, the dead job is
                // cleaned up). Stash the signal status and drop the entry
                // with the status-preserving variant: `kill %1; jobs` lists
                // nothing, no `[1]+ Exit 143` notice fires at the reap
                // boundary, and a later pid-shaped `wait $!` still resolves
                // 143 through completed_statuses (#737's contract). Other
                // signals keep the entry so the async reap announces the
                // death (`kill -9 %1` still surfaces `Killed`); pid
                // operands always keep it for the #737 wrapper-death flow.
                self.shell_state
                    .job_table
                    .mark_completed(pid, 128 + request.signal);
                self.close_coproc_endpoints(pid);
                self.fd_table.close(pid);
                self.shell_state
                    .job_table
                    .remove_job_by_pid_preserve_status(pid);
            }
            continue;
        }

        self.write_buffered_builtin_output(cmd, &[], &stderr)?;
        Ok(Some(status))
    }

    /// rubash#796: does this signal's death go unannounced for a background
    /// job? GNU jobs.c notify_of_job_status suppresses the notice when the
    /// termsig is SIGINT, or SIGPIPE/SIGTERM under config-top.h's
    /// DONT_REPORT_SIGPIPE/DONT_REPORT_SIGTERM, or when the signal is
    /// trapped — the same set report_pending_signal_notices
    /// (job_builtins.rs) already implements for boundary notices. Signal
    /// numbers are the invariant Linux/MSVC values (libc is a unix-only
    /// dependency, and this file compiles on both).
    fn jobspec_death_is_unreported(&self, signal: i32) -> bool {
        const SIGINT: i32 = 2;
        const SIGPIPE: i32 = 13;
        const SIGTERM: i32 = 15;
        if matches!(signal, SIGINT | SIGPIPE | SIGTERM) {
            return true;
        }
        super::trap_exec::signal_trap_name(signal).is_some_and(|name| {
            crate::builtins::trap::get_trap_action(&self.shell_state.env_vars, &name)
                .is_some_and(|action| !action.is_empty())
        })
    }

    pub(in crate::executor) fn execute_ulimit(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = crate::builtins::ulimit::execute_with_io(
            &cmd.words[1..],
            &mut self.shell_state.env_vars,
            &mut stdout,
            &mut stderr,
        )?;
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }
}

#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    crate::builtins::kill::process_exists(pid)
}

#[cfg(windows)]
fn process_exists(pid: u32) -> bool {
    crate::builtins::kill::process_exists(pid)
}

#[cfg(not(any(unix, windows)))]
fn process_exists(_pid: u32) -> bool {
    false
}

struct KillRequest {
    operands: Vec<String>,
    signal: i32,
    check_only: bool,
}

fn kill_request(words: &[String]) -> Option<KillRequest> {
    let mut index = 0;
    let mut signal = 15;
    let mut check_only = false;
    while let Some(word) = words.get(index) {
        if word == "--" {
            index += 1;
            break;
        }
        if word == "-l" || word == "--list" {
            return None;
        }
        if word == "-s" || word == "-n" {
            if words
                .get(index + 1)
                .is_none_or(|signal| crate::builtins::kill::translate_signal(signal).is_none())
            {
                return None;
            }
            let Some(sigspec) = words.get(index + 1) else {
                return None;
            };
            signal = crate::builtins::kill::signal_number_for_spec(sigspec)?;
            check_only |= signal == 0;
            index += 2;
            continue;
        }
        if word.starts_with('-') && word != "-" {
            if word != "-0"
                && crate::builtins::kill::translate_signal(word.trim_start_matches('-')).is_none()
            {
                return None;
            }
            signal = crate::builtins::kill::signal_number_for_spec(word.trim_start_matches('-'))?;
            check_only |= signal == 0;
            index += 1;
            continue;
        }
        break;
    }

    Some(KillRequest {
        operands: words[index..].to_vec(),
        signal,
        check_only,
    })
}
