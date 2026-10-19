# Nested-context inheritance: what each context inherits and owns

Companion to `fast-path-differential.md` (routes axis, #706), the variable-store axis
(#641) and `state-ownership-invariants.md` (ownership axis). This document is the
**nested-context axis** asked for by #713: for every execution context the engine can
enter — `( )` subshell, `$( )`/backticks, a pipeline stage, a shell function, a
foreign `sh -c` child, a background `&` job — which of the six audited properties it
**inherits** from the parent, which it **owns** privately, and where the contract is
**undefined**.

Version 1 (2026-10-13, #713 second pass). Method: the differential harness
`scripts/audit/nested-context-probes.py` — now **45 cases** (24 original + 21 new)
run byte-for-byte (stdout, stderr, exit status separately) against **WSL GNU bash
5.3.0(1)-release** reading the same case file. Every verdict below names its probe
label. Fork-defined observables a forkless engine cannot reproduce are recorded as
**documented divergences**, not bugs (§5).

## 1. The invariant table

Legend: **G** = matches GNU byte-for-byte (probe label) · **D*** = intended forkless
divergence · **BUG #n** = real defect, issue filed/open · **?** = unknown / not yet
measured.

| property | `( )` | `$( )` / backticks | pipeline stage | function | `sh -c` child | background `&` |
|---|---|---|---|---|---|---|
| **1. fd table** (open fds visible; `exec`-established; redirection scope) | G: `fd3-in-subshell`, `fd-exec-close-isolation`, `fd-offset-shared-paren` — the table is a value copy; `( exec 3<&- )` cannot close the parent's fd 3; Rc-shared endpoints share the file offset like `fork()` | G: `fd4-into-comsub` (#368 closed), `stderr-inside-comsub` | G: `fd3-into-pipeline-stage`, `pipe-stage-stderr` | G: `fd3-into-function` — the function is the *same* fd table, no copy | D*: foreign children receive only OS-inherited fds (`platform-fd-inheritance.md`; #378 fixed the numbered-fd pass-through) | G: `bg-…` probes — copy at spawn time |
| **2. traps** (EXIT / ERR / DEBUG / RETURN × `set -E` / `set -T`) | G: `err-plain-subshell`, `err-with-E` (#702), `err-nested-subshell-count`, `err-three-level-count`, `err-flat-region-fire`, `err-return-in-until` (#720), `exit-trap-in-subshell`, `trap-visible-in-subshell`, `ignored-signal-inherit`, `return-in-funcsub` — `reset_for_subshell_child` (`builtins/trap.rs:958`) keeps ERR under `-E` and DEBUG/RETURN under `-T` (trap.c:1466-1474), resets everything else, arms in-child installs | G: `err-in-comsub-under-E`, `debug-into-comsub-functrace-off`, `exit-trap-armed-in-comsub` — the ordinary comsub reset stays unconditional (GNU's comsub child does not fire ERR/DEBUG even under `-E`/`-T`; only an install *inside* the substitution fires) | G: `err-in-pipeline-stage` — the stage resets like a subshell; the pipeline aggregate (execute_cmd.c:2909) fires once for a failing *last* stage | G: `return-trap-functrace-on`, `debug-into-comsub-functrace-off` — ERR inherits only under `-E` (execute_cmd.c:5281-5291), RETURN under `-T`/`-o functrace` (execute_cmd.c:5294+) | D*: signal dispositions only; language traps are not a foreign-child concept | G: `exit-trap-background-subshell`, `bg-var-isolation` |
| **3. diagnostic ownership** (do messages route through the redirected fd table?) | **BUG** (see §3.2): a compound's *own* redirection-failure message inside `( … ) 2>file` still reaches the real stderr — probe `subshell-redirect-diag-route`; the plain-command faces match (`subshell-stderr-route`, `closed-stderr-subshell`, #271/#510 closed) | G: `comsub-stderr-passthru` | G: `pipe-stage-stderr` | G: routed — same table | D*: child's own stderr | G: routed at spawn |
| **4. compound-redirect exit status** | G: `compound-redirect-fail-status` (status 1) — but the message **doubles** (§3.1) and the `2>file` capture drops (`brace-redirect-err-route`) | G (status) | G (status) | G | G | G |
| **5. stdin / EOF ownership** (`read` status, shared offsets) | G: `read-offset-shared-paren`, `read-offset-paren-isolation`, `fd-offset-shared-paren` — an explicit `<&7`/`<file` binding shares the offset across the boundary; fd 0 consumption is scoped (#567 closed) | G: `read` cases in `conformance` | G: `stage-read-var-isolation` — a non-last stage cannot set parent variables (matches GNU without `lastpipe`) | G (same process) | D* | G: `#567` probes |
| **6. fd duplication ordering** (`2>&1 >file`) | G: `dup2` shape probes — the ordered list is a sequence, left to right (#699/#725 fixed the order) | G | G | G | G | G |

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

## 3. Findings — real defects (filed, not fixed in the #720 PR)

### 3.1 NEW — `set -T` DEBUG trap is not inherited into `$( )`
Probe `debug-into-comsub-functrace-on`:

    set -T; trap 'echo D:$BASH_COMMAND' DEBUG; x=$(true); echo got=$x
    GNU  : got=D:true        (the comsub body's DEBUG fires, under -T)
    rubash: got=             (the comsub resets DEBUG unconditionally)

GNU keeps DEBUG in the comsub child when `function_trace_mode` is set
(trap.c:1469-1471 clears DEBUG/RETURN only `if (function_trace_mode == 0)`); rubash's
command-substitution path calls the unconditional `reset_for_subshell` and has no
functrace-scoped variant. Filed as **#825**.

### 3.2 NEW — a compound's own redirection-failure diagnostic bypasses the ordered fd table
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
the doubled emission and the dropped `2>file` capture are new. Filed as **#826**.

### 3.3 Known-open tickets mapped to cells (no new filing)
- #699 — `2>&1 >file` order residuals, builtin write-error routing, closed-stderr
  status corruption (cells 3/6).
- #700 / #701 — `read` on `/dev/stdin`, `/dev/fd/N`, EOF-without-newline under
  compound redirection (cell 5, Windows-origin faces).
- #720 form 3 — `cat <(false)` process-substitution copy path (cell 1×2 seam, PR
  #715's read/procsub area).

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

`scripts/audit/nested-context-probes.py` (45 cases, exit non-zero on any divergence)
— run:

    RUBASH_BIN=<path>/rubash.exe python scripts/audit/nested-context-probes.py

Last full run (this branch, 2026-10-13): `cases=45 divergences=4` — the four are
§3.1 and §3.2 (three probes of one mechanism). Every #720-related trap cell passes.
