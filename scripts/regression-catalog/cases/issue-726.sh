# 726.1/726.2 - redirect leftovers from #699 (rubash#726, fixed by PR #783).
# 726.1: a FUNCTION DEFINITION-level dup splice binds the body's default
# streams, so `f ... 1>out` still routes OUT to err. 726.2: a body-level
# `exec > g` rebinding must NOT escape the group's ordered redirects: the
# trailing `2>&1` captures `x` (the subshell keeps the harness's own stdout
# usable, so the completeness marker still prints).
f(){ echo OUT; echo ERR >&2; } 2>&1 >err
f 1>out
printf 'out=[%s] err=[%s]\n' "$(cat out)" "$(cat err)"
( { exec > g; echo x >&2; } 2>&1 )
printf 'g=[%s]\n' "$(cat g)"
