# 788.1 - a W_COMPASSIGN operand makes the variable an array even without
# -a/-A, and the array bit survives an explicit `+a` in the same command
# (rubash#788, fixed by PR #786; GNU arrayfunc.c:454-514 runs the compound
# binding before the attribute pass). Pre-fix the storage word was written
# but ARRAY_VARS never set: ${a[1]} empty, ${#a[@]} 1, ${a@a} empty.
# Stdout-only by construction: the `declare +a` refusal face prints the
# diagnostic families covered by #551 and is pinned in tests/regression.rs
# (issue_788::case_2) instead, where stderr wording is out of scope.
declare a=(x y); echo "${a@a}|${#a[@]}|${a[1]}"
declare s=x; declare s+=(y); echo "${s@a}|${#s[@]}|${s[1]}"
f(){ local l=(x y); echo "${l@a}|${#l[@]}|${l[1]}"; }; f
declare q='(x y)'; echo "${q@a}|${#q[@]}"
declare -A m=([k]=v); echo "${m@a}|${#m[@]}|${m[k]}"
