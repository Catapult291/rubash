#!/usr/bin/env bash
# check-profile-differential.sh — rubash#815 signature acceptance:
# the FULL 83-suite GNU differential re-run with the profiler ON, scripted.
#
# Methodology: scripts/true-baseline.sh is reused VERBATIM (frozen
# methodology; do not hand-roll probes). This wrapper only points
# RUB_OVERRIDE at a launcher that execs the binary under test with
# --profile --profile-output <scratch>/last.collapsed ahead of the suite
# operand. The profiler writes NOTHING to stdout/stderr (reports go to the
# --profile-output path, outside the suite cwd), so the byte-identity claim
# "profiler on == profiler off == GNU baseline" is decidable from the
# per-suite diff counts alone:
#
#   baseline ledger : true-baseline.sh (profiler off)   -> true-baseline-ledger.log
#   profiler-on     : this script                       -> profile-differential-ledger.log
#
# A suite whose diff count is identical in both ledgers proves the profiler
# did not perturb that suite. Any movement is a rubash regression to fix
# before merge (the failure SET must also match bidirectionally).
#
# Usage (from Windows / Git Bash):
#   bash scripts/check-profile-differential.sh [suite...]
# With no arguments it runs all 83 suites.

set -u

unset BASH_ENV 2>/dev/null || true
unset WINUXSH_ROOT 2>/dev/null || true

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${RUB:-$ROOT/target/debug/rubash.exe}"
OUT="$ROOT/target/issue-suites/results/profile-differential"
LOG="$OUT/profile-differential-ledger.log"

# WSL-side paths for the launcher and the harness. Both Git Bash's MSYS
# form (/d/repo/...) and the native form (D:/repo/...) normalize to
# /mnt/d/repo/... (docs/windows-path-contract.md: never hand a Windows
# process a path the other side cannot resolve).
to_wsl() {
  local p="${1//\\//}"
  case "$p" in
    [A-Za-z]:/*)
      p="/mnt/$(printf '%s' "${p%%:*}" | tr '[:upper:]' '[:lower:]')${p#*:}"
      ;;
    /*)
      p="/mnt/$(printf '%s' "${p:1:1}" | tr '[:upper:]' '[:lower:]')${p:2}"
      ;;
  esac
  printf '%s' "$p"
}

# The inverse: an MSYS /d/... path to the native D:/... form a Windows
# process can open.
to_native() {
  local p="$1"
  case "$p" in
    /[A-Za-z]/*)
      drive="$(printf '%s' "${p:1:1}" | tr '[:lower:]' '[:upper:]')"
      p="${drive}:${p:2}"
      ;;
  esac
  printf '%s' "$p"
}
ROOT_WS="$(to_wsl "$ROOT")"
BIN_WS="$(to_wsl "$BIN")"
OUT_WS="$(to_wsl "$OUT")"
HARNESS_WS="$ROOT_WS/scripts/true-baseline.sh"

mkdir -p "$OUT"
WRAPPER="$OUT/rubash-profile-wrapper.sh"
cat > "$WRAPPER" <<EOF
#!/usr/bin/bash
# rubash#815 differential launcher: profiler ON, reports to the scratch
# dir (never the suite cwd). The suite operand stays \$1 exactly as
# true-baseline.sh passes it. The report path stays in the NATIVE Windows
# form — rubash.exe is a Windows process and cannot resolve /mnt/...
# (docs/windows-path-contract.md) — while the launcher path itself is WSL.
# Shebang is ABSOLUTE: the harness runs with a restricted PATH
# (bash-tests-rw + the Windows toolset only), so /usr/bin/env bash
# resolves to nothing there (observed as rc 127 with an empty rb.out).
exec "$BIN_WS" --profile --profile-output "$(to_native "$OUT")/last.collapsed" "\$@"
EOF
chmod +x "$WRAPPER"

echo "profiler-differential: binary  $BIN"
echo "profiler-differential: wrapper $WRAPPER"
echo "profiler-differential: ledger  $LOG"

MSYS_NO_PATHCONV=1 wsl bash -c "RUB_OVERRIDE='$(to_wsl "$WRAPPER")' LOG_OVERRIDE='$(to_wsl "$LOG")' bash '$HARNESS_WS' $*"
status=$?

echo "profiler-differential: compare failure sets against the profiler-off baseline:"
echo "  awk '{print \$1, \$2}' $ROOT/target/issue-suites/results/true-baseline-ledger.log | sort > /tmp/off.set"
echo "  awk '{print \$1, \$2}' $LOG | sort > /tmp/on.set"
echo "  diff /tmp/off.set /tmp/on.set"
exit $status
