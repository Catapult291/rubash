# issue rubash#846 - top-level `!` + compound + redirect failure.
# The title face (`! { :; } >bad` -> rc 1) is fixed; the residual faces
# tracked as rubash#940 still diverge, so this row stays `diverges`:
#   R1 - the redirect-failure diagnostic bypasses the ordered fd table
#        on the ()/simple faces (it must land in the already-opened
#        fd-2 file; it reaches real stderr instead), and
#   R2 - `!` + compound + redirect failure inside a brace group aborts
#        the whole script (the arm never reaches the last probes).
# Probe order puts the script-killing face last on purpose.
B=./no_dir_846/f

! { :; } 2>d1 >$B; echo "brace=$?"; [ -s d1 ] && echo d1-nonempty || echo d1-empty
! ( :; ) 2>d2 >$B; echo "paren=$?"; [ -s d2 ] && echo d2-nonempty || echo d2-empty
! true    2>d3 >$B; echo "true=$?";  [ -s d3 ] && echo d3-nonempty || echo d3-empty
{ : >$B; echo "group-cont"; } 2>d4
[ -s d4 ] && echo d4-nonempty || echo d4-empty

# R2: under GNU this prints nested=0 then survived=0; on the arm the
# script aborts after the diagnostic (INCOMPLETE - still a divergence).
{ ! { :; } >$B; echo "nested=$?"; }; echo "survived=$?"
