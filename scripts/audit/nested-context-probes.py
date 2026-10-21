#!/usr/bin/env python3
"""#713 nested-context inheritance probes: subshell `( )`, `$( )`, pipeline
stages and background jobs against a genuine GNU bash.

Each case runs in its own file (`bash <file>` / `rubash <file>`) with stdout,
stderr and exit status compared separately. Do NOT run the GNU leg as
`wsl bash -lc "bash -c '...'"`: WSL re-joins the argument list, the single
quotes vanish, and `;` is then interpreted by the outer shell (it silently
fabricates `$?`/`$x` divergences).

The case-file path appears inside diagnostics as `$0` ("<case.sh>: line N:"),
and each leg spells it differently (`D:/...` native, `/d/...` rubash-normalized,
`/mnt/d/...` WSL) — machine noise, not a semantic leg (context-inheritance.md
D1). Both streams are normalized through `_norm` before comparison so the
verdict is semantic; line numbers, message text, routing and status are still
compared byte-for-byte.

Override the binaries with RUBASH_BIN / GNU_CMD:
    RUBASH_BIN=<path>/rubash.exe python scripts/audit/nested-context-probes.py
"""
import subprocess, os, pathlib

RB = os.environ.get(
    "RUBASH_BIN", r"D:/repo/rubash-wt-392-lookup/target/debug/rubash.exe"
)
# The GNU leg must read the case from a FILE: `wsl bash -lc "bash -c ..."`
# re-joins the argument list and strips quotes (see the harness notes above).
GNU_PREFIX = os.environ.get("GNU_CMD", "wsl bash").split()
here = pathlib.Path(__file__).resolve().parent
cases = [
 ("fd9-in-subshell",        "exec 9<&0; ( IFS= read -r -u9 x </dev/null; echo rc=$? )"),
 ("fd3-in-subshell",        "exec 3>&1; ( echo via3 >&3 )"),
 ("stdout-redirect-inherit","( echo hidden >/dev/null ); echo visible"),
 ("trap-visible-in-subshell","trap 'echo T' USR1; ( trap -p USR1 )"),
 ("exit-trap-in-subshell",  "trap 'echo EXIT' EXIT; ( : ); echo done"),
 ("err-plain-subshell",     "trap 'echo E' ERR; ( false ); echo after"),
 ("err-with-E",             "set -E; trap 'echo E' ERR; ( false ); echo after"),
 ("return-in-funcsub",      "trap 'echo R' RETURN; f() ( : ); f; echo after"),
 ("ignored-signal-inherit", "( trap '' INT; trap -p INT )"),
 ("subshell-exit-code",     "( exit 7 ); echo $?"),
 ("comsub-exit-code",       "x=$(exit 7); echo $?"),
 ("subshell-outer-status",  "( exit 5 ); :; echo $?"),
 ("pipe-status-no-pipefail","false | true; echo $?"),
 ("pipe-status-pipefail",   "set -o pipefail; false | true; echo $?"),
 ("bg-wait-status",         "( exit 9 ) & wait $!; echo $?"),
 ("bg-group-wait-status",   "{ sleep 0; exit 3; } & wait; echo $?"),
 # --- 2026-10 round (#713 second pass, after #720): trap inheritance and
 # --- fork-collapse faces. Each label maps to a cell of
 # --- docs/audit/context-inheritance.md.
 ("err-nested-subshell-count",   "set -E; trap 'echo E sub=$BASH_SUBSHELL' ERR; ( (false) ); echo rc=$?"),
 ("err-three-level-count",       "set -E; trap 'echo E sub=$BASH_SUBSHELL' ERR; ( ( (false) ) ); echo rc=$?"),
 ("err-flat-region-fire",        "set -E; trap 'echo E' ERR; f() ( false ); f; echo rc=$?"),
 ("err-return-in-until",         "set -e; trap 'echo E:$?' ERR; f(){ return 7; }; until false; do f; done"),
 ("err-in-pipeline-stage",       "set -E; trap 'echo E' ERR; true | false; echo rc=$?"),
 ("err-in-comsub-under-E",       "set -E; trap 'echo E' ERR; x=$(false); echo rc=$?"),
 ("debug-into-comsub-functrace-off", "trap 'echo D:$BASH_COMMAND' DEBUG; x=$(true); echo got=$x"),
 ("debug-into-comsub-functrace-on",  "set -T; trap 'echo D:$BASH_COMMAND' DEBUG; x=$(true); echo got=$x"),
 ("return-trap-functrace-on",    "set -T; trap 'echo R' RETURN; f(){ :; }; f"),
 ("exit-trap-armed-in-comsub",   "z=$( trap 'echo IX' EXIT; echo body ); echo got=$z"),
 ("exit-trap-background-subshell","trap 'echo BX' EXIT; ( echo body ) & wait; trap - EXIT; echo done"),
 ("fd-exec-close-isolation",     "exec 3>c1.log; echo pre >&3; ( exec 3<&- ); echo post >&3 && echo alive; exec 3>&-"),
 ("fd3-into-pipeline-stage",     "exec 3>c2.log; echo s >&3; echo staged; exec 3>&-; echo f3=$(cat c2.log 2>/dev/null | tr '\\n' '/')"),
 ("fd4-into-comsub",             "exec 4>c3.log; echo v4 >&4; y=$( echo c4 >&4; echo ok ); echo got=$y fd4=$(cat c3.log 2>/dev/null | tr '\\n' '/'); exec 4>&-"),
 ("fd-offset-shared-paren",      "printf '1\\n2\\n3\\n' > c4.txt; exec 7<c4.txt; read -r a <&7; ( read -r b <&7; echo in=$b ); read -r c <&7; echo after=$c; exec 7<&-"),
 ("read-offset-paren-isolation", "printf 'a\\nb\\n' > c5.txt; { read -r l1; } < c5.txt; { read -r l2; } < c5.txt; echo \"$l1/$l2\""),
 ("compound-redirect-fail-status", "{ :; } > /nonexistent_dir_r720/f; echo rc=$?"),
 ("subshell-redirect-diag-route", "( { :; } > /nonexistent_dir_r720/f ) 2>err.txt; echo rc=$? err=[$(cat err.txt)]"),
 ("stage-read-var-isolation",    "echo hello | read -r got; echo stage=$got"),
 ("comsub-pipe-status",          "s=$( true | false ); echo got=$s"),
 ("brace-redirect-err-route",    "{ echo x; } 2>err2.txt > /nonexistent_dir_r720/f; echo rc=$? err=[$(cat err2.txt)]"),
 ("subshell-stderr-route",  "( echo E >&2 ) 2>/dev/null; echo rc=$?"),
 ("comsub-stderr-passthru", "x=$(echo E >&2); echo \"got=[$x]\""),
 ("closed-stderr-subshell", "( echo E >&2 ) 2>&-; echo rc=$?"),
 ("pipe-stage-stderr",      "{ echo E >&2; } | cat; echo rc=$?"),
 ("stderr-inside-comsub",   "x=$( { echo E >&2; } 2>&1 ); echo \"got=[$x]\""),
 ("subshell-var-isolation", "x=1; ( x=2 ); echo $x"),
 ("comsub-var-isolation",   "x=1; x=$(echo 2); echo $x"),
 ("bg-var-isolation",       "x=1; x=2 & wait; echo $x"),
 # --- 2026-10-21 third pass (#713): the doc-cited function fd-table cell
 # --- gets real probes, plus the #934 face set — every non-brace redirect-
 # --- apply path reports an OPEN failure through the real stderr instead of
 # --- the fd-2 binding in force (the brace group is correct since #826;
 # --- `>`-before-`2>` and dup-failure faces already agree).
 ("fd3-into-function",       "exec 3>f0.log; f(){ echo g >&3; }; f; exec 3>&-; echo fd3=$(cat f0.log)"),
 ("func-stderr-route",       "f(){ echo E >&2; }; f 2>frr.txt; echo rc=$? ferr=[$(cat frr.txt)]"),
 ("simple-redirect-err-route","true 2>cap.txt > /nonexistent_dir_r720/f; echo rc=$? cap=[$(cat cap.txt 2>/dev/null)]"),
 ("func-redirect-err-route", "f(){ :; }; f 2>cap.txt > /nonexistent_dir_r720/f; echo rc=$? cap=[$(cat cap.txt 2>/dev/null)]"),
 ("paren-redirect-err-route","( :; ) 2>cap.txt > /nonexistent_dir_r720/f; echo rc=$? cap=[$(cat cap.txt 2>/dev/null)]"),
 ("if-redirect-err-route",   "if true; then :; fi 2>cap.txt > /nonexistent_dir_r720/f; echo rc=$? cap=[$(cat cap.txt 2>/dev/null)]"),
 ("comsub-redirect-err-route","f(){ :; }; x=$( f 2>cap.txt > /nonexistent_dir_r720/f ); echo rc=$? cap=[$(cat cap.txt 2>/dev/null)]"),
 ("exec2-persist-err-route", "exec 2>cap.txt; true > /nonexistent_dir_r720/f; rc=$?; exec 2>&1; echo rc=$rc cap=[$(cat cap.txt)]"),
 ("func-redirect-fail-status","f(){ :; }; f 2>/dev/null > /nonexistent_dir_r720/f; echo rc=$?"),
 # #934 face B: earlier redirects in a failing list must still perform their
 # side effects in parse order (GNU do_redirection_internal, redir.c:767-955)
 # — the simple path truncates cap.txt but reports to the real stderr; the
 # `( )`/compound paths leave even a STALE cap.txt untouched.
 ("simple-redirect-side-effect","printf 'STALE\\n' > cap.txt; true 2>cap.txt > /nonexistent_dir_r720/f; echo rc=$? cap=[$(cat cap.txt)]"),
 ("paren-redirect-side-effect","printf 'STALE\\n' > cap.txt; ( :; ) 2>cap.txt > /nonexistent_dir_r720/f; echo rc=$? cap=[$(cat cap.txt)]"),
]
def _run(argv):
    # stdin is EOF, never inherited: a case that dups fd 0 (fd9-in-subshell)
    # must see EOF deterministically instead of blocking on whatever pipe
    # the runner was launched with (rubash#713 rerun 2026-10-20 hung here
    # under a held-open inherited stdin). Both legs share the same DEVNULL,
    # so the differential comparison is unaffected.
    p = subprocess.run(argv, capture_output=True, stdin=subprocess.DEVNULL)
    return (
        p.stdout.decode(errors="replace"),
        p.stderr.decode(errors="replace"),
        p.returncode,
    )


