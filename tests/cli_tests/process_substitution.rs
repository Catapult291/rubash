#[test]
fn c_command_exec_persistent_output_process_substitution_receives_later_writes() {
    let output = crate::rubash_command()
        .arg("-c")
        .arg("exec 9> >(read -r line; printf 'line=%s\\n' \"$line\"); printf 'hello\\n' >&9; exec 9>&-; wait")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "line=hello\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn assignment_output_process_substitution_feeds_external_stdin() {
    let output = crate::rubash_command()
        .arg("-c")
        .arg(r#"f() { cat "$1" >"$x"; }; x=>(tr '[:lower:]' '[:upper:]') f <(printf 'hi there\n')"#)
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "HI THERE\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn exec_replacing_stderr_process_substitution_flushes_previous_target() {
    let output = crate::rubash_command()
        .arg("-c")
        .arg("rm -f __rubash_psubst_err.tmp; exec 4>&2; exec 2> >(tee __rubash_psubst_err.tmp); echo hello >&2; exec 2>&4; cat __rubash_psubst_err.tmp; rm -f __rubash_psubst_err.tmp")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello\nhello\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn coproc_input_move_marks_array_endpoint_closed() {
    let output = crate::rubash_command()
        .arg("-c")
        .arg("coproc C { echo hi; }; exec 4<&${C[0]}-; read -r x <&4; printf 'x=%s arr=%s\\n' \"$x\" \"${C[*]}\"")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    // GNU coproc pairs land on high fds below 64 (jobs.c move_to_high_fd
    // policy; `coproc C { echo hi; }` binds 63 60, so after `exec 4<&${C[0]}-`
    // moves+closes the read end the array reads `-1 60`). WSL GNU Bash 5.3.0
    // probe 2026-09-27: `x=hi arr=-1 60`.
    assert_eq!(String::from_utf8_lossy(&output.stdout), "x=hi arr=-1 60\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn coproc_child_inherits_function_table_for_debug_trap() {
    // unixwin/rubash#441: GNU coproc forks, so the coprocess shell inherits
    // the full function table. A DEBUG trap installed before the coproc
    // (bash-preexec's `__bp_preexec_invoke_exec`) must resolve inside the
    // child instead of falling through to a PATH lookup and reporting
    // `command not found`. GNU Bash 5.2.37 (msys) oracle probe
    // 2026-10-09: `PREEXEC: inside-coproc`, no diagnostics on stderr.
    let output = crate::rubash_command()
        .arg("-c")
        .arg(
            "preexec() { echo \"PREEXEC: $1\" >&2; }; \
             trap 'preexec \"$BASH_COMMAND\"' DEBUG; \
             coproc C { preexec inside-coproc; echo coproc-ready; }; \
             read -r out <&${C[0]}; \
             echo \"got: $out\"",
        )
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    // The child executes the body's DEBUG trap successfully (the parent's
    // own DEBUG traps for the surrounding commands also log to stderr).
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stdout, "got: coproc-ready\n", "stdout was: {stdout}");
    assert!(
        stderr.contains("PREEXEC: inside-coproc"),
        "stderr was: {stderr}"
    );
    assert!(
        !stderr.contains("command not found"),
        "stderr was: {stderr}"
    );
}

#[test]
fn coproc_function_snapshot_is_one_way_into_child() {
    // The function table rides to the coprocess as a BASH_FUNC snapshot;
    // the child sees it but the parent's table and exported set are
    // untouched by the spawn (fork snapshot semantics).
    let output = crate::rubash_command()
        .arg("-c")
        .arg(
            "parent_fn() { echo p; }; \
             coproc C { parent_fn; }; \
             read -r out <&${C[0]}; \
             echo \"child: $out\"; \
             declare -F parent_fn >/dev/null && echo parent-keeps-fn; \
             export -p | grep -q 'BASH_FUNC_parent_fn' || echo no-parent-export",
        )
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout, "child: p\nparent-keeps-fn\nno-parent-export\n",
        "stdout was: {stdout}"
    );
}

#[test]
fn exec_output_process_substitution_body_inherits_fork_time_stdio() {
    // unixwin/rubash#1336 (eco-harvest installIllumioVEN asset):
    // GNU subst.c:6362 `process_substitute` forks the `>(...)` body while
    // the redirect list is being applied, so the child inherits the
    // shell's fds as of that point. The deferred body run must rebind
    // fds 1/2 to those fork-time snapshots: with `exec > >(body) 2>err`,
    // the body's stdout lands on the shell's original stdout and its
    // stderr on the shell's original stderr — not in the temp carrier
    // (which is deleted right after the run). (TO_OUT feeds the body's
    // stdin, which this body ignores — GNU eats it the same way.)
    let output = crate::rubash_command()
        .arg("-c")
        .arg("rm -f __rubash_psubst_fork_stdio.err; exec > >(echo BODYOUT; echo BODYERR >&2) 2>__rubash_psubst_fork_stdio.err; echo TO_OUT; echo TO_ERR >&2")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "BODYOUT\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "BODYERR\n");
    let err_file = std::fs::read_to_string("__rubash_psubst_fork_stdio.err").unwrap_or_default();
    assert_eq!(err_file, "TO_ERR\n");
    let _ = std::fs::remove_file("__rubash_psubst_fork_stdio.err");
}

#[test]
fn exec_output_process_substitution_body_stderr_follows_redirect_order() {
    // GNU redir.c applies redirects left to right and subst.c:6362 forks
    // during application: in `exec 2>err > >(body)` the body's stderr is
    // already the err file when the body is forked (verified against GNU
    // bash 5.3: the body's diagnostics land in err, not on the original
    // stderr).
    let output = crate::rubash_command()
        .arg("-c")
        .arg("rm -f __rubash_psubst_order.err; exec 2>__rubash_psubst_order.err > >(echo BODYOUT; echo BODYDIAG >&2); echo TO_OUT; echo TO_ERR >&2")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "BODYOUT\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let err_file = std::fs::read_to_string("__rubash_psubst_order.err").unwrap_or_default();
    assert_eq!(err_file, "TO_ERR\nBODYDIAG\n");
    let _ = std::fs::remove_file("__rubash_psubst_order.err");
}

#[test]
fn exec_file_replace_runs_pending_output_process_substitution_body() {
    // GNU redir.c: replacing a descriptor closes the old open file
    // description, so a pending `>(...)` body sees EOF and flushes.
    let output = crate::rubash_command()
        .arg("-c")
        .arg("rm -f __rubash_psubst_replace.out; exec > >(read -r line; printf 'got=%s\\n' \"$line\"); echo HI; exec >__rubash_psubst_replace.out; echo BYE")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "got=HI\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let out_file = std::fs::read_to_string("__rubash_psubst_replace.out").unwrap_or_default();
    assert_eq!(out_file, "BYE\n");
    let _ = std::fs::remove_file("__rubash_psubst_replace.out");
}

#[test]
fn exec_shared_output_process_substitution_body_runs_once() {
    // `exec > >(body) 2>&1` shares one pipe across fds 1/2 (GNU dup2):
    // only the last writer's close delivers EOF, so the body runs once.
    let output = crate::rubash_command()
        .arg("-c")
        .arg("exec > >(echo BODYRAN) 2>&1; echo AFTER")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "BODYRAN\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn exec_combined_replace_runs_pending_output_process_substitution_body() {
    // `exec &>file` after `exec > >(...)`: the pending body flushes with
    // the pre-replace streams before stderr joins stdout on the file.
    let output = crate::rubash_command()
        .arg("-c")
        .arg("rm -f __rubash_psubst_combined.out; exec > >(read -r line; printf 'got=%s\\n' \"$line\"); echo HI; exec &>__rubash_psubst_combined.out; echo BYE")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "got=HI\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let out_file = std::fs::read_to_string("__rubash_psubst_combined.out").unwrap_or_default();
    assert_eq!(out_file, "BYE\n");
    let _ = std::fs::remove_file("__rubash_psubst_combined.out");
}

#[test]
fn exec_output_process_substitution_body_runs_at_explicit_exit() {
    // The installIllumioVEN HANG asset shape: `exec > >(tee log) 2>&1`
    // followed by `exit 0` must flush the buffered banner through the
    // body (GNU exit closes the descriptors; the forked tee sees EOF).
    let output = crate::rubash_command()
        .arg("-c")
        .arg("rm -f __rubash_psubst_exit.tmp; exec > >(tee __rubash_psubst_exit.tmp) 2>&1; echo BANNER; exit 0")
        .output()
        .expect("run rubash");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "BANNER\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let log_file = std::fs::read_to_string("__rubash_psubst_exit.tmp").unwrap_or_default();
    assert_eq!(log_file, "BANNER\n");
    let _ = std::fs::remove_file("__rubash_psubst_exit.tmp");
}
