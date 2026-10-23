use super::*;

/// Hidden fast-path builtins (not in BUILTIN_NAMES): the executor's
/// dispatch claims these names even though `type`/`enable` report them
/// as external (rubash#154 identity tools). Word-level substitution
/// fast paths must consult this so `$(uname -s)` runs the engine
/// builtin instead of whatever external happens to sit on PATH.
pub(in crate::executor) fn is_hidden_late_builtin(word: &str) -> bool {
    matches!(word, "uname" | "arch" | "stty")
}

impl Executor {
    /// GNU builtins/test.c `test -v name[sub]` (and `[ -v ... ]`): the `-v`
    /// operand is argv data that already went through word expansion, so its
    /// subscript resolves under the ExpandedOnce rules — verbatim with
    /// array_expand_once (VA_NOEXPAND/ASS_NOEXPAND via SET_VFLAGS,
    /// builtins/common.h:277-289), a deferred expand_subscript_string pass
    /// without it. A failing indexed subscript is an expr.c evalerror that
    /// discards the rest of the command list (`test -v 'a[$x]'; echo x`
    /// never prints `x`); the diagnostic + abort flag are raised inside
    /// rewrite_operand_array_subscript and the builtin returns status 1.
    pub(in crate::executor) fn execute_test_words(
        &mut self,
        args: &[String],
        bracket: bool,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        // W_ARRAYREF (in-band ARRAYREF_FLAG) is a no-op for test/[ — GNU
        // marks arrayref-shaped operands (execute_cmd.c:4366) but test.def
        // never consults it; strip so operand text compares clean. The
        // strip is borrow-only (GNU copies word->flags, it does not touch
        // the text); the owned per-argument clone existed only so the `-v`
        // rewrite could replace an operand in place, so rewrites are kept
        // in a side table and merged once below. Single `Vec<&str>` collect:
        // the old `stripped` + `resolve` + `builtins/test.rs` three-`Vec`
        // chain (GNU test.c test_command works on `argv` in place) is one
        // merge; `-t` presence fuses into the same `-v` scan so `while [ ]`
        // pays no second linear pass.
        fn strip(arg: &str) -> &str {
            crate::builtins::arrayref::take_arrayref_flag(arg).1
        }
        let mut overrides: Vec<(usize, String)> = Vec::new();
        let mut has_t = false;
        let mut index = 0;
        while index < args.len() {
            let current = strip(&args[index]);
            if current == "-t" {
                has_t = true;
            }
            // Operands are skipped after a `-v` (GNU test.c unary_operator
            // consumes the next word), so the operator test never sees a
            // rewritten value — borrow-only compare, no resolve table scan.
            if current == "-v" && index + 1 < args.len() {
                let operand = strip(&args[index + 1]);
                if operand == "-t" {
                    has_t = true;
                }
                match self.rewrite_operand_array_subscript(operand) {
                    Ok(rewritten) => {
                        if rewritten == "-t" {
                            has_t = true;
                        }
                        overrides.push((index + 1, rewritten));
                    }
                    Err(()) => return Ok(1),
                }
                index += 2;
                continue;
            }
            index += 1;
        }
        // Overrides arrive in increasing index order, so merge with a moving
        // pointer: each side advances once (early-exit when past the index),
        // no per-index linear `find` over the table.
        let mut merged: Vec<&str> = Vec::with_capacity(args.len());
        {
            let mut next_override = 0usize;
            for (position, arg) in args.iter().enumerate() {
                while next_override < overrides.len() && overrides[next_override].0 < position {
                    next_override += 1;
                }
                if next_override < overrides.len() && overrides[next_override].0 == position {
                    merged.push(overrides[next_override].1.as_str());
                } else {
                    merged.push(strip(arg));
                }
            }
        }
        // The fd-terminal marks (__RUBASH_FD_TERMINAL_<fd>, consumed ONLY by
        // the `-t` operator arm in builtins/test.rs) are rebuilt by
        // sync_fd_terminal_marks with a full env_vars retain + fd-table walk
        // plus per-endpoint console probes. Every `while [ ... ]` condition
        // paid that on every iteration even though nothing ever reads a
        // mark; refresh them only when this expression can actually consult
        // one (builtins/test.c binop table: `-t` is the only terminal probe;
        // a `-t` in a string-comparison position merely triggers a harmless
        // idempotent refresh). `has_t` fused into the `-v` scan above: no
        // second `any()` pass, skip the sync entirely without `-t`.
        if has_t {
            self.sync_fd_terminal_marks(Some(cmd));
        }
        // GNU execute_cmd.c: test's diagnostics go to the shell's CURRENTLY
        // BOUND fd 2 (redir.c) — an enclosing `exec 2>/dev/null` contains
        // them (rubash#218/#222-era leak: the raw process stderr bypassed
        // every redirect). The write goes through
        // write_buffered_builtin_output so the COMMAND's own `2>file` /
        // `2>/dev/null` is honored too — write_default_stderr only
        // consults the fd-table bindings and leaked the diagnostic to the
        // real stderr under `command test x =~ y 2>/dev/null`
        // (modernish TESTERE.t probe).
        let mut stderr = Vec::new();
        let status = crate::builtins::test::execute_with_stderr(
            &merged,
            bracket,
            &self.shell_state.env_vars,
            &mut stderr,
        )?;
        self.write_buffered_builtin_output(cmd, &[], &stderr)?;
        Ok(status)
    }

