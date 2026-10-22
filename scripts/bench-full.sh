#!/usr/bin/env bash
# bench-full.sh — three-way perf sweep: GNU bash vs rubash vs niu.
#
# Runs every numbered probe under benchmarks/ against up to three shells and
# reports best-of-N (min-of-N) wall-clock milliseconds per probe.
#
# Usage (from the repo root):
#   bash scripts/bench-full.sh                           # full sweep
#   bash scripts/bench-full.sh --probe 05                # only probes matching "05"
#   bash scripts/bench-full.sh --runs 3                  # cap repetitions at 3
#   NIU_BIN=/path/to/niu bash scripts/bench-full.sh      # include product shell
#
# Shell selection (env overrides):
#   BASH_BIN    reference GNU bash            (default: bash)
#   RUBASH_BIN  candidate rubash binary       (default: target/release/rubash,
#               fallback: target/debug/rubash; RUBASH=... also honored)
#   NIU_BIN     product niu binary, optional  (default: unset = skipped -> null)
#
# Methodology: per probe, per shell: 1 validation+warmup run, then N timed
# runs, wall clock around the child, MINIMUM reported (best-of-N cancels
# scheduling noise; host-load discipline still applies — see docs/performance.md).
# Each run is bounded by the probe's PERF timeout (default 60s) via timeout(1)
# when available.
#
# Output contract (stable):
#   - stdout: human-readable table only. Sidecar files never change stdout
#     shape except a trailing "Artifacts:" footer line.
#   - sidecar files (written EVERY run, same stamp):
#       target/bench-results/bench-<UTC>.tsv   machine-readable table
#       target/bench-results/bench-<UTC>.json  same data + provenance meta
#     Override with --out-dir / --tsv / --json.
#
# TSV columns (tab-separated, one header line, no comments):
#   case  bash_ms  rubash_ms  niu_ms  ratio  method  runs  commit  date  machine  load
#   Missing shells (niu absent, TIMEOUT) leave the numeric field empty and the
#   ratio empty. ratio = rubash_ms / bash_ms (empty when either side missing).
#
# JSON shape:
#   {"meta": {"commit","date","machine","load","nproc","method",
#             "shells":{"bash","rubash","niu"}},
#    "results": [{"case","bash_ms","rubash_ms","niu_ms","ratio","method","runs",
#                 "status"}, ...]}
#   Missing numerics are null. Per-row "method" is "min-of-<N>".
#
# Portability: plain bash 4+ (also runs under Termux bash on Android, where
# `date +%s%N` may lack %N — falls back to python3 / $EPOCHREALTIME / seconds).
# This is why Android numbers are produced by running this script on-device
# rather than in CI (no hosted runner exists for the Termux phone).

set -u

# ---- clean environment (keep the measurement honest) -----------------------
unset BASH_ENV 2>/dev/null || true
unset WINUXSH_ROOT 2>/dev/null || true
unset -f rm rmdir unlink 2>/dev/null || true

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BENCH="$ROOT/benchmarks"

FILTER=""
RUNS_CAP=""
OUTDIR="$ROOT/target/bench-results"
JSON_OUT=""
TSV_OUT=""
while [ $# -gt 0 ]; do
  case "$1" in
    --probe) FILTER="$2"; shift ;;
    --runs) RUNS_CAP="$2"; shift ;;
    --out-dir) OUTDIR="$2"; shift ;;
    --json) JSON_OUT="$2"; shift ;;
    --tsv) TSV_OUT="$2"; shift ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
  shift
done

# ---- shell selection --------------------------------------------------------
resolve_exe() { # $1=name-or-path -> executable path (empty if none)
  if [ -x "$1" ]; then
    printf '%s\n' "$1"
  else
    command -v "$1" 2>/dev/null || true
  fi
}

BASH_BIN="$(resolve_exe "${BASH_BIN:-bash}")"
RUBASH_CANDIDATE="${RUBASH_BIN:-${RUBASH:-$ROOT/target/release/rubash}}"
RUBASH_BIN="$(resolve_exe "$RUBASH_CANDIDATE")"
if [ -z "$RUBASH_BIN" ]; then
  RUBASH_BIN="$(resolve_exe "$ROOT/target/debug/rubash")"
