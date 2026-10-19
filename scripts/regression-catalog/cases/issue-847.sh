# issue rubash#847 - the two remaining faces of the span-state family.
#
# 1. E3b: the #822 assignment/reader ANSI-C span scanner tracked only
#    single-quote state, so a span inside double quotes was decoded anyway
#    (master stored `prexpost`). GNU keeps the whole spelling as data.
# 2. u-family: unset's deferred operand pass quote-removes a quoted-source
#    $'...' spelling instead of decoding it, so the $'...'-written cell
#    survives (the CR Nit 2 divergence #839 pinned). Bare and variable
#    spellings must keep deleting.
#
# GNU bash 5.3.0 baselines, byte-verified probes 2026-10-19.
k=$'x\ty'
declare -A W1; W1["pre$'x'post"]=w; printf 'W1='; printf '%s' "${!W1[@]}" | od -An -tx1 | tr -d ' \n'; printf '\n'
declare -A G; ((G["pre$'x'post"]=1)); printf 'gr=%s\n' "${G["pre$'x'post"]}"
printf 'gv='; printf '%s' "${!G[@]}" | od -An -tx1 | tr -d ' \n'; printf '\n'
declare -A E3; ((E3["pre$x'post"]=1)); printf 'E3='; printf '%s' "${!E3[@]}" | od -An -tx1 | tr -d ' \n'; printf '\n'
declare -A E1; E1['a$'b'c']=1; printf 'E1='; printf '%s' "${!E1[@]}" | od -An -tx1 | tr -d ' \n'; printf '\n'
declare -A A1; ((A1[$'x\ty']=1)); unset 'A1[$'"'"'x\ty'"'"']'; printf 'u1='; declare -p A1 | head -1
declare -A A2; ((A2[$'x\ty']=1)); unset "A2[$'x\ty']"; printf 'u2='; declare -p A2 | head -1
declare -A A3; ((A3[$'x\ty']=1)); unset "A3[\$'x\ty']"; printf 'u3='; declare -p A3 | head -1
declare -A G1; ((G1[$'x\ty']=1)); unset G1[$'x\ty']; printf 'bare='; declare -p G1 | head -1
declare -A A5; ((A5[$'x\ty']=1)); unset "A5[$k]"; printf 'var='; declare -p A5 | head -1
declare -A C; C[x]=a; C[\$x]=i; unset "C[\$'x']"; printf 'lit='; declare -p C | head -1