    pub(in crate::executor) fn execute_late_builtin_command(
        &mut self,
        cmd: &CommandNode,
        word: &str,
    ) -> Result<(), ExecuteError> {
        match word {
            "let" => {
                // Same fd-2 binding contract as `(( ))` (rubash#1127):
                // let's own redirections apply while its diagnostics run.
                self.exit_code = self.with_compound_output_redirects(cmd, |executor| {
                    Ok(executor.execute_let(&cmd.words[1..]))
                })?;
                Ok(())
            }
            "umask" => {
                if crate::builtins::enable::is_disabled(&self.shell_state.env_vars, "umask") {
                    self.execute_external(cmd)
                } else {
                    self.exit_code = self.execute_umask(cmd)?;
                    Ok(())
                }
            }
            "ulimit" => {
                self.exit_code = self.execute_ulimit(cmd)?;
                Ok(())
            }
            "unset" => {
                self.exit_code = self.execute_unset(cmd)?;
                Ok(())
            }
            "read" => {
                if crate::builtins::enable::is_disabled(&self.shell_state.env_vars, "read") {
                    self.execute_external(cmd)
                } else {
                    self.exit_code = self.execute_read(cmd);
                    Ok(())
                }
            }
            "mapfile" | "readarray" => {
                if crate::builtins::enable::is_disabled(&self.shell_state.env_vars, word) {
                    self.execute_external(cmd)
                } else {
                    // GNU execute_builtin (execute_cmd.c:4787+): redirections
                    // are applied BEFORE the builtin function runs — a
                    // redirect-open failure reports `line N: file: No such
                    // file or directory' and returns 1 without executing
                    // mapfile, so the array is left untouched and option/
                    // identifier validation never happens (rubash#324).
                    self.apply_no_output_builtin_redirects(cmd)?;
                    self.exit_code = self.execute_mapfile(cmd);
                    Ok(())
                }
            }
            "shift" => self.execute_shift_command(cmd),
            "times" => {
                self.exit_code = self.execute_times(cmd)?;
                Ok(())
            }
            "caller" => {
                self.exit_code = self.execute_caller(cmd)?;
                Ok(())
            }
            "jobs" => {
                self.exit_code = self.execute_jobs(cmd)?;
                Ok(())
            }
            "disown" => {
                self.exit_code = self.execute_disown(cmd)?;
                Ok(())
            }
            "wait" => {
                self.exit_code = self.execute_wait(cmd)?;
                Ok(())
            }
            "fg" => {
                self.exit_code =
                    self.execute_fg_bg(cmd, crate::builtins::fg_bg::JobControlBuiltin::Fg)?;
                Ok(())
            }
            "bg" => {
                self.exit_code =
                    self.execute_fg_bg(cmd, crate::builtins::fg_bg::JobControlBuiltin::Bg)?;
                Ok(())
            }
            "suspend" => {
                self.exit_code = self.execute_suspend(cmd)?;
                Ok(())
            }
            "history" => {
                self.exit_code = self.execute_history(cmd)?;
                Ok(())
            }
            "bind" => {
                self.exit_code = self.execute_bind(cmd)?;
                Ok(())
            }
            "fc" => {
                self.exit_code = self.execute_fc(cmd)?;
                Ok(())
            }
            "complete" => {
                self.exit_code = self.execute_completion_builtin(
                    cmd,
                    crate::builtins::complete::CompletionBuiltin::Complete,
                )?;
                Ok(())
            }
            "compgen" => {
                self.exit_code = self.execute_completion_builtin(
                    cmd,
                    crate::builtins::complete::CompletionBuiltin::Compgen,
                )?;
                Ok(())
            }
            "compopt" => {
                self.exit_code = self.execute_completion_builtin(
                    cmd,
                    crate::builtins::complete::CompletionBuiltin::Compopt,
                )?;
                Ok(())
            }
            "time" => {
                self.execute_time_command_node(cmd)?;
                Ok(())
            }
            "trap" => {
                self.exit_code = self.execute_trap(cmd)?;
                Ok(())
            }
            "type" => {
                // niubash #108: route every invocation through the buffered
                // path. Command substitution (`$(type -t ls)`) swaps in
                // `stdout_capture` above this layer and is NOT visible to
                // `command_has_output_redirects`, so gating on explicit
                // redirects leaked the description to the process stdout.
                // `execute_type_with_io` also handles the disabled-builtin
                // state (`enable -n type`) internally.
                self.exit_code = self.execute_type_redirected(cmd)?;
                Ok(())
            }
            "test" => {
                if crate::builtins::enable::is_disabled(&self.shell_state.env_vars, "test") {
                    self.execute_external(cmd)
                } else {
                    self.exit_code = self.execute_test_words(&cmd.words[1..], false, cmd)?;
                    Ok(())
                }
            }
            "[" => {
                if crate::builtins::enable::is_disabled(&self.shell_state.env_vars, "[") {
                    self.execute_external(cmd)
                } else {
                    self.exit_code = self.execute_test_words(&cmd.words[1..], true, cmd)?;
                    Ok(())
                }
            }
            "[[" => {
                // GNU has no `[[` builtin: the word only works when the parser
                // built a conditional command node. `v=9 [[ -n a ]]` or an
                // expanded `[[` therefore hits `[[: command not found` (127)
                // like any other missing command.
                match cmd.conditional_command.as_ref() {
                    Some(command) => {
                        // rubash#846 (GNU execute_cmd.c:854-883): the
                        // `[[ ]]` command's OWN redirect failure — mark it
                        // for the `!` flip sites.
                        if let Err(error) = self.apply_no_output_builtin_redirects(cmd) {
                            if matches!(error, ExecuteError::IoError(_)) {
                                self.note_compound_redirect_failure();
                            }
                            return Err(error);
                        }
                        self.exit_code = self.execute_conditional_command(command);
                        // rubash#1177: same `=~` nounset-fatal conversion
                        // as the with-redirects path.
                        if let Some(result) = self.take_conditional_nounset_fatal() {
                            return result;
                        }
                        Ok(())
                    }
                    None => self.execute_external(cmd),
                }
            }
            "((" => {
                // rubash#846 (GNU execute_cmd.c:854-883): the `(( ))`
                // command's OWN redirect failure — mark it for the `!` flip
                // sites.
                if let Err(error) = self.apply_no_output_builtin_redirects(cmd) {
                    if matches!(error, ExecuteError::IoError(_)) {
                        self.note_compound_redirect_failure();
                    }
                    return Err(error);
                }
                // GNU redir.c do_redirections binds the `(( ))` command's
                // own redirections for its whole execution, and error.c
                // writes the evalerror diagnostic to fd 2 as bound — so
                // `((a+=1)) 2>&1` reports on stdout (rubash#1127). Bind the
                // output redirects in the fd table around the evaluation;
                // the diagnostic printers route through fd 2 (arith_diag).
                let arith_status = self.with_compound_output_redirects(cmd, |executor| {
                    Ok(executor.execute_arithmetic_command(cmd))
                })?;
                self.exit_code = arith_status;
                // GNU expr.c:1190-1216: an unbound variable under `set -u`
                // inside `(( ))` raises FORCE_EOF and terminates the
                // noninteractive shell (probe a2: `set -u; ((b)); echo
                // after` never prints "after"). The status alone would
                // otherwise keep the script running. The exit code is
                // context-owned: 127 only at the `-c` top-level catch
                // (shell.c:1471); script mode exits 1 (eval.c:104-109) and
                // a `( ... )` subshell child contains the jump at
                // execute_cmd.c:1811 and exits 1 (niubash#163).
                if self.shell_state.arithmetic_nounset_error.get() {
                    self.shell_state.arithmetic_nounset_error.set(false);
                    self.shell_state.arithmetic_expansion_error.set(false);
                    // GNU expr.c:1208-1216 expr_streval: interactive_shell
                    // takes `jump_to_top_level (DISCARD)` — the `(( ))` is
                    // abandoned with status 1 and the session keeps
                    // reading; the FORCE_EOF branch is noninteractive
                    // only. The exit code is context-owned: 127 only at
                    // the `-c` top-level catch (shell.c:1471); script mode
                    // exits 1 (eval.c:104-109) and a `( ... )` subshell
                    // child contains the jump at execute_cmd.c:1811 and
                    // exits 1 (niubash#163).
                    if self.expansion_error_is_interactive_discard() {
                        self.exit_code = 1;
                        return Err(ExecuteError::ExpansionFailure(1));
                    }
                    let code = self.expansion_fatal_status();
                    self.exit_code = code;
                    return Err(ExecuteError::ExitCode(code));
                }
                Ok(())
            }
            "dirname" => {
                // The fast paths emulate external commands: with PATH
                // unset they must not resolve (rubash#1126).
                if !crate::executor::path::path_is_unset(&self.shell_state.env_vars) {
                    if let Some(status) = self.try_execute_dirname_fast_path(cmd) {
                        self.exit_code = status;
                        return Ok(());
                    }
                }
                self.execute_external(cmd)
            }
            "basename" => {
                if !crate::executor::path::path_is_unset(&self.shell_state.env_vars) {
                    if let Some(status) = self.try_execute_basename_fast_path(cmd) {
                        self.exit_code = status;
                        return Ok(());
                    }
                }
                self.execute_external(cmd)
            }
            // Bash posix-mode notes (item 19): in POSIX mode a function whose
            // name contains a slash is never executed — the word falls
            // through to the PATH search (`a/b` -> command not found, 127),
            // even though the definition itself stays accepted. The default
            // mode keeps the historical function-first lookup.
            _ if self.shell_state.functions.contains_key(word)
                && !(self.posix_mode_enabled() && word.contains('/')) =>
            {
                self.execute_function(word, &cmd.words[1..], cmd)
            }
            // uname/arch sit AFTER the function guard on purpose (rubash#154):
            // in GNU bash they are external commands, so a shell function
            // named `uname`/`arch` must keep winning over the engine
            // builtin. The `command`-builtin path (command_no_alias_late)
            // bypasses functions by definition and dispatches directly.
            "uname" => {
                if crate::executor::path::path_is_unset(&self.shell_state.env_vars) {
                    return self.execute_external(cmd);
                }
                self.exit_code = self.execute_identity_tool(cmd, "uname");
                Ok(())
            }
            "arch" => {
                if crate::executor::path::path_is_unset(&self.shell_state.env_vars) {
                    return self.execute_external(cmd);
                }
                self.exit_code = self.execute_identity_tool(cmd, "arch");
                Ok(())
            }
            "stty" => {
                if crate::executor::path::path_is_unset(&self.shell_state.env_vars) {
                    return self.execute_external(cmd);
                }
                self.exit_code = self.execute_console_mode_tool(cmd);
                Ok(())
            }
            _ => self.execute_external(cmd),
        }
    }
}
