//! The invocation-aware `--help` / `--version` surface (issue #585,
//! rubash#596): GNU answers both off argv[0]'s basename (shell.c:477-482
//! show_shell_usage(stdout, extra=1), version.c:88-104), so the bash/sh
//! invocation faces print the GNU forms while product names keep the
//! product help. This module lives in the LIB so every binary that embeds
//! the engine answers identically — the niu host used to print its product
//! help under `bash --help` because the fork lived in this crate's bin
//! target only (owner report 2026-10-14).

/// The engine's accepted GNU long options (shell.c:264-274) — shared by the
/// argv parser and the usage table `show_shell_help` prints.
pub const LONG_OPTIONS: &[&str] = &[
    "debug",
    "debugger",
    "dump-po-strings",
    "dump-strings",
    "help",
    "init-file",
    "login",
    "noediting",
    "noprofile",
    "norc",
    "posix",
    "pretty-print",
    "rcfile",
    "restricted",
    "verbose",
    "version",
];

/// Long options that consume the following argv entry (type Charp in
/// shell.c:264,274).
pub const LONG_OPTIONS_WITH_VALUE: &[&str] = &["init-file", "rcfile"];

/// The usage-contract shell name for this invocation, when `--help` must
/// print the GNU usage form (issue #585). GNU keys the fork off argv[0]'s
/// basename (shell.c set_shell_name): the `.exe` spelling is a Windows
/// artifact and never reaches the usage lines, the `-` login marker is
/// KEPT in the display name (empirical 5.3.0 oracle: `exec -a -bash bash
/// --help` prints `Usage:\t-bash ...`), and the bash-vs-sh decision uses
/// the dash-stripped lowercased stem so `-BASH.EXE` still forks. Any other
/// name (rubash, niu, ...) returns None and keeps the product help.
pub fn invocation_help_name(argv0: Option<&str>) -> Option<String> {
    let basename = argv0?.rsplit(['/', '\\']).next()?;
    let stem = if basename.len() >= 4 && basename.to_ascii_lowercase().ends_with(".exe") {
        &basename[..basename.len() - 4]
    } else {
        basename
    };
    let decided = stem.strip_prefix('-').unwrap_or(stem);
    match decided.to_ascii_lowercase().as_str() {
        "bash" | "sh" => Some(stem.to_string()),
        _ => None,
    }
}

/// GNU show_shell_usage (shell.c:2057-2103) with extra=1 — the `--help`
/// form, issue #585: printed to STDOUT and exited EXECUTION_SUCCESS
/// (shell.c:477-482), unlike the usage-error form above (stderr, no
/// version/extra block). The version line uses the engine's own version
/// string and machtype in GNU's `version %s-(%s)` shape
/// (shell.c:2071-2072 `_("GNU bash, version %s-(%s)\n")`); the long-option
/// table is the engine's accepted LONG_OPTIONS set, and the Shell options
/// block matches the usage-error contract byte for byte. The bash/sh names
/// in the usage and `Type ...' lines follow the invocation (shell_name,
/// shell.c:2084-2088, 2094-2095); the home-page lines keep GNU's literal
/// `bash home page` text. No Windows-only resources are referenced: the
/// same strings render in the unix cross-compile lanes.
///
/// `version` is the caller's runtime `$BASH_VERSION` when it has an
/// executor; `None` (embedders without one, e.g. the niu launcher help
/// path) falls back to the engine's advertised version value — the same
/// literal `init.rs` seeds the environment with.
pub fn show_shell_help(shell_name: &str, version: Option<&str>) {
    let version = version
        .map(str::to_string)
        .unwrap_or_else(crate::executor::bash_version_value);
    println!(
        "GNU bash, version {version}-({})",
        crate::executor::machtype_value()
    );
    println!("Usage:\t{shell_name} [GNU long option] [option] ...");
    println!("\t{shell_name} [GNU long option] [option] script-file ...");
    println!("GNU long options:");
    for name in LONG_OPTIONS {
        println!("	--{name}");
    }
    println!("Shell options:");
    println!("	-ilrsD or -c command or -O shopt_option		(invocation only)");
    println!("	-abefhkmnptuvxBCEHPT or -o option");
    println!("Type `{shell_name} -c \"help set\"' for more information about shell options.");
    println!("Type `{shell_name} -c help' for more information about shell builtin commands.");
    println!("Use the `bashbug' command to report bugs.");
    println!();
    println!("bash home page: <http://www.gnu.org/software/bash>");
    println!("General help using GNU software: <http://www.gnu.org/gethelp/>");
}

/// GNU show_shell_version (version.c:88-104): the version banner, the FSF
/// copyright/license block, then rubash#488's build fingerprint appendix —
/// everything before the appendix stays byte-identical to the bash 5.3
/// contract the upstream .right assets gate on. `version` is the caller's
/// runtime `$BASH_VERSION`; `None` falls back to the advertised value.
pub fn show_shell_version(version: Option<&str>) {
    let version = version
        .map(str::to_string)
        .unwrap_or_else(crate::executor::bash_version_value);
    println!(
        "GNU bash, version {version} ({})",
        crate::executor::machtype_value()
    );
    println!("Copyright (C) 2025 Free Software Foundation, Inc.");
    println!("License GPLv3+: GNU GPL version 3 or later <http://gnu.org/licenses/gpl.html>");
    println!();
    println!("This is free software; you are free to change and redistribute it.");
    println!("There is NO WARRANTY, to the extent permitted by law.");
    println!("{}", crate::build_info::version_appendix());
}
