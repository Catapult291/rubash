# Nested-context inheritance: what each context inherits and owns

Companion to `fast-path-differential.md` (routes axis, #706), the variable-store axis
(#641) and `state-ownership-invariants.md` (ownership axis). This document is the
**nested-context axis** asked for by #713: for every execution context the engine can
enter — `( )` subshell, `$( )`/backticks, a pipeline stage, a shell function, a
foreign `sh -c` child, a background `&` job — which of the six audited properties it
**inherits** from the parent, which it **owns** privately, and where the contract is
**undefined**.

Version 2 (2026-10-21, #713 third pass). Method: the differential harness
`scripts/audit/nested-context-probes.py` — now **56 cases** (24 original + 21 second
pass + 11 this pass) run byte-for-byte (stdout, stderr, exit status separately)
against GNU bash reading the same case file. The canonical oracle is **WSL GNU bash
5.3.0(1)-release**; this pass's leg ran against native **MSYS2 GNU bash
5.2.37(1)-release** (`GNU_CMD="D:/Git/bin/bash.exe"`) because WSL was unavailable to
the runner — all measured cells are fork-emulation semantics MSYS2 implements
faithfully, and the `$0` path-spelling noise is normalized inside the harness since
this pass (D1). Every verdict below names its probe label. Fork-defined observables
a forkless engine cannot reproduce are recorded as **documented divergences**, not
bugs (§5).

## 1. The invariant table

Legend: **G** = matches GNU byte-for-byte (probe label) · **D*** = intended forkless
divergence · **BUG #n** = real defect, issue filed/open · **?** = unknown / not yet
measured.

| property | `( )` | `$( )` / backticks | pipeline stage | function | `sh -c` child | background `&` |
|---|---|---|---|---|---|---|
| **1. fd table** (open fds visible; `exec`-established; redirection scope) | G: `fd3-in-subshell`, `fd-exec-close-isolation`, `fd-offset-shared-paren` — the table is a value copy; `( exec 3<&- )` cannot close the parent's fd 3; Rc-shared endpoints share the file offset like `fork()` | G: `fd4-into-comsub` (#368 closed), `stderr-inside-comsub` | G: `fd3-into-pipeline-stage`, `pipe-stage-stderr` | G: `fd3-into-function` — the function is the *same* fd table, no copy | D*: foreign children receive only OS-inherited fds (`platform-fd-inheritance.md`; #378 fixed the numbered-fd pass-through) | G: `bg-…` probes — copy at spawn time |
| **2. traps** (EXIT / ERR / DEBUG / RETURN × `set -E` / `set -T`) | G: `err-plain-subshell`, `err-with-E` (#702), `err-nested-subshell-count`, `err-three-level-count`, `err-flat-region-fire`, `err-return-in-until` (#720), `exit-trap-in-subshell`, `trap-visible-in-subshell`, `ignored-signal-inherit`, `return-in-funcsub` — `reset_for_subshell_child` (`builtins/trap.rs:958`) keeps ERR under `-E` and DEBUG/RETURN under `-T` (trap.c:1466-1474), resets everything else, arms in-child installs | G: `err-in-comsub-under-E`, `debug-into-comsub-functrace-off`, `exit-trap-armed-in-comsub` — the ordinary comsub reset stays unconditional (GNU's comsub child does not fire ERR/DEBUG even under `-E`/`-T`; only an install *inside* the substitution fires) | G: `err-in-pipeline-stage` — the stage resets like a subshell; the pipeline aggregate (execute_cmd.c:2909) fires once for a failing *last* stage | G: `return-trap-functrace-on`, `debug-into-comsub-functrace-off` — ERR inherits only under `-E` (execute_cmd.c:5281-5291), RETURN under `-T`/`-o functrace` (execute_cmd.c:5294+) | D*: signal dispositions only; language traps are not a foreign-child concept | G: `exit-trap-background-subshell`, `bg-var-isolation` |
| **3. diagnostic ownership** (do messages route through the redirected fd table?) | **BUG #934** (§3.4): the `( )` command's *own* failing redirect list reports through the real stderr and never performs the earlier `2>` opens — probes `paren-redirect-err-route`, `paren-redirect-side-effect`; the plain faces match (`subshell-stderr-route`, `closed-stderr-subshell`, `subshell-redirect-diag-route` — the #826 face is fixed) | **BUG #934**: an open failure *inside* the substitution leaks to the real stderr instead of the in-force `2>` — `comsub-redirect-err-route`; pass-through itself is correct (`comsub-stderr-passthru`, `stderr-inside-comsub`) | G: `pipe-stage-stderr`; the stage's *own* open failure leaks like the simple path (#934) | **BUG #934**: `func-redirect-err-route`, `func-redirect-fail-status` — the function's own failing redirect list bypasses the table; body diagnostics route correctly (`func-stderr-route`). Simple commands share the defect (`simple-redirect-err-route`, `exec2-persist-err-route`, `simple-redirect-side-effect`) | D*: child's own stderr | G: routed at spawn |
| **4. compound-redirect exit status** | G: `compound-redirect-fail-status` (status 1); the message is now captured correctly on `{ }` (`brace-redirect-err-route`, #826 closed) but still misroutes on every other shape (#934) | G (status) | G (status) | G (status; `func-redirect-fail-status` shows the diagnostic face) | G | G |
| **5. stdin / EOF ownership** (`read` status, shared offsets) | G: `fd-offset-shared-paren`, `read-offset-paren-isolation` — an explicit `<&7`/`<file` binding shares the offset across the boundary; fd 0 consumption is scoped (#567 closed) | G: `read` cases in `conformance` | G: `stage-read-var-isolation` — a non-last stage cannot set parent variables (matches GNU without `lastpipe`) | G (same process) | D* | G: `#567` probes |
| **6. fd duplication ordering** (`2>&1 >file`) | G on the success path (#699/#725 fixed the order); **BUG #934 face B** on failure — the compound apply paths skip the earlier `2>` open's side effects entirely (`paren-redirect-side-effect`, `if`-family) | G on success | G on success | G on success; the simple/function paths perform the earlier open but misroute the report (`simple-redirect-side-effect`) | G | G |

Process identity is settled outside this table: `$BASHPID` increments per `( )` while
`$$` stays the parent (recorded in #713), and `bash -c`/foreign children carry the
platform identities (`platform-identity.md`).

## 2. The GNU mechanism each inherited cell reduces to

All anchors are the vendored bash 5.3 mirror (`third_party/bash`, submodule commit
`b4608166`).

- **Subshell entry** — `execute_cmd.c:1576` `execute_in_subshell`: the child gets a
  whole-state copy; `execute_cmd.c:1686` `reset_signal_handlers()` → `trap.c:1496`
  `reset_or_restore_signal_handlers(reset_signal)`; `trap.c:1466-1474` keeps DEBUG /
  RETURN only under `function_trace_mode` and ERROR only under `error_trace_mode`,
  leaving the trap *strings* in place (`restore_traps`, trap.c:1517, re-arms later
  installs). Rubash: `reset_for_subshell_child` (`builtins/trap.rs:958`) for true `( )`
  and flat `f() ( )` regions; the unconditional `reset_for_subshell` for command
  substitution / pipeline-element / `read` copy paths.
- **Fork collapse (the #720 form-1 rule)** — `execute_cmd.c:1799-1806`: a user
  subshell (or coproc) child whose whole body is a single cm_simple/cm_subshell marks
  it `CMD_NO_FORK`; `execute_cmd.c:662-663` then runs a so-marked subshell body in
  place — no second `make_child`, no `wait_for`. Observable: in `( (false) )` the
  middle frame never adjudicates anything, so `( (false) )` fires ERR twice, not three
  times, under `-E`. Eligibility is `should_optimize_fork` /
  `optimize_subshell_command` (`builtins/evalstring.c:106-122, 162-179`); function
  bodies are *never* collapsed as subshells (`optimize_shell_function`,
  evalstring.c:180-198, marks only cm_simple/cm_connection).
- **Parent-side ERR adjudication (the #720 form-2 rule)** — the frame that forked a
  user subshell waits and runs `run_error_trap` keyed on the subshell command
  (`execute_cmd.c:756-778`): `user_subshell && was_error_trap && ignore_return == 0 &&
  invert == 0 && status != 0` — **no &&/|| exemption** at this site (unlike the simple
  command's execute_cmd.c:1000). `$LINENO` inside that fire is the subshell's own
  close-paren line (`execute_cmd.c:696` `SET_LINE_NUMBER(command->value.Subshell->line)`)
  and `BASH_COMMAND` is the subshell's `print_cmd` text fixed by the fork preamble
  (`execute_cmd.c:703-710` writes `the_printed_command_except_trap`). The errexit jump
  comes *after* the fire (`execute_cmd.c:777-795` runs no ERR fire of its own).
- **Other ERR sites** — simple command execute_cmd.c:1000 (with
  `pipe_in/pipe_out == NO_PIPE` exemption, so a failing non-last pipeline stage is
  silent), compound wrappers execute_cmd.c:1166 (cm_arith/cm_cond/cm_function_def
  only), pipeline aggregate execute_cmd.c:2909, redirection failure execute_cmd.c:874.
  `run_error_trap` itself gates on SIG_TRAPPED/SIG_IGNORED/SIG_INPROGRESS
  (trap.c:1321-1326).
- **Function inheritance** — execute_cmd.c:5270-5300: DEBUG under `-T`, ERR under
  `-E`, RETURN under `-T` or per-function trace; each restored via unwind-protect on
  return. An in-body install survives (`trap_if_untrapped`, trap.c:737-743).
- **Command substitution** — `reset_signal_handlers()` like any child
  (subst.c path); GNU 5.3 in script context does **not** re-arm ERR/DEBUG for the
  comsub child even under `-E`/`-T` (probes `err-in-comsub-under-E`,
  `debug-into-comsub-functrace-off`), while an install *inside* the substitution fires
  (#345) and the EXIT trap armed inside runs at its end (`exit-trap-armed-in-comsub`).

## 3. Findings — real defects

### 3.1 FIXED — `set -T` DEBUG trap is not inherited into `$( )`
Probe `debug-into-comsub-functrace-on`:

    set -T; trap 'echo D:$BASH_COMMAND' DEBUG; x=$(true); echo got=$x
    GNU  : got=D:true        (the comsub body's DEBUG fires, under -T)
    rubash: got=             (the comsub resets DEBUG unconditionally)

GNU keeps DEBUG in the comsub child when `function_trace_mode` is set
(trap.c:1469-1471 clears DEBUG/RETURN only `if (function_trace_mode == 0)`); rubash's
command-substitution path calls the unconditional `reset_for_subshell` and has no
functrace-scoped variant. Filed as **#825**, since closed.

### 3.2 FIXED — a compound's own redirection-failure diagnostic bypasses the ordered fd table
Three probes, one mechanism (`brace-redirect-err-route`,
`compound-redirect-fail-status`, `subshell-redirect-diag-route`):

    { :; } 2>err2.txt > /nonexistent_dir/f
    GNU  : message captured in err2.txt, nothing on real stderr
    rubash: message printed twice on real stderr, err2.txt never created

    ( { :; } > /nonexistent_dir/f ) 2>err.txt
    GNU  : message captured in err.txt
    rubash: message captured *and* leaked to the real stderr

The status (1) is right in every shape; the *message* is (a) emitted twice — once per
adjudication layer — and (b) written through the process's real stderr instead of the
command's redirected fd 2, and the `2>err2.txt` binding is never opened at all when a
later redirect fails. This is the "diagnostic destination" mirror of
`state-ownership-invariants.md` row B (175 `eprintln!` outside entry points) — the
compound wrapper's open-failure reporter does not consult the fd table the command
established. Faces (b)-in-subshell and the `cd` variant are already tracked in #699;
the doubled emission and the dropped `2>file` capture are new. Filed as **#826**,
since closed — but only the `{ }` wrapper path got the fix; every other apply path
still carried the defect (see §3.4).

### 3.3 Tickets previously mapped to cells — all closed as of this pass
- #699 — `2>&1 >file` order residuals, builtin write-error routing, closed-stderr
  status corruption (cells 3/6). Closed.
- #700 / #701 — `read` on `/dev/stdin`, `/dev/fd/N`, EOF-without-newline under
  compound redirection (cell 5, Windows-origin faces). Closed.
- #720 — ERR-trap nested-context faces incl. `cat <(false)` procsub (cells 1×2).
  Closed.
- The live cell-defect after this pass is **#934** (§3.4), the only open item.

### 3.4 NEW — redirect open-failure diagnostic bypasses the fd table on all
non-brace paths (#934, two faces)

#826 fixed the invariant on the `{ }` wrapper only
(`apply_brace_group_redirects`, `src/executor/source_type_state.rs`). GNU applies
the whole redirect list left-to-right in `do_redirection_internal`
(`redir.c:767-955`) and reports an open failure through the fd-2 binding *in force
at that point* (`redirection_error`, `redir.c:135`). Every other apply path
violates one or both halves:

- **Face A — wrong destination.** The open-failure message goes to the process's
  real stderr even when an earlier `2>file` (same command or persistent
  `exec 2>file`) is bound. Status 1 and single emission are correct everywhere.
  Faces: `simple-redirect-err-route`, `func-redirect-err-route`,
  `paren-redirect-err-route`, `if-redirect-err-route`,
  `comsub-redirect-err-route`, `exec2-persist-err-route`,
  `func-redirect-fail-status` (the `2>/dev/null` suppression face).
- **Face B — missing side effects.** On the `( )`/`if`-family compound paths the
  earlier `2>cap.txt` open never even happens when a later `>` fails — a stale
  cap.txt survives untouched (GNU truncates it). Faces:
  `paren-redirect-side-effect` (and the `if`/while/for/case/until siblings share
  `apply_redirects_to_commands`, `compound_exec.rs:3610`). The simple/function
  paths do perform the open (`simple-redirect-side-effect`) — only their
  reporter is wrong.

Correct faces already held: `{ }` brace (all three #826 probes), `>`-before-`2>`
ordering (both shells agree the message reaches real stderr), and dup-failure
diagnostics (`N: Bad file descriptor` does route through the table). The defect
is specific to the *open*-failure reporter and the parse-order application
contract — one mechanism, like #826, generalized to the remaining apply paths
(`apply_ordered_output_redirects` `redirection.rs:1108`,
`apply_function_call_redirects` `function_calls.rs:835`,
`apply_no_output_builtin_redirects` `builtin_redirects.rs:52`, pipeline-stage
wiring).

## 4. Documented forkless divergences (by design, do not "fix")
- **D1** diagnostic branding: `$0: line N:` prefix carries the engine's own argv[0]
  (`rubash.exe` vs `bash`), and the WSL/native path spelling follows the host —
  byte-equality on *text* is only defined modulo the brand.
- **D2** foreign `sh -c` children: language traps (EXIT/ERR/DEBUG/RETURN) do not
  exist across a process boundary; only exported variables, cwd and OS signal
  dispositions cross. Numbered-fd pass-through is the #378 contract
  (`platform-fd-inheritance.md`).
- **D3** unexported variables do not cross into `sh -c` (matches GNU exactly, listed
  here because it is a *fork* semantic, not a parse one).

## 5. Unknown / not yet measured
- Unix and macOS legs (`bash-upstream-tests.md` runs them only in CI snapshots):
  `/dev/stdin`, `/dev/fd/N` and text-mode faces of #700/#701 are Windows-shaped and
  may not exist there, while D2 may be *harder* on Unix (real exec).
- Job-control cells: `Ctrl-Z` / terminal pgrp ownership of a foreground pipeline
  stage (no portable probe in this harness yet).
- `coproc` (a context of its own; the table omits it — current coproc support is
  minimal).
- `wait`/`$!` interplay with traps firing mid-`wait` (partially covered by trap8
  upstream slice; not a first-class cell here).

## 6. Harness

`scripts/audit/nested-context-probes.py` (56 cases, exit non-zero on any divergence)
— run:

    RUBASH_BIN=<path>/rubash.exe python scripts/audit/nested-context-probes.py

Since this pass the harness normalizes the `$0` case-file path spelling
(`D:\...`, `/d/...`, `/mnt/d/...`, `/cygdrive/d/...` → `<CASE>`) on both streams
before comparing — the D1 brand noise no longer masquerades as a divergence, so
`divergences=N` is once again a clean regression signal. Line numbers, message
text, routing and status are still byte-compared.

Last full run (master `d4fc195c`, 2026-10-21): `cases=56 divergences=9` — all
nine are §3.4 / #934 faces (`simple`/`func`/`paren`/`if`/`comsub`/`exec2`
redirect-err-route, `func-redirect-fail-status`, and the two `*-side-effect`
probes); zero regressions elsewhere — every #720/#825/#826 trap and brace cell
still passes. Oracle leg this run: native MSYS2 GNU bash 5.2.37(1)
(`GNU_CMD="D:/Git/bin/bash.exe"`); the canonical WSL 5.3.0 leg should be
re-confirmed at merge time. Runner:
`RUBASH_BIN=<exe> GNU_CMD='wsl /usr/local/bin/bash' python
scripts/audit/nested-context-probes.py` from an empty scratch cwd (case
artefacts land there); stdin is EOF by harness construction. On GBK consoles
run with `PYTHONIOENCODING=utf-8`. On hosts whose `wsl.exe` prepends banner
noise to guest stderr, strip that exact prefix outside the comparison — it is
machine noise, not a leg.
