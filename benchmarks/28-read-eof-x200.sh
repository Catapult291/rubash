#!/usr/bin/env bash
# Perf probe 28: read-on-EOF storm — 200 reads on an exhausted stdin (#800B).
# addition.sh (eco-harvest run 39078222286) measured ~4.8s per `read` at EOF;
# GNU bash returns rc=1 from a pipe/file EOF immediately (read.def:762-768).
# 200 iterations turn any per-read stall into seconds; a healthy reader
# finishes in well under a second.
# Shape family: EOF-bound read loop (the #800B read face).
# PERF: runs=7 timeout=60
i=0
while [ "$i" -lt 200 ]; do
  read -r line </dev/null
  i=$((i+1))
done
exit 0