fi
NIU_BIN="$(resolve_exe "${NIU_BIN:-__absent__}")"
if [ -z "$BASH_BIN" ]; then
  echo "FATAL: reference bash not found (set BASH_BIN=...)" >&2
  exit 1
fi
if [ -z "$RUBASH_BIN" ]; then
  echo "FATAL: rubash binary not found/executable: $RUBASH_CANDIDATE" >&2
  echo "       build first (cargo build --release) or set RUBASH_BIN=<path>" >&2
  exit 1
fi
HAVE_NIU=0
if [ -n "$NIU_BIN" ]; then
  HAVE_NIU=1
fi

# ---- helpers ----------------------------------------------------------------

meta_field() { # $1=file $2=key -> value (empty if absent; quotes stripped)
  local line tok k v
  while IFS= read -r line; do
    case "$line" in
      "# PERF:"*) ;;
      *) continue ;;
    esac
    for tok in $line; do
      k="${tok%%=*}"
      if [ "$k" = "$2" ]; then
        v="${tok#*=}"
        v="${v#\"}"; v="${v%\"}"
        printf '%s\n' "$v"
        return 0
      fi
    done
  done < "$1"
  return 0
}

resolve_target() { # $1=repo-root $2=bench-dir $3=file-value $4=probe-path
  local root="$1" bench="$2" fval="$3" probe="$4"
  if [ -z "$fval" ]; then
    echo "$probe"
  elif [ -f "$bench/$fval" ]; then
    echo "$bench/$fval"
  elif [ -f "$root/$fval" ]; then
    echo "$root/$fval"
  else
    echo "$probe"   # let the run report the failure
  fi
}

enumerate_probes() {
  local p
  for p in "$BENCH"/[0-9][0-9]-*.sh; do
    [ -f "$p" ] || continue
    if [ -n "$FILTER" ]; then
      case "$(basename "$p")" in *"$FILTER"*) ;; *) continue ;; esac
    fi
    echo "$p"
  done
}

prepare_corpus() { # idempotent; LF copies under target/perf-corpus/
  mkdir -p "$ROOT/target/perf-corpus/nvm-dir" "$OUTDIR"
  if [ -f "$ROOT/third_party/bash/configure" ]; then
    tr -d '\r' < "$ROOT/third_party/bash/configure" > "$ROOT/target/perf-corpus/configure"
    head -1374 "$ROOT/target/perf-corpus/configure" > "$ROOT/target/perf-corpus/configure-head1374"
  fi
}

now_ns() { # monotonic-ish wall clock in nanoseconds (portable incl. Termux)
  local t
  t="$(date +%s%N 2>/dev/null || true)"
  case "$t" in
    ''|*N*|*%*) ;;
    *) printf '%s\n' "$t"; return 0 ;;
  esac
  if command -v python3 >/dev/null 2>&1; then
    python3 -c 'import time; print(time.time_ns())'
    return 0
  fi
  if [ -n "${EPOCHREALTIME:-}" ]; then
    awk -v e="$EPOCHREALTIME" 'BEGIN { printf "%.0f\n", e * 1e9 }'
    return 0
  fi
  t="$(date +%s 2>/dev/null || echo 0)"
  printf '%s000000000\n' "$t"
}

HAVE_TIMEOUT=0
command -v timeout >/dev/null 2>&1 && HAVE_TIMEOUT=1

