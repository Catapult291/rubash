#!/usr/bin/env bash
# Perf probe 29: external spawn loop at the #800A asset scale — 32 spawns.
# kiddae/dotfiles hermine .bashrc (eco-harvest run 39078222286) pays
# ~32 `$(tput …)` spawns; verdict source_wall was 9.7s ≈ ~300ms/spawn on the
# windows runner. `uname -s >/dev/null` is the OS-neutral stand-in for the
# tput child (probe 09 covers the x300 sweep; this one pins the per-spawn
# budget at the asset's own multiplier).
# Shape family: fork/exec loop (the "tput x32" face of #800A).
# PERF: runs=7 timeout=60
i=0
while [ "$i" -lt 32 ]; do
  uname -s >/dev/null
  i=$((i+1))
done
exit 0
