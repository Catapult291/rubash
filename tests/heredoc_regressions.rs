use std::{env, process::Command};

#[test]
fn nested_then_sequential_heredocs_keep_fifo_ownership() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg("if true; then cat <<A; fi\none\nA\ncat <<B\ntwo\nB")
        .output()
        .expect("run nested sequential heredoc probe");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "one\ntwo\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn unquoted_heredoc_preserves_raw_c0_parameter_payload() {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(
            r#"x=$(printf '\025'); cat <<EOF | od -An -tx1
$x
EOF"#,
        )
        .output()
        .expect("run heredoc raw payload probe");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        " 15 0a
"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn unterminated_heredoc_with_backslash_newline_warns_at_last_physical_line() {
    // rubash#1028: file lines 1-5 arbitrary, `cat <<EOF` on line 6, body
    // `line1\` + `line2`, no terminator. GNU bash 5.3.20 prints the joined
    // body `line1line2` and warns at line 8 (the last PHYSICAL body line:
    // read_a_line joins `\<newline>` while collecting, parse.y) with
    // "at line 6" naming the gather line (make_cmd.c:626).
    let script_path = "target/rubash-1028-heredoc-backslash-warn.sh";
    std::fs::write(
        script_path,
        "# line1\n# line2\n# line3\n# line4\n# line5\ncat <<EOF\nline1\\\nline2\n",
    )
    .expect("write heredoc warning probe script");
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(script_path)
        .output()
        .expect("run heredoc backslash-newline warning probe");
    let _ = std::fs::remove_file(script_path);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "line1line2\n");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(
            "line 8: warning: here-document at line 6 delimited by end-of-file (wanted `EOF')"
        ),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
