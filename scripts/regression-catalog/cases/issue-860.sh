# issue rubash#860 - the unset empty-span/mask pseudo-diagnostic family.
#
# mask-before-expand: GNU's expand_word_internal scans left to right, so in
# `A4[$x\ty]` the `$name` extraction stops at the backslash (subst.c:10934
# legal_variable_char) and the unquoted `\t` collapses to `t` AFTER it — the
# deferred unset pass resolves key `ty` and keeps silent. rubash masked the
# escape first (`$x\ty` -> `$xty`), expanding the WRONG name: a bogus
# `[...]: bad array subscript` + rc1. The empty-span writes
# (`((U8[$'']=1))`, `U7[$'']=1`, `let "C9['']=1"`) share the region: GNU
# rejects the empty key with ONE `name['']` diagnostic and stores nothing.
#
# GNU bash 5.3.0 baselines, byte-verified probes 2026-10-19.
declare -A A4
unset 'A4[$x\ty]'; echo "u0rc=$?"
declare -p A4 | head -1
x=Q
declare -A B4; B4[Qty]=1; B4[xty]=2
unset 'B4[$x\ty]'; echo "u1rc=$?"
declare -p B4 | head -1
declare -A R4; R4[Qty]=5
printf 'r1=[%s]\n' "${R4[$x\ty]}"
unset x
declare -A S6; tx=ZZ; S6[$\tx]=1
printf 's6keys='; printf '%s' "${!S6[@]}" | od -An -tx1 | tr -d ' \n'; printf '\n'
declare -A U8; ((U8[$'']=1)); echo "e1rc=$?"
declare -p U8 | head -1
declare -A C2; ((C2['']=1)); echo "e2rc=$?"
declare -p C2 | head -1
declare -A U7; U7[$'']=1; echo "e3rc=$?"
declare -p U7 | head -1
declare -A C9; let "C9['']=1"; echo "e4rc=$?"
declare -p C9 | head -1
u=
declare -A Ca; let "Ca[$u]=1"; echo "e5rc=$?"
declare -p Ca | head -1
declare -A Cd; Cd[x]=5; ((Cd[$u]+=1)); echo "e6rc=$?"
declare -p Cd | head -1
declare -A C; z=$((C[$''])); echo "rd1=$z"
declare -A C3; w=$((C3[""])); echo "rd2=$w"
declare -A De; ((De['''''']=1)); echo "e7rc=$?"
