#!/usr/bin/env bash
# Perf probe 32: case dispatch — 20 literal arms x10000.
# Shape family: compound dispatch hot path (rubash#662 case-arm / #921).
# The subject is a constant that misses every literal arm, so all 20 arms
# are scanned per iteration (worst-case per-arm cost).
# PERF: runs=3 timeout=60
i=0
while [ "$i" -lt 10000 ]; do
  case zzz in
    0) ;;
    1) ;;
    2) ;;
    3) ;;
    4) ;;
    5) ;;
    6) ;;
    7) ;;
    8) ;;
    9) ;;
    10) ;;
    11) ;;
    12) ;;
    13) ;;
    14) ;;
    15) ;;
    16) ;;
    17) ;;
    18) ;;
    19) ;;
    *) ;;
  esac
  i=$((i + 1))
done
exit 0
