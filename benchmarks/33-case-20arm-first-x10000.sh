#!/usr/bin/env bash
# Perf probe 33: case dispatch — 20 literal arms x10000, subject matches arm 0.
# The match is found immediately, so this isolates the per-`case` fixed cost
# (head render / word expansion / arm-loop entry) from the per-arm scan of
# probe 32. Shape family: compound dispatch hot path (rubash#662 / #921).
# PERF: runs=3 timeout=60
i=0
while [ "$i" -lt 10000 ]; do
  case 0 in
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
