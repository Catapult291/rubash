#!/usr/bin/env bash
# Perf probe 30: prompt-render transform — 5000 ${PS1@P} renders of a
# spawn-free theme-shaped PS1 (#813 item 3, the #810 CR F6 gap).
# The #810 engine pass routes every render through decode_prompt_string plus
# the double-quoted embedded-parameter walk (parse.y:6683-6698); the CR
# measured ~50µs per render (20k in ~1s) with no frame-budget risk. This
# probe pins that budget: prompt escapes (\u \h \w \D{}) plus parameter,
# arithmetic-length and bare-special content, rendered into a scratch
# variable — no command substitution, so the loop measures only the
# decode+expand machinery. A future engine regression shows up as loop wall
# time, and a 60fps-budget violation (one render must stay far below a
# 16ms frame) shows up immediately.
# Shape family: prompt decode + promptvars second pass (${var@P} transform).
# PERF: runs=7 timeout=60
PS1='\u@\h:\w\$ [${SHLVL}:${#PWD}] rc=$? t=\D{%H:%M:%S} '
i=0
while [ "$i" -lt 5000 ]; do
  _rendered=${PS1@P}
  i=$((i+1))
done
exit 0