# measure_shell <shell> <probe> <runs> <timeout_s> -> prints "min_ms|status"
# (min_ms empty on TIMEOUT; status is OK | RCn | TIMEOUT | MISSING).
measure_shell() {
  local sh="$1" probe="$2" runs="$3" timeout_s="$4"
  local args fval tgt rc status t0 t1
  args="$(meta_field "$probe" args)"
  fval="$(meta_field "$probe" file)"
  tgt="$(resolve_target "$ROOT" "$BENCH" "$fval" "$probe")"

  local cmd=("$sh")
  local a
  if [ -n "$args" ]; then
    for a in $args; do cmd+=("$a"); done   # args tokens never contain spaces
  fi
  if [ "$(meta_field "$probe" nofile)" != "1" ]; then
    cmd+=("$tgt")
  fi
  if [ ! -x "$sh" ] && ! command -v "$sh" >/dev/null 2>&1; then
    printf '|MISSING\n'
    return 0
  fi

  local run=()
  if [ "$HAVE_TIMEOUT" -eq 1 ]; then
    run=(timeout -k 5 "$timeout_s")
  fi
  # ${run[@]+"${run[@]}"}: empty-array safe under `set -u` on bash < 4.4.
  ${run[@]+"${run[@]}"} "${cmd[@]}" >/dev/null 2>&1 </dev/null
  rc=$?
  if [ "$rc" -eq 124 ]; then
    printf '|TIMEOUT\n'
    return 0
  fi
  if [ "$rc" -eq 0 ]; then status=OK; else status="RC$rc"; fi

  local times_ns="" i ms
  for i in $(seq 1 "$runs"); do
    t0="$(now_ns)"
    ${run[@]+"${run[@]}"} "${cmd[@]}" >/dev/null 2>&1 </dev/null
    rc=$?
    t1="$(now_ns)"
    if [ "$rc" -eq 124 ]; then
      printf '|TIMEOUT\n'
      return 0
    fi
    ms="$(awk -v a="$t0" -v b="$t1" 'BEGIN { printf "%.3f", (b - a) / 1e6 }')"
    times_ns="$times_ns$ms
"
  done
  local best
  best="$(printf '%s' "$times_ns" | sort -n | head -1)"
  printf '%s|%s\n' "$best" "$status"
}

json_str() { # $1=raw -> JSON-quoted string (stdout)
  local s="$1"
  s="${s//\\/\\\\}"
  s="${s//\"/\\\"}"
  s="${s//$'\t'/\\t}"
  s="${s//$'\n'/\\n}"
  s="${s//$'\r'/\\r}"
  printf '"%s"' "$s"
}

num_or_null() { # $1=ms-or-empty -> ms or JSON null
  if [ -n "$1" ]; then printf '%s' "$1"; else printf 'null'; fi
}

# ---- provenance meta ----------------------------------------------------------

prepare_corpus

COMMIT="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
DATE_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || echo unknown)"
MACHINE="$(uname -srm 2>/dev/null || uname -a 2>/dev/null || echo unknown)"
if [ -r /proc/loadavg ]; then
  LOAD="loadavg $(cat /proc/loadavg)"
else
  LOAD="$(uptime 2>/dev/null || echo 'load unknown')"
fi
NPROC="$(nproc 2>/dev/null || getconf _NPROCESSORS_ONLN 2>/dev/null || echo 1)"
BASH_VER="$("$BASH_BIN" --version 2>/dev/null | head -1)"
RUBASH_VER="$("$RUBASH_BIN" --version 2>/dev/null | head -1)"
if [ "$HAVE_NIU" -eq 1 ]; then
  NIU_VER="$("$NIU_BIN" --version 2>/dev/null | head -1)"
else
  NIU_VER="(absent)"
fi

# ---- sweep --------------------------------------------------------------------

TS_ROWS=""   # accumulated TSV body lines
JSON_ITEMS="" # accumulated JSON result objects
HUMAN=""      # accumulated human table lines

