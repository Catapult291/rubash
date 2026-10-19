# 714.1 - read -n/-N counts DECODED units, not transport units (rubash#714).
# A raw high byte travels as a two-char carrier pair; when the limit landed
# inside a pair pre-fix, the byte budget cut mid-pair and the affected byte
# surfaced as a stray carrier instead of the raw byte. Fixed by PR #775
# (count decoded units). GNU reads raw bytes, so both sides must print the
# same three kept bytes for each limit shape.
printf 'a\377b\377c\n' > f
read -N 3 v < f; printf 'N3=[%s]\n' "$v"
read -n 3 w < f; printf 'n3=[%s]\n' "$w"
