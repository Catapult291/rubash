# 776.1 - `read -N` binds the record verbatim: an embedded newline inside
# the N-byte budget is KEPT (rubash#776). Pre-fix the FUNCTION_STDIN mirror
# path dropped it, so the record rendered as [ab] instead of [a<NL>b].
printf 'a\nb\n' > f
read -N 3 v < f
printf '[%s]\n' "$v"