for probe in $(enumerate_probes); do
  name="$(basename "$probe" .sh)"
  runs="$(meta_field "$probe" runs)";    [ -n "$runs" ] || runs=7
  timeout_s="$(meta_field "$probe" timeout)"; [ -n "$timeout_s" ] || timeout_s=60
  if [ -n "$RUNS_CAP" ] && [ "$runs" -gt "$RUNS_CAP" ]; then runs="$RUNS_CAP"; fi
  method="min-of-$runs"

  b_out="$(measure_shell "$BASH_BIN" "$probe" "$runs" "$timeout_s")"
  r_out="$(measure_shell "$RUBASH_BIN" "$probe" "$runs" "$timeout_s")"
  if [ "$HAVE_NIU" -eq 1 ]; then
    n_out="$(measure_shell "$NIU_BIN" "$probe" "$runs" "$timeout_s")"
  else
    n_out="|SKIP"
  fi
  b_ms="${b_out%%|*}"; b_st="${b_out##*|}"
  r_ms="${r_out%%|*}"; r_st="${r_out##*|}"
  n_ms="${n_out%%|*}"; n_st="${n_out##*|}"

  ratio=""
  if [ -n "$b_ms" ] && [ -n "$r_ms" ]; then
    ratio="$(awk -v r="$r_ms" -v g="$b_ms" 'BEGIN { if (g > 0) printf "%.3f", r / g }')"
  fi

  status="$r_st"
  [ "$b_st" != "$status" ] && status="$status/bash:$b_st"
  if [ "$n_st" != "SKIP" ] && [ "$n_st" != "$r_st" ]; then
    status="$status/niu:$n_st"
  fi

  b_cell="${b_ms:--}"; r_cell="${r_ms:--}"; n_cell="${n_ms:--}"
  HUMAN="$HUMAN$(printf '%-34s %12s %12s %12s %8s   %s\n' \
    "$name" "$b_cell" "$r_cell" "$n_cell" "${ratio:--}" "$status")"$'\n'

  TS_ROWS="$TS_ROWS$(printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$name" "$b_ms" "$r_ms" "$n_ms" "$ratio" "$method" "$runs" \
    "$COMMIT" "$DATE_UTC" "$MACHINE" "$LOAD")"$'\n'

  ratio_json="null"; [ -n "$ratio" ] && ratio_json="$ratio"
  JSON_ITEMS="$JSON_ITEMS$(printf '{"case":"%s","bash_ms":%s,"rubash_ms":%s,"niu_ms":%s,"ratio":%s,"status":"%s"},' \
    "$name" "$(num_or_null "$b_ms")" "$(num_or_null "$r_ms")" "$(num_or_null "$n_ms")" \
    "$ratio_json" "$status")"$'\n'
done
JSON_ITEMS="${JSON_ITEMS%$'\n'}" # strip trailing newline (empty sweep -> empty string)
JSON_ITEMS="${JSON_ITEMS%,}"   # strip trailing comma of last object

# ---- emit sidecars (machine-readable; stdout shape untouched) -------------------

STAMP="$(date -u +%Y%m%dT%H%M%SZ 2>/dev/null || echo unknown)"
[ -n "$TSV_OUT" ] || TSV_OUT="$OUTDIR/bench-$STAMP.tsv"
[ -n "$JSON_OUT" ] || JSON_OUT="$OUTDIR/bench-$STAMP.json"
mkdir -p "$OUTDIR"

{
  printf 'case\tbash_ms\trubash_ms\tniu_ms\tratio\tmethod\truns\tcommit\tdate\tmachine\tload\n'
  printf '%s' "$TS_ROWS"
} > "$TSV_OUT"

{
  printf '{\n"meta": {\n'
  printf '  "commit": %s,\n' "$(json_str "$COMMIT")"
  printf '  "date": %s,\n' "$(json_str "$DATE_UTC")"
  printf '  "machine": %s,\n' "$(json_str "$MACHINE")"
  printf '  "load": %s,\n' "$(json_str "$LOAD")"
  printf '  "nproc": %s,\n' "$(json_str "$NPROC")"
  printf '  "method": "best-of-N (per-shell minimum)",\n'
  printf '  "shells": {"bash": %s, "rubash": %s, "niu": %s}\n' \
    "$(json_str "$BASH_VER")" "$(json_str "$RUBASH_VER")" "$(json_str "$NIU_VER")"
  printf '},\n"results": [\n%s\n]\n}\n' "$JSON_ITEMS"
} > "$JSON_OUT"

# ---- human-readable stdout (unchanged contract: table + footer) -----------------

printf '%-34s %12s %12s %12s %8s   %s\n' "case" "bash_ms" "rubash_ms" "niu_ms" "r/b" "status"
printf '%s' "$HUMAN"
echo
echo "method: best-of-N (min); commit $COMMIT; date $DATE_UTC"
echo "machine: $MACHINE ($LOAD)"
echo "Artifacts: $TSV_OUT $JSON_OUT"
