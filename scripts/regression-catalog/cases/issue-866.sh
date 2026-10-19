# rubash#866: a user-forgeable ambient __RUBASH_BACKGROUND_WRAPPER value
# must not impose the wrapper's PDEATHSIG semantics on children that never
# went through the wrapper spawn path. GNU never imposes it: the directly
# exec'd background child of the inner shell outlives that shell and writes
# the marker, so the outer (still-alive) observer prints SURVIVED. The
# pre-fix arm armed the child from the forged env and it died at the inner
# shell's exit -> DIED. The outer shell stays alive for the whole window so
# the marker file decouples the orphan's late write from the runner's wait.
export __RUBASH_BACKGROUND_WRAPPER=1
"$BASH" -c 'sh -c "sleep 2; echo SURVIVED > marker866" & exit 0'
sleep 3
[ -f marker866 ] && echo SURVIVED || echo DIED
