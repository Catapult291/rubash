use super::super::*;
use std::fs;
use std::path::Path;

#[test]
fn test_dirstack_expands_as_dynamic_array() {
    let _cwd_guard = CwdGuard::take();
    let output_path = "target/rubash-dirstack-array-output.txt";
    let _ = fs::remove_file(output_path);
    let input = format!(
        "PWD=/tmp/rubash-dirstack; \
         printf '%s:%s:%s\\n' \"${{!DIRSTACK[@]}}\" \"${{#DIRSTACK[@]}}\" \"${{DIRSTACK[0]}}\" > {output_path}; \
         DIRSTACK[0]=/tmp/rubash-dirstack-updated; \
         printf '%s:%s:%s\\n' \"${{!DIRSTACK[@]}}\" \"${{#DIRSTACK[@]}}\" \"${{DIRSTACK[0]}}\" >> {output_path}; \
         declare -p DIRSTACK >> {output_path}"
    );
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(
        fs::read_to_string(output_path).unwrap(),
        "0:1:/tmp/rubash-dirstack\n0:1:/tmp/rubash-dirstack-updated\ndeclare -a DIRSTACK=([0]=\"/tmp/rubash-dirstack-updated\")\n"
    );
    let _ = fs::remove_file(output_path);
}

#[test]
fn test_dirs_redirects_stderr() {
    let _cwd_guard = CwdGuard::take();
    let error_path = "target/rubash-dirs-stderr-output.txt";
    let status_path = "target/rubash-dirs-stderr-status.txt";
    let _ = fs::remove_file(error_path);
    let _ = fs::remove_file(status_path);
    let input = format!("dirs bad 2> {error_path}; echo $? > {status_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    // #509: GNU dirs.def rejects the operand through EX_USAGE — status 2,
    // not the plain builtin failure 1 this test used to pin (WSL GNU
    // 5.3.0: `dirs bad` -> `dirs: bad: invalid option` + usage, rc 2).
    assert_eq!(fs::read_to_string(status_path).unwrap(), "2\n");
    let error = fs::read_to_string(error_path).unwrap();
    assert!(error.contains("dirs: bad: invalid option"));
    assert!(error.contains("dirs: usage:"));
    let _ = fs::remove_file(error_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_dirs_appends_stderr() {
    let _cwd_guard = CwdGuard::take();
    let error_path = "target/rubash-dirs-stderr-append-output.txt";
    let _ = fs::remove_file(error_path);
    fs::write(error_path, "before\n").unwrap();
    let input = format!("dirs bad 2>> {error_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    // #509: EX_USAGE (2), see the redirect twin above.
    assert_eq!(executor.last_exit_code(), 2);
    let error = fs::read_to_string(error_path).unwrap();
    assert!(error.starts_with("before\n"));
    assert!(error.contains("dirs: bad: invalid option"));
    assert!(error.contains("dirs: usage:"));
    let _ = fs::remove_file(error_path);
}

#[test]
fn test_pushd_redirects_stderr() {
    let _cwd_guard = CwdGuard::take();
    let error_path = "target/rubash-pushd-stderr-output.txt";
    let status_path = "target/rubash-pushd-stderr-status.txt";
    let _ = fs::remove_file(error_path);
    let _ = fs::remove_file(status_path);
    let input = format!("pushd +9 2> {error_path}; echo $? > {status_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(status_path).unwrap(), "1\n");
    let error = fs::read_to_string(error_path).unwrap();
    assert!(error.contains("pushd: +9: directory stack index out of range"));
    let _ = fs::remove_file(error_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_pushd_n_accepts_double_dash_before_directory() {
    let _cwd_guard = CwdGuard::take();
    let output_path = "target/rubash-pushd-n-double-dash-output.txt";
    let _ = fs::remove_file(output_path);
    let input = format!("PWD=/; pushd -n -- /tmp > {output_path}; dirs -p >> {output_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(
        fs::read_to_string(output_path).unwrap(),
        "/ /tmp\n/\n/tmp\n"
    );
    let _ = fs::remove_file(output_path);
}

#[test]
fn test_pushd_accepts_existing_relative_directory() {
    let _cwd_guard = CwdGuard::take();
    let output_path = "target/rubash-pushd-relative-output.txt";
    let _ = fs::remove_file(output_path);
    let input =
        format!("pushd . > {output_path}; echo $? >> {output_path}; dirs -p >> {output_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    let output = fs::read_to_string(output_path).unwrap();
    let lines: Vec<_> = output.lines().collect();
    // wt33 (#374): pushd/dirs report the shell (/d/…) domain since #224.
    let cwd = std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
        .replacen("D:/", "/d/", 1)
        .to_ascii_lowercase();
    assert_eq!(
        lines.first().copied(),
        Some(format!("{cwd} {cwd}").as_str())
    );
    assert_eq!(lines.get(1), Some(&"0"));
    assert_eq!(lines.get(2), Some(&cwd.as_str()));
    let _ = fs::remove_file(output_path);
}

#[test]
fn test_popd_appends_stderr() {
    let _cwd_guard = CwdGuard::take();
    let error_path = "target/rubash-popd-stderr-append-output.txt";
    let _ = fs::remove_file(error_path);
    fs::write(error_path, "before\n").unwrap();
    let input = format!("popd +9 2>> {error_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 1);
    let error = fs::read_to_string(error_path).unwrap();
    assert!(error.starts_with("before\n"));
    // rubash#830: with the display down to the cwd entry the offset is 0, so
    // GNU's pushd_error (pushd.def:540-546) names "directory stack empty"
    // regardless of the operand — `popd +9` on a spent stack (WSL GNU bash
    // 5.3.0: "popd: directory stack empty", rc 1); the out-of-range wording
    // is reserved for a non-zero offset.
    assert!(error.contains("popd: directory stack empty"));
    let _ = fs::remove_file(error_path);
}

#[test]
fn test_popd_double_dash_ignores_following_stack_index() {
    let _cwd_guard = CwdGuard::take();
    // rubash#830: the top-pop popd now cds for real, so the shell process's
    // cwd moves mid-script (`pushd /tmp; pushd /bin; popd` lands the process
    // on the pop target). Redirect paths resolved against the moving cwd
    // would scatter (and, under `>>`, accumulate across runs) — anchor the
    // harness files at the crate's target directory in the shell's POSIX
    // spelling, which resolves identically from every cwd; the Rust-side
    // fs:: probes use the native form of the same location.
    let manifest = env!("CARGO_MANIFEST_DIR");
    let anchor = {
        let bytes = manifest.as_bytes();
        format!(
            "/{}{}",
            (bytes[0] as char).to_ascii_lowercase(),
            manifest[2..].replace('\\', "/")
        )
    };
    let output_name = "rubash-popd-double-dash-index-output.txt";
    let scratch_name = "rubash-popd-double-dash-index-scratch.txt";
    let output_path = format!("{anchor}/target/{output_name}");
    let scratch_path = format!("{anchor}/target/{scratch_name}");
    let output_native = Path::new(manifest).join("target").join(output_name);
    let scratch_native = Path::new(manifest).join("target").join(scratch_name);
    let _ = fs::remove_file(&output_native);
    let _ = fs::remove_file(&scratch_native);
    let input = format!(
        "PWD=/; pushd /tmp > {scratch_path}; pushd /bin >> {scratch_path}; \
         popd -- +8 > {output_path}; popd -- -8 >> {output_path}; echo $? >> {output_path}"
    );
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(
        fs::read_to_string(&output_native).unwrap(),
        "/tmp /\n/\n0\n"
    );
    let _ = fs::remove_file(&output_native);
    let _ = fs::remove_file(&scratch_native);
}

#[test]
fn test_popd_n_removes_next_directory_without_cd() {
    let _cwd_guard = CwdGuard::take();
    let output_path = "target/rubash-popd-n-output.txt";
    let scratch_path = "target/rubash-popd-n-scratch.txt";
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(scratch_path);
    let input = format!(
        "PWD=/; pushd -n /tmp > {scratch_path}; popd -n -- > {output_path}; dirs -p >> {output_path}"
    );
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(output_path).unwrap(), "/\n/\n");
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(scratch_path);
}

#[test]
fn test_popd_n_empty_stack_returns_failure() {
    let _cwd_guard = CwdGuard::take();
    let error_path = "target/rubash-popd-n-empty-error.txt";
    let status_path = "target/rubash-popd-n-empty-status.txt";
    let _ = fs::remove_file(error_path);
    let _ = fs::remove_file(status_path);
    let input = format!("PWD=/; popd -n 2> {error_path}; echo $? > {status_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(status_path).unwrap(), "1\n");
    let error = fs::read_to_string(error_path).unwrap();
    assert!(error.contains("popd: directory stack empty"));
    let _ = fs::remove_file(error_path);
    let _ = fs::remove_file(status_path);
}

#[test]
fn test_kill_redirects_output() {
    let _cwd_guard = CwdGuard::take();
    let output_path = "target/rubash-kill-redirect-output.txt";
    let _ = fs::remove_file(output_path);
    let input = format!("kill -l HUP > {output_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(output_path).unwrap(), "1\n");
    let _ = fs::remove_file(output_path);
}

#[test]
fn test_kill_appends_output() {
    let _cwd_guard = CwdGuard::take();
    let output_path = "target/rubash-kill-append-output.txt";
    let _ = fs::remove_file(output_path);
    fs::write(output_path, "before\n").unwrap();
    let input = format!("kill -l HUP >> {output_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(output_path).unwrap(), "before\n1\n");
    let _ = fs::remove_file(output_path);
}

#[test]
fn test_kill_translates_exit_signal() {
    let _cwd_guard = CwdGuard::take();
    let output_path = "target/rubash-kill-exit-signal-output.txt";
    let _ = fs::remove_file(output_path);
    let input = format!("kill -l 0 > {output_path}; kill -l EXIT >> {output_path}");
    let tokens = tokenize(&input);
    let ast = parse(&tokens);
    let mut executor = Executor::new();

    let result = executor.execute_ast(&ast);

    assert!(result.is_ok());
    assert_eq!(executor.last_exit_code(), 0);
    assert_eq!(fs::read_to_string(output_path).unwrap(), "EXIT\n0\n");
    let _ = fs::remove_file(output_path);
}
