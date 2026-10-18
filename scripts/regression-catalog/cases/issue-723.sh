# 723.1 - `producer | head | while read` must terminate with the consumer's
# output (rubash#723, fixed by PR #793). Pre-fix this stalled with sys-CPU
# burn and empty output; the runner's INCOMPLETE-on-timeout is the pre-fix
# verdict, so a hang can never score as a pass.
yes | head -3 | while read -r l; do echo "got:$l"; done
echo done