def _case_file(script):
    f = here / "case.sh"
    f.write_text(script.replace("\r\n", "\n"), encoding="utf-8")
    return f


def _norm(text):
    """Fold the case-file path's per-leg spellings to one token.

    Diagnostics embed `$0`; the rubash leg renders the same file `/d/...`,
    a native GNU leg `D:\\...` or `D:/...`, and a WSL leg `/mnt/d/...`.
    Everything else — line numbers, message text, routing, status — stays
    byte-compared.
    """
    f = here / "case.sh"
    posix = str(f).replace("\\", "/")
    drive, tail = posix[0].lower(), posix[2:]
    spellings = {
        str(f),
        posix,
        f"/{drive}{tail}",
        f"/mnt/{drive}{tail}",
        f"/cygdrive/{drive}{tail}",
    }
    for spelled in sorted(spellings, key=len, reverse=True):
        text = text.replace(spelled, "<CASE>")
    return text


def _as_guest_path(path):
    """Translate for a WSL guest; a native bash just gets the path back.

    Normalize to forward slashes first: WSL's command-line handling eats
    backslashes, so `wslpath` would receive `D:repodirfile` with no
    separators and echo it back unchanged.
    """
    win = str(path).replace("\\", "/")
    out = subprocess.run(["wsl", "wslpath", "-a", win], capture_output=True)
    return out.stdout.decode().strip() or win


def run_rb(script):
    # File form for both legs: `-c` binds a different stdin transport.
    return _run([RB, str(_case_file(script))])


def run_gnu(script):
    f = _case_file(script)
    if GNU_PREFIX[:1] == ["wsl"]:
        return _run(GNU_PREFIX + [_as_guest_path(f)])
    return _run(GNU_PREFIX + [str(f)])


if __name__ == "__main__":
    bad = 0
    for label, script in cases:
        rb, gn = run_rb(script), run_gnu(script)
        rb = (_norm(rb[0]), _norm(rb[1]), rb[2])
        gn = (_norm(gn[0]), _norm(gn[1]), gn[2])
        if rb != gn:
            bad += 1
            print(
                f"### DIVERGE {label}\n  script   : {script}\n"
                f"  rubash   : out={rb[0]!r} err={rb[1]!r} rc={rb[2]}\n"
                f"  GNU bash : out={gn[0]!r} err={gn[1]!r} rc={gn[2]}"
            )
    print(f"\ncases={len(cases)} divergences={bad}")
    raise SystemExit(1 if bad else 0)
