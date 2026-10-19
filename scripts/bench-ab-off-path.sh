#!/usr/bin/env bash
# bench-ab-off-path.sh — rubash#815 acceptance: profiler-OFF hot-path cost.
# Interleaved A/B over the #607-#609 probe families (loop / arith / strconcat
# / fncall / cmdsub / external / expansion / assignment) between two rubash
# binaries, alternating runs to cancel machine noise. Median of N per probe.
#
# Usage: bash scripts/bench-ab-off-path.sh <binary-A> <binary-B> [runs]
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
A="$1"
B="$2"
RUNS="${3:-7}"

PROBES=(
  04-loop-true-builtin-x2000
  05-arith-x5000
  06-strconcat-x5000
  07-fncall-noop-x5000
  08-cmdsub-true-x1000
  09-external-uname-x300
  11-pipeline-yes-head
  15-expansion-x5000
  26-scalar-assign-x200000
  27-prefix-assign-x20000
  29-spawn-external-x32
)

median() { # median of whitespace-separated ints (ns)
  printf '%s\n' "$@" | sort -n | awk '{a[NR]=$1} END {print (NR%2) ? a[(NR+1)/2] : int((a[NR/2]+a[NR/2+1])/2)}'
}

printf "%-32s %12s %12s %8s\n" probe "A(ns)" "B(ns)" "B/A"
for probe in "${PROBES[@]}"; do
  script="$ROOT/benchmarks/$probe.sh"
  [ -f "$script" ] || { printf "%-32s MISSING\n" "$probe"; continue; }
  a_times=(); b_times=()
  # one validation run each (rc gate), then interleaved timed runs
  "$A" "$script" >/dev/null 2>&1
  "$B" "$script" >/dev/null 2>&1
  i=0
  while [ "$i" -lt "$RUNS" ]; do
    s=$(date +%s%N); "$A" "$script" >/dev/null 2>&1; e=$(date +%s%N)
    a_times+=($((e - s)))
    s=$(date +%s%N); "$B" "$script" >/dev/null 2>&1; e=$(date +%s%N)
    b_times+=($((e - s)))
    i=$((i + 1))
  done
  ma=$(median "${a_times[@]}")
  mb=$(median "${b_times[@]}")
  if [ "$ma" -gt 0 ]; then
    ratio=$(awk -v b="$mb" -v a="$ma" 'BEGIN {printf "%.3f", b / a}')
  else
    ratio="n/a"
  fi
  printf "%-32s %12s %12s %8s\n" "$probe" "$ma" "$mb" "$ratio"
done
