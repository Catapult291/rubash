<p align="center">
  <img src="assets/rubash-lockup.svg" alt="rubash — embeddable bash-compatible engine" width="620" />
</p>

# Rubash

An embeddable GNU Bash-compatible shell engine, written from scratch in Rust.

[中文](README.zh-CN.md) · [unixwin.dev](https://unixwin.dev)

Docs & downloads: [unixwin.dev](https://unixwin.dev)

[![CI](https://github.com/unixwin/rubash/actions/workflows/ci.yml/badge.svg)](https://github.com/unixwin/rubash/actions/workflows/ci.yml)
[![Rust Version](https://img.shields.io/badge/rust-1.70+-blue)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

## What Is Rubash

Rubash is a from-scratch reimplementation of GNU Bash semantics in Rust, packaged as an **embeddable, headless engine** — lexer, parser, expansion engine, executor, builtins, and all. It targets byte-level compatibility with GNU Bash 5.3.x (the vendored upstream tree is at the Bash-5.3 patch 20 level, #957) and runs natively on Windows, Linux, and macOS.

Rubash itself is not a shell product. It ships with a reference CLI used by the compatibility harness and tooling, while interactive shells are built *on top of* the engine: niubash embeds Rubash for all bash semantics and owns line editing, prompt rendering, and completions itself.

**Measured, not claimed**: compatibility is verified against GNU Bash's own 83-suite upstream test corpus — 83/83 suites pass on the 2026-10-12 full re-run. Raw stdout equality on Windows is 62/83; the other 20 differ only on environment-bound lines (host-tool output, NTFS filename/attr bits, locale, path form — each per-line classified in the ledger), and nameref's single residual line is proven to be GNU Bash's own coproc reap-timing race, not a rubash divergence (ledger below).

**Why native matters**: shells billed as "bash on Windows" (Git Bash, MSYS2) ship a ported bash that rides on a POSIX emulation layer (`msys-2.0.dll`), with fork emulation and path translation that leak quirks into every script. Rubash has no such layer — one self-contained binary speaking Win32 directly.

**Paths are first-class, one dialect per child**: the MSYS model *guesses* which arguments look like paths and rewrites them — which is why every AI agent and script has to set `MSYS_NO_PATHCONV=1` to stop `/flags` from becoming `C:/Program Files/Git/flags`. Rubash inverts the model with a per-child dialect contract (the same line GNU draws: `shell_execve` hands `execve` the raw word bytes and never rewrites argv). A child that owns a POSIX layer — anything resolved out of a WinuxCmd installation, or through the `winuxcmd` dispatcher — receives its argv **verbatim** and resolves `/d/...`, `/tmp`, `/dev/*` itself. A native Windows program instead receives real Win32 spellings for path-shaped operands, translated **uniformly** (never half of one argv translated and half left POSIX), so a native tool always gets a valid Win32 path for every operand, including not-yet-existing targets it is about to create. For emergency rollback the pre-Option-B translation behavior is available by exporting `__RUBASH_ARGV_DIALECT=legacy`.

**Platform status — cross-platform, Windows-first**: Windows is the primary
target and the platform with the full stack: one self-contained binary
solving the classic Windows bash pain points (no POSIX emulation layer, no
path-conversion heuristics, no `MSYS_NO_PATHCONV`, native Win32 paths as
the currency, MSYS2-compatible identity for the bash ecosystem). Linux
builds and runs natively (`x86_64-unknown-linux-gnu`) with real
getrlimit/chmod/faccessat/uname(2) semantics and signal delivery — CI runs
the engine's test lanes on Ubuntu (library tests, compatibility
`cli_tests`, marker goldens) and replays the GNU upstream corpus on every
push (the `Bash upstream test progress` job, `scripts/run-bash-upstream-tests.sh`);
the earlier manual pass of the same corpus on the Linux binary scored
17/24 suites byte-identical (2026-09-27 wt4/linuxrun lane, ledger
archived in #477/#581) and the remaining gap is tracked in the issue
tracker. macOS runs the library test suite in CI
(`macos-latest`) alongside the cross-target check. The engine's
semantic model (in-process subshells, fd-table semantics, process
boundaries) is deliberately platform-neutral, so the same ledger travels.

## Identity and Compatibility (rubash#154)

The engine presents a **selectable platform identity** instead of outsourcing
it to whatever `uname.exe` happens to sit on PATH. `uname` and `arch` are
engine builtins (full option parsing, coreutils/MSYS2 output shapes), and
`OSTYPE`/`MACHTYPE`/`HOSTTYPE` follow the same persona.

- **Default persona: MSYS2-compatible** (Windows default; disclosed
  everywhere — this is a compatibility mask, not a claim of being an MSYS2
  port. Non-Windows builds always report the honest-native identity — the
  MSYS persona exists to keep Windows inside the MSYS/Cygwin script
  ecosystem and is unreachable elsewhere):
  - `uname -s` → `MSYS_NT-<ver>` — or `MINGW64_NT-` / `UCRT64_NT-` /
    `CLANG64_NT-` … when `MSYSTEM` is set, mapping the value the same way
    the MSYS2 runtime does;
  - `uname -m` / `arch` → `x86_64` / `aarch64` (build arch);
  - `uname -r` → the Windows version string (`10.0-19044`, same string that
    finishes `uname -s`);
  - `uname -o` → `Msys`; `uname -a` → `sysname nodename release version
    machine Msys` (the Git Bash shape);
  - `OSTYPE=msys`, `MACHTYPE=<arch>-pc-msys` (bound `set_if_not`-style, as
    GNU variables.c:723-725 does — an inherited value wins).
  Ecosystem scripts that branch on `case "$(uname -s)" in MINGW*|MSYS*|
  CYGWIN*)` or `$OSTYPE` ∈ {msys, cygwin} take their best-tested path.
- **`RUBASH_IDENTITY=native` switches to the honest-native persona**:
  `uname -s` → `Windows_NT` (the native `%OS%` value), `uname -o` →
  `Windows`, `OSTYPE=windows`, `MACHTYPE=<arch>-pc-windows`. Tests and
  native-first users select this; unset the variable (or set any other
  value) to return to the default.

**Disclosure surfaces** (the persona must never be silent):

- `rubash --help` prints an Identity section (current persona + how to switch);
- `rubash --identity` prints the persona and every effective value
  (`uname -s/-m/-r/-o`, `arch`, `OSTYPE`, `MACHTYPE`);
- this README section and the repo-root `SKILL.md` disclosure section
  (so AI agents consuming the repo also know the persona semantics).

`uname`/`arch` are *hidden* fast-path builtins (like `sleep`/`dirname`):
`type`/`enable`/`compgen -b` keep reporting them as external commands, and
they work even when PATH carries no `uname.exe` at all.

## Compatibility at a Glance

```
GNU Bash 5.3.x test suite — 83 files, true-baseline measurement
(ledger: 2026-10-12 full re-run on master 764b9dde, host WINUXSH_ROOT
pin scrubbed — no upstream-script stubs, niu-mounted /bin/sh fixture,
per-suite TMPDIR, foreground timeout; environment-bound diffs counted
as zero after audit; earlier slice: 2026-09-25 on 71c933eb)

  PASS:            83/83 suites  ██████████████████████████████  100%
  ────────────────────────────────────────────────────────────────
  Raw:             62/83 suites byte-identical stdout on Windows —
                   literal 0-diff runs, scripts/true-baseline.sh
  Env-classified:  +20 suites differ only on environment-bound lines
                   (host-tool output, NTFS filename/attr bits, locale,
                   /dev/tty, path form, signal table, coproc reap
                   timing) — per-line attribution in the ledger below
  Clean:           82/83 suites with no rubash-caused diff after the
                   per-line environment audit
  nameref:         1 line (`declare -r RO_PID` visible after coproc) —
                   proven GNU 5.3.0's own coproc reap-timing race: GNU
                   vs itself flips 59/60↔1/60 on the harness filesystem,
                   60/60 deterministic on ext4 where its output is
                   byte-identical to rubash (stdout and stderr) —
                   docs/nameref-suite-residual-gnu-race-20261012.md
  Was 3427 on Sep 9 → −99%+ raw
```

The 2026-10-12 full re-run predates the v1.4.2 tag.

### Fully passing suites (zero diff, environment diffs excluded)

The clean set is derived from the ledger's per-suite classification, not
hand-maintained: a suite lands here when every differing line is
environment-attributed (per AGENTS.md, split rubash-caused vs
environment-caused with the covering issue). The raw count above (62)
plus the environment-classified set (20) plus nameref's proven-GNU race
account for all 83.

`alias` `appendop` `arith` `arith-for` `array` `assoc` `attr` `braces` `builtins` `case` `casemod` `complete` `comsub-eof` `comsub-posix` `comsub` `comsub2` `cond` `coproc` `cprint` `dbg-support` `dbg-support2` `dstack` `dstack2` `dynvar` `errors` `exp` `exportfunc` `extglob` `extglob2` `extglob3` `func` `getopts` `glob-bracket` `glob` `globstar` `heredoc` `herestr` `histexp` `history` `ifs-posix` `ifs` `intl` `invert` `invocation` `iquote` `jobs` `lastpipe` `mapfile` `more-exp` `new-exp` `nquote` `nquote1` `nquote2` `nquote3` `nquote4` `nquote5` `parser` `posix2` `posixexp` `posixexp2` `posixpat` `posixpipe` `precedence` `printf` `procsub` `quote` `quotearray` `read` `redir` `rhs-exp` `rsh` `set-e` `set-x` `shopt` `strip` `test` `tilde` `tilde2` `trap` `type` `varenv` `vredir`

(nameref — the 83rd suite — is accounted one block above: its single residual line is GNU Bash's own coproc reap race, proven byte-for-byte in `docs/nameref-suite-residual-gnu-race-20261012.md`.)

### Beyond the GNU suite — mixed evaluation suites and the real-world ecosystem

The same true-baseline method (script files, byte-level diff vs GNU 5.3.x)
applied to dedicated evaluation suites and real-world corpora
(full inventory + verdicts archived in issue #477):

| Surface | Result |
| --- | --- |
| **modernish** capability suite (cross-shell quirk probes) | **166/166 rc-vector identical** |
| **mvdan/sh parser corpus** | 559 snippets, parse-accept/exit parity after the operator-class fixes |
| **ble.sh** (29,601-line line-editor framework) | `bash -n` **rc=0 parity**; source mode terminates |
| **bash-completion** | 452/452 matrix; `bash -n` canary byte-identical |
| **git-completion** | 140/140 |
| **oh-my-bash** | 22 libs load, **252/252 functions** environment parity, PS1 byte-identical (5 themes) |
| **nvm.sh** (v0.40.x loader) | loads byte-clean incl. empty-`NVM_DIR` auto-use on Windows native (`$PWD` POSIX persona, #537); own test slice engine-caused failures fixed |
| **FFmpeg / OpenSSH / PHP / ltmain / cmake configure scripts** | `--help`/error-path output **byte-identical** |
| **git's own test framework** (t/test-lib.sh) | runs end-to-end under the engine |
| **GNU 83-suite on Linux native binary** | 17/24 byte-identical (2026-09-27 wt4/linuxrun lane, ledger archived in #477/#581); the corpus is now replayed in CI on every push (`Bash upstream test progress`, Ubuntu) |

Everything above is pinned as regression fixtures under `tests/` (335
integration test files, incl. `tests/regression.rs` — GNU-golden matrices,
suite-slice snapshots, and perf canaries) running in CI on every push —
plus a C-source line-level audit
(`docs/SOURCE-AUDIT.md`, 336 behaviors inventoried) that keeps finding and
closing gaps before suites hit them.

## Performance

Measured, not claimed — every number below is pinned to the PR or issue
that produced it, and the tests or gates that keep it honest are named.
The in-repo ledger is [`docs/performance.md`](docs/performance.md); perf
canaries ride the CI regression suite (`cargo test --test regression`).

**Interpreter hot paths — the Android-matrix triage, closed with numbers.**
The niubash#352 Android benchmark (aarch64, vs GNU bash 5.3.20) attributed
the interpreter-bound gap to three subsystems; all three are fixed and
closed:

- arithmetic-for / loop execution 2.37x → closed via #665/#671/#679
  (rubash#607);
- parameter expansion up to 7.3x per-op → `${s^}` alone ~6x faster after
  #666 (rubash#608);
- scalar assignment 2.72x → net ~1.06x GNU after #672/#678 (rubash#609).

**Array store: O(n²) → O(1) amortized** (#618/#616). Element writes
(`m[k]=$i` / `a[i]=$i`) append a storage segment instead of re-rendering,
with budget-triggered compaction — at 5000 distinct keys, associative
writes went 445x → 9.9x vs GNU and indexed writes 736x → 3.8x (ledger:
`docs/performance.md`).

**The literal fast-path wave** (each with its measured delta and a GNU
differential battery):

- multi-word literal command fast path: `[ 1 -eq 2 ]` ×20k ≈177→155 ms
  (#967);
- `[[ ]]` direct numeric leaf + decimal fast path: 50k `[[ 1 -eq 2 ]]`
  1.41x→1.10x vs GNU (#966);
- whitelist-skip of the procsub metadata scan: 1.56x→1.39x (~11%) (#974);
- decimal-subscript array write lane: interleaved 8k 3.65x→3.16x; also
  fixes a real double side-effect in `a[$((i++))]=v` (#959);
- case: head-only `$BASH_COMMAND` render: 20-arm ×10k −10%, plus the
  divergence fix (#958);
- literal-word admission shared across `expand_word_mut` and command words,
  targeting the 43% `expand_command_words` share of the export/unset body
  (#956); walker entry-chain trim (#973) and offset-window positional
  params for `shift` (#954) land in the same family.
- silent-builtin short-circuit with empty output and no redirects:
  shift ~1.5-4%, export/unset unchanged (#984);
- plain-`$name` subscript fast lane: assoc interleave 8k 3.5x→2.6x,
  linear (~3.1x asymptote) (#985);
- skip identity IFS-marking passes on quoteless words: export/unset 2k
  2.14x→1.97x (~3-4%) (#986);
- linearize subscript-span scan in compound element splitting: 8k literal
  fill ~850ms→~200ms, 20k ~5.1s→640ms (~8x, linear) (#987);
- fast-expand `[ $VAR -op N ]` integer comparisons: loop-while 30k
  ~2.4x→~1.8x vs bash (#990).

**export/unset economics** — [issue #916](https://github.com/unixwin/rubash/issues/916)
measured the per-op cost family (e.g. 2000 dynamic exports: 245 ms vs
GNU's 33 ms; the unset leg ~200x). The write-lane fixes are merged
(#917/#918/#920: single-probe mirror, gated env fallback, bulk unmark;
#943 reworks unbind for correctness); the issue stays open on the
residual.

**Windows process economics.** A console-less session used to pay a
hidden-console allocation on every external spawn, ~22 ms per child
(niubash#462). #913 replaces that with one hidden console per session —
pre-allocated windowless on Windows 11 24H2+/Server 2025, adopted from the
first `CREATE_NO_WINDOW` child elsewhere — pinned by the 14 regression
tests in `tests/console_window_gate.rs`. The tput whitelist arm (#884,
face A of [#800](https://github.com/unixwin/rubash/issues/800)) answers
admitted TERM×capability shapes in-process, byte-equal to GNU ncurses on a
269-row differential matrix: the ticket probe (64 tput calls) fell
1165 ms → 47 ms, zero spawns, pinned by `tests/issue800_tput_inline_arm_regressions.rs`.
The winuxcmd-side twin removed the ~17 ms per-call i18n overhead in the
real binary (~0.4 ms in-process after; 3208-row replay byte-identical,
[WinuxCmd#1269](https://github.com/unixwin/WinuxCmd/pull/1269)). Face B of
#800 — `read` on EOF, ~4.8 s/call — is still open.

## Architecture

```
src/
├── bin/             The `rubash` reference CLI
├── lexer/           Tokenizer (quoting, escaping, heredocs, continuations)
├── parser/          Recursive-descent (simple cmds, pipelines, case, arith-for, [[ ]])
├── expand/          Expansion engine (parameters, glob, braces, tilde, arith)
├── executor/        Command execution, builtins, dispatch, arrays, traps, identity
├── builtins/        62 engine builtins (declare, read, printf, kill, ...)
├── fd/              fd table and transports
├── jobs/            Job control, coprocs, process substitution
├── shell/           ShellState, subshell clone, init
└── lib.rs           Core types and error handling
```

- **Lexer**: Bash-style quoting, escaping, comments, variables, command substitution, arithmetic expansion, here-doc/here-string tokens, common redirects.
- **Parser**: Simple commands, pipelines, AND/OR lists, functions, brace/subshell groups, `if`, `for`, arithmetic `for`, `while`, `until`, `case`, `select`, `[[ ... ]]`, `coproc`, `time` prefixes.
- **Executor**: External commands, pipelines, redirects, temporary assignments, function calls, `source`/`.`, `eval`, shebangless script fallback, and the per-child argv dialect contract (verbatim argv for POSIX-layer children, uniform Win32 spellings for native ones).
- **Expansion**: Variables, positional parameters, indexed and associative arrays, command substitution, arithmetic expansion, brace expansion, tilde expansion, pathname globbing, `${parameter...}` operators, case/replacement transforms.
- **Builtins**: 62 engine builtins — the machine-checked list lives in [`docs/builtins.md`](docs/builtins.md) (kept in sync by a test over `src/executor/builtin_names.rs`), plus intentionally-hidden fast-path builtins (`sleep`, `dirname`, `basename`, `uname`, `arch`) that keep `type`/`compgen -b` reporting them as external, exactly like GNU.

### Subshells without fork

POSIX `fork()` has no Win32 equivalent. Emulation layers (MSYS2, Cygwin) fake it at the syscall level — expensive, fragile, and the source of their best-known quirks. Rubash reproduces fork's *semantics* instead, at three layers:

1. **In-process subshells.** `( list )` and `$( )` never spawn a process. `ShellState::clone` produces the child's variables, aliases, functions, traps, and history — the memory side of a fork — and the copy is discarded when the subshell ends.
2. **A real-handle fd table with POSIX `dup` semantics.** Slots hold raw Windows `HANDLE`s, and `DuplicateHandle` duplicates share the same kernel file object — therefore the same file offset. That is exactly POSIX "dup shares the open file description", verified empirically in a POC before landing. `fork_table()` duplicates the whole table handle-by-handle, the way `fork` copies the fd table but not the file objects.
3. **Real processes only at true process boundaries.** External commands and pipeline members run via `CreateProcess` + `os_pipe` (native `fork`/`exec` on Unix); background jobs and coprocs get their own processes, with job control on Job Objects and SIGCONT via `ResumeThread` on Windows.

The result: subshells and command substitutions pay zero process-creation cost, while everything a script can observe — exit codes, fd inheritance, shared offsets, signal dispositions — behaves like GNU Bash.

## Quick Start

### Build from Source

> Cross-platform: Windows is the full-stack primary target; Linux builds
> and runs natively with CI test lanes; macOS runs the library test suite
> in CI (see "Platform status" above).

```bash
git clone https://github.com/unixwin/rubash.git
cd rubash
cargo build
target/debug/rubash --version
```

### Run a Script

```bash
target/debug/rubash path/to/script.sh
target/debug/rubash -c 'echo hello from rubash'
```

### Run the Compatibility Suite

```sh
# GNU upstream corpus replay (the CI lane runner, Linux):
scripts/run-bash-upstream-tests.sh        # writes target/bash-upstream-tests/summary.md

# Full 83-suite true-baseline measurement (the harness behind the ledger;
# from Windows, per scripts/true-baseline.sh's own header):
MSYS_NO_PATHCONV=1 wsl bash /path/to/rubash/scripts/true-baseline.sh          # all 83 suites
MSYS_NO_PATHCONV=1 wsl bash /path/to/rubash/scripts/true-baseline.sh array    # single suite
```

## Testing

```bash
# Unit + integration tests (335 integration test files under tests/)
cargo test --lib
cargo test --test cli_tests

# Regression suite: GNU goldens, ecosystem snapshots, perf canaries
cargo test --test regression

# bashdb compatibility
cargo test --test cli_tests bashdb_compat -- --nocapture

# Source expansion
cargo test --test cli_tests source_expands -- --nocapture

# Console-less session gate (single hidden console, zero visible windows)
cargo test --test console_window_gate
```

The engine also runs the [bashdb](https://github.com/Trepan-Debuggers/bashdb) core debugger loop (list, step, next, where, continue, quit) end-to-end.

## Documentation

- GitHub issue tracker — **single source of truth** for Rubash ↔ GNU Bash compatibility status (snapshot ledgers archived 2026-10-10 in issue #477)
- [`docs/PROVENANCE.md`](docs/PROVENANCE.md) — provenance statement: what Rubash is relative to GNU Bash, and contributor methodology rules
- [`docs/builtins.md`](docs/builtins.md) — builtin inventory and dispatch model
- [`docs/bashdb-debugging-rubash.md`](docs/bashdb-debugging-rubash.md) — bashdb fixture setup and smoke test
- [`docs/bash-upstream-tests.md`](docs/bash-upstream-tests.md) — how to run GNU Bash upstream tests

## Development Principles

- Fix by root cause subsystem, not by individual expected-output lines.
- Keep bashdb external and clean; temporary instrumentation is for diagnosis only.
- Every failing bashdb command is an opportunity to find and fix a Rubash compatibility gap.
- Compatibility baseline is GNU Bash 5.3.x — the vendored upstream tree
  (`third_party/bash`) is at the Bash-5.3 patch 20 level (#957), with
  local and WSL GNU 5.3.20 baselines as differential oracles (#978).

## Provenance

Rubash is a **from-scratch rewrite of GNU Bash semantics in Rust** — not a port or translation of the GNU Bash C code. No GNU Bash source is compiled into, linked with, or copied into Rubash's own code. Compatibility is defined against the *observable behavior* of GNU Bash 5.3.x and verified by black-box differential testing. The vendored GNU Bash source lives in the separate `third_party/bash` submodule under its original GPL-3.0-or-later license and serves only as a semantic reference and test oracle. Full statement and contributor rules in [`docs/PROVENANCE.md`](docs/PROVENANCE.md).

## License

MIT — see [`LICENSE`](LICENSE).

## Contributing

Issues, compatibility reproductions, focused regression tests, and implementation patches welcome. Read [`AGENTS.md`](AGENTS.md) before compatibility work.

## Acknowledgements

- GNU Bash team — the reference implementation whose observable behavior defines our compatibility target
- Trepan-Debuggers/bashdb — external debugger and compatibility stress test
- Rust community — language and tooling

---

*Last updated: 2026-10-22*
