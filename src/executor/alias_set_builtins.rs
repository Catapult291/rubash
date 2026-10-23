use super::*;

impl Executor {
    pub(in crate::executor) fn execute_unalias(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = crate::builtins::alias::unalias_with_io(
            &cmd.words[1..],
            &mut self.shell_state.aliases,
            &mut stderr,
        )?;
        self.clear_alias_definition_lines(&cmd.words[1..]);
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }

    fn clear_alias_definition_lines(&mut self, args: &[String]) {
        let mut index = 0;
        let mut clear_all = false;
        while let Some(arg) = args.get(index) {
            if arg == "--" {
                index += 1;
                break;
            }
            if !arg.starts_with('-') || arg == "-" {
                break;
            }
            if arg[1..].chars().all(|option| option == 'a') {
                clear_all = true;
            }
            index += 1;
        }

        // rubash#817: each definition carries a (line, read-generation)
        // marker pair — clear both halves.
        if clear_all {
            self.shell_state.env_vars.retain(|name, _| {
                !name.starts_with("__RUBASH_ALIAS_LINE_")
                    && !name.starts_with("__RUBASH_ALIAS_GEN_")
            });
            return;
        }

        for name in &args[index..] {
            self.shell_state
                .env_vars
                .remove(&format!("__RUBASH_ALIAS_LINE_{name}"));
            self.shell_state
                .env_vars
                .remove(&format!("__RUBASH_ALIAS_GEN_{name}"));
        }
    }

    pub(in crate::executor) fn execute_alias(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let posix = self.posix_mode_enabled();
        let status = crate::builtins::alias::alias_with_io(
            &cmd.words[1..],
            &mut self.shell_state.aliases,
            &mut stdout,
            &mut stderr,
            posix,
        )?;
        self.record_alias_definition_lines(&cmd.words[1..], cmd.line);
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        Ok(status)
    }

    fn record_alias_definition_lines(&mut self, args: &[String], line: Option<usize>) {
        let Some(line) = line else {
            return;
        };
        // rubash#817: the line alone is ambiguous across nested read units
        // (a sourced file restarts at 1; an eval string renumbers
        // caller_line + i - 1), so the definition also records the
        // read-unit generation it was consumed by — the guard suppresses
        // only when BOTH match, which is GNU's same-read rule
        // (parse.y read-time expansion).
        let generation = self.shell_state.read_generation;
        let mut index = 0;
        while let Some(arg) = args.get(index) {
            if arg == "--" {
                index += 1;
                continue;
            }
            if arg.starts_with('-') && arg != "-" {
                index += 1;
                continue;
            }
            if let Some((name, _)) = arg.split_once('=') {
                if !name.is_empty() {
                    self.shell_state
                        .env_vars
                        .insert(format!("__RUBASH_ALIAS_LINE_{name}"), line.to_string());
                    self.shell_state
                        .env_vars
                        .insert(format!("__RUBASH_ALIAS_GEN_{name}"), generation.to_string());
                }
            }
            index += 1;
        }
    }

    pub(in crate::executor) fn execute_set(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<i32, ExecuteError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = crate::builtins::set::set_with_io(
            cmd.words[1..].iter().map(String::as_str),
            &mut self.shell_state.env_vars,
            &mut stdout,
            &mut stderr,
        )?;
        if cmd.words.len() == 1 {
            // GNU set.def set_builtin with no arguments calls
            // print_var_list over ALL variables — shell functions are
            // variables in GNU's table and print after the scalars, in
            // name order, through print_function_def (rubash#1056).
            let mut names: Vec<&String> = self.shell_state.functions.keys().collect();
            names.sort();
            for name in names {
                if let Some(body) = self.shell_state.functions.get(name) {
                    let commands = body.commands.clone();
                    self.write_function_definition(name, &commands, false, &mut stdout)?;
                }
            }
        }
        self.write_buffered_builtin_output(cmd, &stdout, &stderr)?;
        self.load_history_file_if_needed();
        // GNU set.def returns EX_USAGE (258 > EX_SHERRBASE) for invalid
        // options/option names; execute_cmd.c:4888 sets special_builtin_failed
        // for `set` (a POSIX special builtin). Rubash's set_with_io returns
        // the converted status (EX_BADUSAGE = 2); mark the failure so the
        // POSIX non-interactive shell exits.
        if status == crate::builtins::set::EX_USAGE {
            self.special_builtin_failed.set(true);
        }
        Ok(status)
    }

    /// bashhist.c load_history: the first "set -o history" in a session
    /// reads HISTFILE into the list (only when no lines have been recorded
    /// yet and the file has not already been loaded).
    pub(in crate::executor) fn load_history_file_if_needed(&mut self) {
        let Some(session) = self.shell_state.session_history.clone() else {
            return;
        };
        if !crate::builtins::set::shell_option_enabled(&self.shell_state.env_vars, "history") {
            return;
        }
        let needs_load = {
            let shell = session.borrow_mut();
            !shell.histfile_loaded && shell.lines_this_session == 0
        };
        if !needs_load {
            return;
        }
        let Some(path) = self.get_env("HISTFILE") else {
            return;
        };
        let histsize = crate::history::SessionHistory::size_limit(self.get_env("HISTSIZE"));
        let mut shell = session.borrow_mut();
        shell.histfile_loaded = true;
        let _ = shell.load_file(&path, histsize);
    }

    pub(in crate::executor) fn execute_set_command(
        &mut self,
        cmd: &CommandNode,
    ) -> Result<(), ExecuteError> {
        // GNU execute_cmd.c:5584 execute_builtin_or_function runs
        // do_redirections BEFORE the builtin (a failed redirect reports
        // EX_REDIRFAIL without running it); the buffered builtin applies
        // those redirect side effects in write_buffered_builtin_output.
        // The fast paths below used to return without it, silently
        // dropping `set ... > f` (no file, no error -- rubash#1020).
        // Routing every redirected `set` to the full path instead is
        // wrong: the full set_with_io never assigns positional params,
        // so `set -- -a -b > f` would lose $@. Route every fast path
        // through the same redirect application the slow path uses;
        // with no redirects it is a proven no-op skip.
        if crate::builtins::set::shell_option_enabled(&self.shell_state.env_vars, "restricted")
            && cmd.words[1..]
                .iter()
                .any(|word| word.starts_with('+') && word[1..].chars().any(|flag| flag == 'r'))
        {
            self.exit_code = self.execute_set(cmd)?;
            return Ok(());
        }
        if self.apply_simple_set_flags(&cmd.words[1..]) {
            self.write_buffered_builtin_output(cmd, &[], &[])?;
            self.exit_code = 0;
            return Ok(());
        }
        if self.apply_set_positional_operands(&cmd.words[1..]) {
            self.write_buffered_builtin_output(cmd, &[], &[])?;
            self.exit_code = 0;
            return Ok(());
        }
        if cmd.words.get(1).map(String::as_str) == Some("--") {
            // TODO(builtins/set.def/variables.c): `set --` replaces shell
            // positional parameters. Full set option parsing lives in
            // builtins::set; this branch covers upstream source tests that
            // inspect `$@`.
            self.set_positional_params(cmd.words[2..].to_vec());
            self.write_buffered_builtin_output(cmd, &[], &[])?;
            self.exit_code = 0;
            return Ok(());
        }
        self.exit_code = self.execute_set(cmd)?;
        Ok(())
    }
}
